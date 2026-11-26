// `for` loops, ranges and iterator chains (-> counted loops / java.util.stream).
// Iterator values are `Stream<T>` (ranges are boxed); iterators pulled with `next()`/`peek()`
// become a `ListIterator` over a materialized list.

use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasLoopBody, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, LoopFrame, expr, strip_parens, traits};
use crate::emit::render_expr as render;
use crate::jir::{Block, Expr, LambdaBody, Stmt, StmtKind};
use crate::ty::{Int, Ty};

// ---- for loops ---------------------------------------------------------------------------

pub fn lower_for(cx: &mut Cx, f: &ast::ForExpr, out: &mut Vec<Stmt>) {
    let line = cx.line(f.syntax());
    let (Some(p), Some(iterable)) = (f.pat(), f.iterable()) else {
        let e = cx.unsupported(f.syntax(), "for loop");
        out.push(Stmt::new(line, StmtKind::Expr(e)));
        return;
    };
    let rust_label = f
        .label()
        .and_then(|l| l.lifetime())
        .map(|l| l.text().to_string());
    cx.loops.push(LoopFrame {
        java_label: rust_label.as_deref().map(stmt::java_label_name),
        rust_label,
        value_dest: None,
        switch_depth: cx.switch_depth,
        is_block: false,
    });
    cx.push_scope();
    let iterable = strip_parens(&iterable);
    let stmt = lower_for_kind(cx, f, &p, &iterable, line, out);
    cx.pop_scope();
    let label = cx.loops.pop().unwrap().java_label;
    out.push(match stmt {
        ForStmt::Counted { header, body } => Stmt::new(
            line,
            StmtKind::For {
                label,
                header,
                body,
            },
        ),
        ForStmt::Each {
            ty,
            name,
            iter,
            body,
        } => Stmt::new(
            line,
            StmtKind::ForEach {
                label,
                ty,
                name,
                iter,
                body,
            },
        ),
    });
}

enum ForStmt {
    Counted {
        header: String,
        body: Block,
    },
    Each {
        ty: String,
        name: String,
        iter: Expr,
        body: Block,
    },
}

fn lower_for_kind(
    cx: &mut Cx,
    f: &ast::ForExpr,
    p: &ast::Pat,
    iterable: &ast::Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) -> ForStmt {
    // `for i in a..b` and `(a..b).rev()` / `.step_by(k)`: counted loops.
    let wildcard = matches!(p, ast::Pat::WildcardPat(_));
    if let Some(name) = simple_ident(p).or_else(|| wildcard.then(|| "i".to_string()))
        && let Some((r, shape)) = range_loop_shape(iterable)
    {
        let (start, end, inclusive) = range_bounds(cx, &r, out);
        let step = match &shape {
            RangeShape::StepBy(k) => Some(expr::lower_expr(cx, k, out)),
            _ => None,
        };
        let ty = cx.pat_ty(p);
        let jt = ty.java();
        let captured = matches!(p, ast::Pat::IdentPat(ip) if cx.binding_captured(ip));
        // Lambdas can't capture a mutating loop counter: count with a temp, rebind per iteration.
        // `for _ in` gets a Java-only counter, so it can't capture a Rust `i` from outside.
        let counter = if wildcard && crate::mode::simple() {
            cx.claim_first(&["i", "j", "k", "l", "m", "n"])
                .unwrap_or_else(|| cx.fresh(&name))
        } else if captured || wildcard {
            cx.fresh(&name)
        } else {
            cx.declare(&name)
        };
        let (s, e) = (render(&start), render(&end));
        let cmp = if inclusive { "<=" } else { "<" };
        let header = match shape {
            RangeShape::Plain => format!("{jt} {counter} = {s}; {counter} {cmp} {e}; {counter}++"),
            RangeShape::Rev => {
                let first = if inclusive {
                    e
                } else {
                    render(&Expr::bin(end, "-", Expr::raw("1")))
                };
                format!("{jt} {counter} = {first}; {counter} >= {s}; {counter}--")
            }
            RangeShape::StepBy(_) => {
                format!(
                    "{jt} {counter} = {s}; {counter} {cmp} {e}; {counter} += {}",
                    render(&step.unwrap())
                )
            }
        };
        let rebind = captured.then(|| {
            let var = cx.declare(&name);
            Stmt::new(
                line,
                StmtKind::Local {
                    ty: jt.clone(),
                    name: var,
                    init: Some(Expr::name(counter.clone())),
                },
            )
        });
        let mut body = loop_body(cx, f);
        if let Some(r) = rebind {
            body.stmts.insert(0, r);
        }
        return ForStmt::Counted { header, body };
    }

    // `for (i, x) in v.iter().enumerate()` / `s.chars().enumerate()`, `for (a, b) in
    // v.iter().zip(w.iter().skip(1))`, `for w in v.windows(k)` / `v.chunks(k)`: index loops.
    if let Some(shape) = super::indexed::shape(cx, iterable)
        && let Some(fields) = super::indexed::pattern_fields(p, &shape)
    {
        let ix = super::indexed::lower(cx, &shape, line, out);
        // The user's own index name when it's a plain, uncaptured binding.
        let plain_idx = matches!(ix.items[0], super::indexed::Item::Index)
            .then(|| simple_ident(&fields[0]))
            .flatten()
            .filter(|_| !matches!(&fields[0], ast::Pat::IdentPat(ip) if cx.binding_captured(ip)));
        let idx = match &plain_idx {
            Some(n) => cx.declare(n),
            None if crate::mode::simple() => cx
                .claim_first(&["i", "j", "k", "l", "m", "n"])
                .unwrap_or_else(|| cx.fresh("i")),
            None => cx.fresh("i"),
        };
        let header = ix.header(&idx);
        let mut binds = Vec::new();
        for ((field, value), item) in fields.iter().zip(super::indexed::item_exprs(&ix, &idx)).zip(&ix.items) {
            if plain_idx.is_some() && matches!(value, Expr::Name(ref n) if *n == idx) {
                continue;
            }
            // `for (a, b) in v.iter_mut().zip(..)`: `a` names the slot `v[i]`, so `*a += b` writes it.
            if let ast::Pat::IdentPat(ip) = field
                && ip.pat().is_none()
                && matches!(cx.pat_ty(field), Ty::Ref { mutable: true, ref inner } if super::is_mut_ref_cell_ty(&Ty::Ref { mutable: true, inner: inner.clone() }))
                && let super::indexed::Item::Elem { seq, text: false, .. } = item
                && let Some(local) = cx.sema.to_def(ip)
            {
                if let Some(n) = simple_ident(field) {
                    cx.declare(&n);
                }
                cx.elem_aliases.insert(local, (seq.clone(), item.at(&idx), super::ElemAliasKind::List));
                continue;
            }
            super::pat::bind_pattern(cx, field, value, &mut binds);
        }
        let mut body = loop_body(cx, f);
        binds.append(&mut body.stmts);
        body.stmts = binds;
        return ForStmt::Counted { header, body };
    }

    // `for x in v.iter_mut()` over immutable Java values: index loop where `*x` reads/writes `v[i]`.
    if let ast::Pat::IdentPat(ip) = p
        && let Some(name) = simple_ident(p)
        && let Some(list_ast) = iter_mut_of_list(cx, iterable)
    {
        let list = expr::lower_expr(cx, &list_ast, out);
        let list_ty = cx.ty(&list_ast);
        let list = ensure_simple(cx, list, &list_ty, line, out);
        let idx = cx.fresh("i");
        let header = format!("int {idx} = 0; {idx} < {}.size(); {idx}++", render(&list));
        let _ = cx.declare(&name);
        if let Some(local) = cx.sema.to_def(ip) {
            cx.elem_aliases
                .insert(local, (render(&list), idx, super::ElemAliasKind::List));
        }
        let body = loop_body(cx, f);
        return ForStmt::Counted { header, body };
    }

    if let ast::Expr::MethodCallExpr(call) = iterable
        && matches!(method_name(call).as_str(), "values_mut" | "iter_mut")
        && let Some(map_ast) = call.receiver()
        && matches!(cx.ty(&map_ast).strip_refs(), Ty::Map { .. })
    {
        let name = method_name(call);
        let (key_pat, value_pat) = if name == "iter_mut" {
            let ast::Pat::TuplePat(tuple) = p else {
                return unsupported_for(cx, f, "map iter_mut pattern");
            };
            let fields: Vec<_> = tuple.fields().collect();
            if fields.len() != 2 {
                return unsupported_for(cx, f, "map iter_mut pattern");
            }
            (Some(fields[0].clone()), fields[1].clone())
        } else {
            (None, p.clone())
        };
        let ast::Pat::IdentPat(value) = &value_pat else {
            return unsupported_for(cx, f, "map mutable value pattern");
        };
        let Some(local) = cx.sema.to_def(value) else {
            return unsupported_for(cx, f, "map mutable value binding");
        };
        let map = expr::lower_expr(cx, &map_ast, out);
        let entry = cx.fresh("entry");
        let mut binds = Vec::new();
        if let Some(key) = key_pat {
            super::pat::bind_pattern(
                cx,
                &key,
                Expr::call(Expr::name(entry.clone()), "getKey", vec![]),
                &mut binds,
            );
        }
        if let Some(name) = simple_ident(&value_pat) {
            cx.declare(&name);
        }
        cx.elem_aliases.insert(
            local,
            (entry.clone(), String::new(), super::ElemAliasKind::Entry),
        );
        let mut body = loop_body(cx, f);
        binds.append(&mut body.stmts);
        body.stmts = binds;
        return ForStmt::Each {
            ty: "var".into(),
            name: entry,
            iter: Expr::call(map, "entrySet", vec![]),
            body,
        };
    }

    // Simple mode: `for (i, x) in it.enumerate()` over a stream (map entries, stdin lines) counts
    // with a plain `int` instead of numbering the stream through an `AtomicInteger`.
    if crate::mode::simple()
        && let ast::Expr::MethodCallExpr(m) = iterable
        && method_name(m) == "enumerate"
        && let Some(src) = m.receiver()
        && let ast::Pat::TuplePat(tp) = p
        && let [idx_pat, elem_pat] = tp.fields().collect::<Vec<_>>().as_slice()
        && let ast::Pat::IdentPat(idx_ip) = idx_pat
        && idx_ip.mut_token().is_none()
        && let Some(idx_name) = simple_ident(idx_pat)
    {
        return counted_each(cx, f, idx_ip, &idx_name, elem_pat, &strip_parens(&src), line, out);
    }
    for_each(cx, f, p, iterable, out)
}

fn for_each(cx: &mut Cx, f: &ast::ForExpr, p: &ast::Pat, iterable: &ast::Expr, out: &mut Vec<Stmt>) -> ForStmt {
    // A line-mode stdin `lines()` stream carries plain Strings, not phantom `Result`s.
    let elem_ty = if super::std_map::io::plain_lines_iterable(cx, iterable) {
        Ty::Str
    } else {
        cx.pat_ty(p)
    };
    let it_ty = cx.ty(iterable);
    let iter = for_each_source(cx, iterable, &it_ty, &elem_ty, out);
    // Leaving early must leave the rest of stdin unread.
    let iter = match iter {
        Expr::Call { recv: Some(r), name, .. } if name == "toList" && reads_stdin(&r) && exits_early(cx, f) => {
            Expr::Cast("Iterable<String>".into(), Box::new(Expr::raw(format!("{}::iterator", render(&r)))))
        }
        other => other,
    };
    let (name, mut binds) = match simple_ident(p) {
        Some(n) => (cx.declare(&n), Vec::new()),
        None => {
            let tmp = cx.fresh("item");
            let mut binds = Vec::new();
            super::pat::bind_pattern(cx, p, Expr::name(tmp.clone()), &mut binds);
            (tmp, binds)
        }
    };
    let mut body = loop_body(cx, f);
    binds.append(&mut body.stmts);
    body.stmts = binds;
    ForStmt::Each {
        ty: stmt::decl_type(&elem_ty),
        name,
        iter,
        body,
    }
}

// `int i = 0; for (x : src) { ...; i++; }`, or `int i = index++;` first in the body when the
// increment could be skipped (`continue`), be unreachable, or `i` is captured by a lambda.
#[allow(clippy::too_many_arguments)]
fn counted_each(
    cx: &mut Cx,
    f: &ast::ForExpr,
    idx_ip: &ast::IdentPat,
    idx_name: &str,
    elem_pat: &ast::Pat,
    src: &ast::Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) -> ForStmt {
    let jt = cx.pat_ty(&ast::Pat::IdentPat(idx_ip.clone())).java();
    let captured = cx.binding_captured(idx_ip);
    let skips = continues_loop(f);
    let pos = out.len();
    let idx = cx.declare(idx_name);
    let counter = if captured || skips { cx.fresh("index") } else { idx.clone() };
    let ForStmt::Each { ty, name, iter, mut body } = for_each(cx, f, elem_pat, src, out) else {
        unreachable!()
    };
    let falls_through = body.stmts.last().is_none_or(completes);
    let counter = if counter == idx && !falls_through { cx.fresh("index") } else { counter };
    out.insert(
        pos,
        Stmt::new(line, StmtKind::Local { ty: jt.clone(), name: counter.clone(), init: Some(Expr::raw("0")) }),
    );
    if counter == idx {
        let end = body.close_line.max(line);
        body.stmts.push(Stmt::new(end, StmtKind::Expr(Expr::raw(format!("{idx}++")))));
    } else {
        body.stmts.insert(
            0,
            Stmt::new(line, StmtKind::Local { ty: jt, name: idx, init: Some(Expr::raw(format!("{counter}++"))) }),
        );
    }
    ForStmt::Each { ty, name, iter, body }
}

// A `break`, `return` or `?` in the body (conservatively, at any depth); `line?` on a plain stdin
// line never leaves.
fn exits_early(cx: &Cx, f: &ast::ForExpr) -> bool {
    f.loop_body().is_some_and(|b| {
        b.syntax().descendants().any(|n| {
            ast::BreakExpr::can_cast(n.kind())
                || ast::ReturnExpr::can_cast(n.kind())
                || ast::TryExpr::cast(n).is_some_and(|t| {
                    !t.expr().is_some_and(|e| super::std_map::io::phantom_line_result(cx, &e))
                })
        })
    })
}

// `STDIN.lines()`: the line reader over stdin.
fn reads_stdin(e: &Expr) -> bool {
    matches!(e, Expr::Call { recv: Some(r), name, .. } if name == "lines" && matches!(&**r, Expr::Raw(s) | Expr::Name(s) if s == "STDIN"))
}

// A `continue` (bare or labeled) that restarts this loop.
fn continues_loop(f: &ast::ForExpr) -> bool {
    let label = f.label().and_then(|l| l.lifetime()).map(|l| l.text().to_string());
    let Some(body) = f.loop_body() else { return false };
    body.syntax().descendants().filter_map(ast::ContinueExpr::cast).any(|c| {
        match c.lifetime() {
            Some(l) => Some(l.text().to_string()) == label,
            // Bare: the innermost loop around it is this one.
            None => c
                .syntax()
                .ancestors()
                .find(|a| {
                    ast::ForExpr::can_cast(a.kind())
                        || ast::WhileExpr::can_cast(a.kind())
                        || ast::LoopExpr::can_cast(a.kind())
                })
                .is_some_and(|a| a == *f.syntax()),
        }
    })
}

// Whether control can reach the statement after `s`.
fn completes(s: &Stmt) -> bool {
    match &s.kind {
        StmtKind::Return(_) | StmtKind::Break(_) | StmtKind::Continue(_) | StmtKind::Throw(_) => false,
        StmtKind::If { then, els: Some(e), .. } => then.stmts.last().is_none_or(completes) || completes(e),
        StmtKind::Block(b) => b.stmts.last().is_none_or(completes),
        StmtKind::Switch { arms, .. } => arms.iter().any(|a| match &a.body {
            crate::jir::ArmBody::Expr(_) => true,
            crate::jir::ArmBody::Throw(_) => false,
            crate::jir::ArmBody::Block(b) => b.stmts.last().is_none_or(completes),
        }),
        StmtKind::While { cond, .. } => !matches!(cond, Expr::Raw(t) if t == "true"),
        _ => true,
    }
}

fn unsupported_for(cx: &mut Cx, f: &ast::ForExpr, reason: &str) -> ForStmt {
    let value = cx.unsupported(f.syntax(), reason);
    ForStmt::Each {
        ty: "var".into(),
        name: cx.fresh("unsupported"),
        iter: value,
        body: Block::default(),
    }
}

fn loop_body(cx: &mut Cx, f: &ast::ForExpr) -> Block {
    f.loop_body()
        .map(|b| stmt::lower_block(cx, &b, &Dest::Discard))
        .unwrap_or_default()
}

enum RangeShape {
    Plain,
    Rev,
    StepBy(ast::Expr),
}

fn range_loop_shape(e: &ast::Expr) -> Option<(ast::RangeExpr, RangeShape)> {
    use ast::RangeItem;
    match strip_parens(e) {
        ast::Expr::RangeExpr(r) if r.start().is_some() && r.end().is_some() => {
            Some((r, RangeShape::Plain))
        }
        ast::Expr::MethodCallExpr(m) => {
            let name = method_name(&m);
            let ast::Expr::RangeExpr(r) = strip_parens(&m.receiver()?) else {
                return None;
            };
            r.start()?;
            r.end()?;
            match name.as_str() {
                "rev" => Some((r, RangeShape::Rev)),
                "step_by" => Some((r, RangeShape::StepBy(m.arg_list()?.args().next()?))),
                _ => None,
            }
        }
        _ => None,
    }
}

fn iter_mut_of_list(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let target = match strip_parens(e) {
        ast::Expr::MethodCallExpr(m) if method_name(&m) == "iter_mut" => m.receiver()?,
        ast::Expr::RefExpr(r) if r.mut_token().is_some() => r.expr()?,
        _ => return None,
    };
    match cx.ty(&target).strip_refs() {
        Ty::Vec(t) if super::is_cell_value_ty(t) => Some(target),
        _ => None,
    }
}

// `v.iter_mut().for_each(|x| *x += k);` over numbers/Strings: the index loop that
// `for x in v.iter_mut()` becomes, with the closure body as the loop body.
fn iter_mut_for_each(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    if cx.lambda_depth > 0
        || !m
            .syntax()
            .parent()
            .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
    {
        return None;
    }
    let list_ast = iter_mut_of_list(cx, recv_ast)?;
    let Some(ast::Expr::ClosureExpr(c)) = arg_exprs(m).into_iter().next() else {
        return None;
    };
    let params: Vec<ast::Param> = c.param_list()?.params().collect();
    let [param] = params.as_slice() else {
        return None;
    };
    let Some(ast::Pat::IdentPat(ip)) = param.pat() else {
        return None;
    };
    let name = ip.name()?.text().to_string();
    let body = c.body()?;
    // A `return` would leave the closure call, which is a `continue` here; keep it simple.
    if body
        .syntax()
        .descendants()
        .any(|n| ast::ReturnExpr::can_cast(n.kind()))
    {
        return None;
    }
    let line = cx.line(m.syntax());
    let list = expr::lower_expr(cx, &list_ast, out);
    let list_ty = cx.ty(&list_ast);
    let list = ensure_simple(cx, list, &list_ty, line, out);
    let idx = cx.fresh("i");
    let header = format!("int {idx} = 0; {idx} < {}.size(); {idx}++", render(&list));
    cx.push_scope();
    let _ = cx.declare(&name);
    if let Some(local) = cx.sema.to_def(&ip) {
        cx.elem_aliases
            .insert(local, (render(&list), idx, super::ElemAliasKind::List));
    }
    let mut stmts = Vec::new();
    stmt::lower_to_dest(cx, &body, &Dest::Discard, &mut stmts);
    cx.pop_scope();
    let close_line = cx.end_line(body.syntax());
    out.push(Stmt::new(
        line,
        StmtKind::For {
            label: None,
            header,
            body: Block { stmts, close_line },
        },
    ));
    Some(Expr::raw("null"))
}

// The bounds of a range written as `a..b` / `a..=b` / `RangeInclusive::new(a, b)`, or of an
// immutable local initialised with one (a Java range is a one-shot stream, not a value to query).
fn range_parts(cx: &Cx, e: &ast::Expr) -> Option<(Option<ast::Expr>, Option<ast::Expr>, bool)> {
    use ast::RangeItem;
    match strip_parens(e) {
        ast::Expr::RangeExpr(r) => Some((r.start(), r.end(), r.op_kind() == Some(ast::RangeOp::Inclusive))),
        ast::Expr::CallExpr(c)
            if c.expr().is_some_and(|f| f.syntax().text().to_string().ends_with("RangeInclusive::new")) =>
        {
            let mut args = c.arg_list()?.args();
            Some((args.next(), args.next(), true))
        }
        p @ ast::Expr::PathExpr(_) => {
            let ip = cx.local_of_path(&p)?.primary_source(cx.db).into_ident_pat()?;
            if ip.mut_token().is_some() {
                return None;
            }
            let init = ip.syntax().parent().and_then(ast::LetStmt::cast)?.initializer()?;
            let parts = range_parts(cx, &init)?;
            // Read again at the use: the bounds must not have changed since.
            let fixed = |b: &Option<ast::Expr>| b.as_ref().is_none_or(|b| fixed_value(cx, b));
            (fixed(&parts.0) && fixed(&parts.1)).then_some(parts)
        }
        _ => None,
    }
}

// A literal, or an immutable local.
fn fixed_value(cx: &Cx, e: &ast::Expr) -> bool {
    match strip_parens(e) {
        ast::Expr::Literal(_) => true,
        ast::Expr::PrefixExpr(p) => p.expr().is_some_and(|e| fixed_value(cx, &e)),
        p @ ast::Expr::PathExpr(_) => cx
            .local_of_path(&p)
            .and_then(|l| l.primary_source(cx.db).into_ident_pat())
            .is_some_and(|ip| ip.mut_token().is_none()),
        _ => false,
    }
}

// `v.iter()` / `v.iter_mut()` / `v.into_iter()` / `&v` where v is a list: returns `v`.
pub(super) fn list_receiver_of_iter(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let target = match strip_parens(e) {
        ast::Expr::MethodCallExpr(m)
            if matches!(method_name(&m).as_str(), "iter" | "iter_mut" | "into_iter") =>
        {
            m.receiver()?
        }
        ast::Expr::RefExpr(r) => r.expr()?,
        other => other,
    };
    matches!(cx.ty(&target).strip_refs(), Ty::Vec(_)).then_some(target)
}

// Right-hand side of `for (T x : ...)`.
fn for_each_source(
    cx: &mut Cx,
    iterable: &ast::Expr,
    it_ty: &Ty,
    elem_ty: &Ty,
    out: &mut Vec<Stmt>,
) -> Expr {
    // `&v`, `v.iter()`, `v.into_iter()` over a collection: iterate the collection itself.
    let collection = match iterable {
        ast::Expr::MethodCallExpr(m)
            if matches!(method_name(m).as_str(), "iter" | "iter_mut" | "into_iter") =>
        {
            m.receiver()
        }
        ast::Expr::RefExpr(r) => r.expr(),
        _ => None,
    };
    if let Some(c) = collection {
        let cty = cx.ty(&c);
        if !matches!(cty.strip_refs(), Ty::Iter(_)) {
            let v = expr::lower_expr(cx, &c, out);
            // A Deref newtype iterates its target (`for x in w.iter()`).
            if let Some(target) = traits::deref_target(cx, &cty) {
                return collection_iterable(Expr::call(v, "deref", vec![]), &target);
            }
            return collection_iterable(v, &cty);
        }
    }
    if let ast::Expr::MethodCallExpr(m) = iterable
        && method_name(m) == "chars"
        && let Some(r) = m.receiver()
        && cx.ty(&r).is_str()
    {
        let s = expr::lower_expr(cx, &r, out);
        return Expr::call(s, "toCharArray", vec![]);
    }
    if let Some(local) = cx.local_of_path(iterable)
        && cx.pulled_iters.contains_key(&local)
    {
        let it = expr::lower_expr(cx, iterable, out);
        return Expr::Cast(
            format!("Iterable<{}>", elem_ty.boxed()),
            Box::new(Expr::raw(format!("() -> {}", render(&it)))),
        );
    }
    // Simple mode: `for x in [a, b]` only reads the literal, so `List.of(a, b)` will do.
    cx.read_only_literal =
        crate::mode::simple() && matches!(iterable, ast::Expr::ArrayExpr(a) if a.semicolon_token().is_none());
    let src = expr::lower_expr(cx, iterable, out);
    cx.read_only_literal = false;
    // A user type with `impl Iterator` iterates through its generated `stream()`.
    if let Some(item) = traits::iter_item(cx, it_ty) {
        let stream = Expr::call(src, "stream", vec![]);
        return Expr::Cast(
            format!("Iterable<{}>", item.boxed()),
            Box::new(Expr::raw(format!("{}::iterator", render(&stream)))),
        );
    }
    // A collection lowered to `c.stream()` is iterable as `c` itself.
    if let Some(list) = collection_stream(&src) {
        return list;
    }
    if crate::mode::simple()
        && let Some(plain) = simple_loop_source(&src)
    {
        return plain;
    }
    match it_ty.strip_refs() {
        Ty::Iter(t) => {
            let t = if super::std_map::io::plain_lines_iterable(cx, iterable) {
                elem_ty.clone()
            } else {
                (**t).clone()
            };
            Expr::Cast(
                format!("Iterable<{}>", t.boxed()),
                Box::new(Expr::raw(format!("{}::iterator", render(&src)))),
            )
        }
        other => collection_iterable(src, other),
    }
}

// Simple mode: a split's `Arrays.stream(parts)` iterates `parts`, and a bare `lines()` stream its
// `toList()` (stdin is read up front, which piped input doesn't notice).
fn simple_loop_source(src: &Expr) -> Option<Expr> {
    let Expr::Call { recv: Some(recv), name, args } = src else {
        return None;
    };
    match (name.as_str(), args.as_slice()) {
        ("stream", [arr]) if matches!(&**recv, Expr::Raw(r) if r == "Arrays") => Some(arr.clone()),
        ("lines", []) => Some(Expr::call(src.clone(), "toList", vec![])),
        _ => None,
    }
}

// `c.stream()` where `c` is a collection: the collection is already `Iterable`.
fn collection_stream(e: &Expr) -> Option<Expr> {
    let Expr::Call {
        recv: Some(recv),
        name,
        args,
    } = e
    else {
        return None;
    };
    if name != "stream" || !args.is_empty() {
        return None;
    }
    match &**recv {
        Expr::Name(_) => Some((**recv).clone()),
        Expr::Call { name, .. }
            if matches!(name.as_str(), "values" | "keySet" | "entrySet" | "reversed") =>
        {
            Some((**recv).clone())
        }
        _ => None,
    }
}

// Simple mode: `v.iter().for_each(|x| { .. })` as a statement over a list, set or deque is a
// for-each loop, so the body may assign outer locals without one-element-array cells.
pub fn closure_is_loop(cx: &Cx, c: &ast::ClosureExpr) -> bool {
    if !crate::mode::simple() {
        return false;
    }
    let Some(call) = c
        .syntax()
        .parent()
        .filter(|a| ast::ArgList::can_cast(a.kind()))
        .and_then(|a| a.parent())
        .and_then(ast::MethodCallExpr::cast)
    else {
        return false;
    };
    let statement = call
        .syntax()
        .parent()
        .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()));
    let Some(ast::Expr::MethodCallExpr(src)) = call.receiver().map(|r| strip_parens(&r)) else {
        return false;
    };
    let collection = matches!(method_name(&src).as_str(), "iter" | "into_iter")
        && src.receiver().is_some_and(|v| {
            matches!(
                cx.ty(&v).strip_refs(),
                Ty::Vec(_) | Ty::Set { .. } | Ty::Deque(_)
            )
        });
    let one_param = c.param_list().is_some_and(|pl| pl.params().count() == 1);
    // `return` in the closure body means `continue`; `?` would leave the enclosing function.
    let plain_body = c.body().is_some_and(|b| {
        !b.syntax().descendants().any(|n| {
            (ast::ReturnExpr::can_cast(n.kind()) || ast::TryExpr::can_cast(n.kind()))
                && !n
                    .ancestors()
                    .take_while(|a| a != c.syntax())
                    .any(|a| ast::ClosureExpr::can_cast(a.kind()))
        })
    });
    method_name(&call) == "for_each" && statement && collection && one_param && plain_body
}

fn for_each_loop(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: &ast::Expr,
    c: &ast::ClosureExpr,
    out: &mut Vec<Stmt>,
) {
    let line = cx.line(m.syntax());
    let (Some(ast::Expr::MethodCallExpr(src)), Some(p)) = (
        Some(strip_parens(recv)),
        c.param_list()
            .and_then(|pl| pl.params().next())
            .and_then(|p| p.pat()),
    ) else {
        return;
    };
    let Some(coll) = src.receiver() else { return };
    let cty = cx.ty(&coll);
    let v = expr::lower_expr(cx, &coll, out);
    let iter = collection_iterable(v, &cty);
    cx.push_scope();
    let ty = cx.pat_ty(&p);
    let jty = if ty.is_nameable() {
        ty.strip_refs().java()
    } else {
        "var".into()
    };
    let mut prelude = Vec::new();
    let name = match simple_ident(&p) {
        Some(n) => cx.declare(&n),
        None => {
            let tmp = cx.fresh("item");
            super::pat::bind_pattern(cx, &p, Expr::name(&tmp), &mut prelude);
            tmp
        }
    };
    let mut body = match c.body() {
        Some(ast::Expr::BlockExpr(b)) => stmt::lower_block(cx, &b, &Dest::Discard),
        Some(e) => {
            let mut stmts = Vec::new();
            stmt::lower_to_dest(cx, &e, &Dest::Discard, &mut stmts);
            Block {
                stmts,
                close_line: cx.end_line(e.syntax()),
            }
        }
        None => Block::default(),
    };
    cx.pop_scope();
    prelude.append(&mut body.stmts);
    body.stmts = prelude;
    out.push(Stmt::new(
        line,
        StmtKind::ForEach {
            label: None,
            ty: jty,
            name,
            iter,
            body,
        },
    ));
}

fn collection_iterable(v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Map { .. } => Expr::call(v, "entrySet", vec![]),
        Ty::Option(inner) => {
            // `None` lowers to an untyped `Optional.empty()`.
            let untyped = matches!(&v, Expr::Raw(s) if s == "Optional.empty()");
            let v = if untyped {
                Expr::Cast(
                    format!("Optional<{}>", inner.boxed()),
                    Box::new(Expr::Cast("Optional".into(), Box::new(v))),
                )
            } else {
                v
            };
            Expr::call(Expr::call(v, "stream", vec![]), "toList", vec![])
        }
        Ty::Result(..) => Expr::call(Expr::call(Expr::call(v, "ok", vec![]), "stream", vec![]), "toList", vec![]),
        _ => v,
    }
}

fn simple_ident(p: &ast::Pat) -> Option<String> {
    match p {
        ast::Pat::IdentPat(ip) if ip.pat().is_none() => ip.name().map(|n| n.text().to_string()),
        ast::Pat::RefPat(r) => r.pat().and_then(|p| simple_ident(&p)),
        _ => None,
    }
}

pub(crate) fn method_name(m: &ast::MethodCallExpr) -> String {
    m.name_ref()
        .map(|n| n.text().to_string())
        .unwrap_or_default()
}

fn range_bounds(cx: &mut Cx, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> (Expr, Expr, bool) {
    use ast::RangeItem;
    let start = r
        .start()
        .map(|s| expr::lower_expr(cx, &s, out))
        .unwrap_or(Expr::raw("0"));
    let end = r
        .end()
        .map(|e| expr::lower_expr(cx, &e, out))
        .unwrap_or(Expr::raw("Integer.MAX_VALUE"));
    let inclusive = r.op_kind() == Some(ast::RangeOp::Inclusive);
    (start, end, inclusive)
}

fn range_is_long(cx: &Cx, r: &ast::RangeExpr) -> bool {
    matches!(cx.ty(&ast::Expr::RangeExpr(r.clone())), Ty::Iter(ref t) if matches!(**t, Ty::Int(i) if i.is_long()))
}

// A range used as a value: a boxed stream, so it composes like any other iterator.
pub fn lower_range_expr(cx: &mut Cx, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    use ast::RangeItem;
    let class = if range_is_long(cx, r) {
        "LongStream"
    } else {
        "IntStream"
    };
    if r.end().is_none() {
        // `a..` is unbounded.
        let start = r
            .start()
            .map(|s| expr::lower_expr(cx, &s, out))
            .unwrap_or(Expr::raw("0"));
        let i = cx.fresh("i");
        return boxed(Expr::call(
            Expr::raw(class),
            "iterate",
            vec![start, Expr::raw(format!("{i} -> {i} + 1"))],
        ));
    }
    let (s, e, inclusive) = range_bounds(cx, r, out);
    let m = if inclusive { "rangeClosed" } else { "range" };
    boxed(Expr::call(Expr::raw(class), m, vec![s, e]))
}

fn boxed(e: Expr) -> Expr {
    Expr::call(e, "boxed", vec![])
}

// Re-evaluating `e` must be safe and cheap; otherwise bind it to a temporary on this line.
pub(super) fn ensure_simple(cx: &mut Cx, e: Expr, ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    fn plain(e: &Expr) -> bool {
        match e {
            Expr::Name(_) => true,
            Expr::Raw(s) => {
                s.starts_with('"')
                    || s.starts_with('\'')
                    || s.chars()
                        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']'))
            }
            // `this.data`, `o.data`: Rust's borrow keeps the field unchanged meanwhile.
            Expr::Field(base, _) => plain(base),
            _ => false,
        }
    }
    let simple = plain(&e);
    if simple || cx.lambda_depth > 0 {
        return e;
    }
    let tmp = cx.fresh("src");
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: stmt::decl_type(ty),
            name: tmp.clone(),
            init: Some(e),
        },
    ));
    Expr::name(tmp)
}

// ---- iterator `let` bindings ---------------------------------------------------------------

// `let mut it = s.chars().peekable();` used with `next()`/`peek()`: materialize into a list and
// walk it with a ListIterator. Returns false if the binding should stay a plain stream.
pub fn lower_iter_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    ip: &ast::IdentPat,
    rust_name: &str,
    ty: &Ty,
    out: &mut Vec<Stmt>,
) -> bool {
    let Ty::Iter(item) = ty.strip_refs() else {
        return false;
    };
    let Some(init) = l.initializer() else {
        return false;
    };
    let Some(local) = cx.sema.to_def(ip) else {
        return false;
    };
    let Some(scope) = cx.enclosing_fn(ip.syntax()) else {
        return false;
    };
    let pulled = cx
        .expanded_descendants(&scope)
        .into_iter()
        .filter_map(ast::MethodCallExpr::cast)
        .any(|m| {
            matches!(
                method_name(&m).as_str(),
                "next" | "peek" | "next_if" | "next_if_eq"
            ) && m.receiver().and_then(|r| cx.local_of_path(&r)) == Some(local)
        });
    if !pulled {
        return false;
    }
    let line = cx.line(l.syntax());
    let plain = super::std_map::io::plain_iterator(cx, &init, local, &scope);
    let src = expr::lower_expr(cx, &init, out);
    let name = cx.declare(rust_name);
    let items = cx.fresh(&format!("{name}_items"));
    let b = if plain { Ty::Str.boxed() } else { item.boxed() };
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("List<{b}>"),
            name: items.clone(),
            init: Some(Expr::call(src, "toList", vec![])),
        },
    ));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("ListIterator<{b}>"),
            name,
            init: Some(Expr::call(
                Expr::name(items.clone()),
                "listIterator",
                vec![],
            )),
        },
    ));
    cx.pulled_iters.insert(local, items);
    true
}

// ---- iterator methods ------------------------------------------------------------------------

fn arg_exprs(m: &ast::MethodCallExpr) -> Vec<ast::Expr> {
    m.arg_list()
        .map(|al| al.args().collect())
        .unwrap_or_default()
}

fn lower_arg(cx: &mut Cx, a: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    expr::lower_expr(cx, a, out)
}

// Iterator sources, adaptors and consumers. Returns None when the method isn't iterator-related.
pub fn lower_iter_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    // `Result::unwrap`/`Option::map` can hand back an iterator; they belong to std_map.
    if key.starts_with("Option::") || key.starts_with("Result::") {
        return None;
    }
    let name = method_name(m);
    let recv_ast = m.receiver()?;
    let ret_ty = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
    if name == "for_each"
        && let Some(e) = iter_mut_for_each(cx, m, &recv_ast, out)
    {
        return Some(e);
    }
    if name == "for_each"
        && let Some(ast::Expr::ClosureExpr(c)) = arg_exprs(m).first().map(strip_parens)
        && closure_is_loop(cx, &c)
    {
        for_each_loop(cx, m, &recv_ast, &c, out);
        return Some(Expr::raw("null"));
    }
    // A range's own methods read its bounds: `(a..=b).end()` is `b`.
    let range_method = key.starts_with("Range");
    if range_method
        && matches!(name.as_str(), "is_empty" | "start" | "end" | "into_inner")
        && let Some((start, end, inclusive)) = range_parts(cx, &recv_ast)
    {
        let line = cx.line(m.syntax());
        let ty = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
        let bound = |cx: &mut Cx, b: &Option<ast::Expr>, out: &mut Vec<Stmt>| b.as_ref().map(|b| lower_arg(cx, b, out));
        let (s, e) = (bound(cx, &start, out), bound(cx, &end, out));
        return match (name.as_str(), s, e) {
            ("start", Some(s), _) => Some(s),
            ("end", _, Some(e)) => Some(e),
            ("into_inner", Some(s), Some(e)) => Some(expr::tuple_expr(cx, &ty, vec![s, e])),
            ("is_empty", Some(s), Some(e)) => {
                let elem = start.as_ref().map(|a| cx.ty(a)).unwrap_or(Ty::Int(crate::ty::Int::I32));
                let s = ensure_simple(cx, s, &elem, line, out);
                let e = ensure_simple(cx, e, &elem, line, out);
                Some(Expr::Unary("!", Box::new(Expr::bin(s, if inclusive { "<=" } else { "<" }, e))))
            }
            _ => None,
        };
    }
    // `(a..b).contains(&x)`: two comparisons.
    if range_method
        && name == "contains"
        && let Some((r_start, r_end, inclusive)) = range_parts(cx, &recv_ast)
        && let Some(x) = arg_exprs(m).first()
    {
        let line = cx.line(m.syntax());
        let ty = cx.ty(x).strip_refs().clone();
        let xv = lower_arg(cx, x, out);
        let xv = ensure_simple(cx, xv, &ty, line, out);
        let cmp = |a: Expr, op: &'static str, b: Expr| match super::std_map::num::compare_expr(
            &ty,
            a.clone(),
            b.clone(),
        ) {
            Some(c) => Expr::bin(c, op, Expr::raw("0")),
            None => Expr::bin(a, op, b),
        };
        let mut conds = Vec::new();
        if let Some(start) = r_start {
            let s = lower_arg(cx, &start, out);
            conds.push(cmp(xv.clone(), ">=", s));
        }
        if let Some(end) = r_end {
            let e = lower_arg(cx, &end, out);
            conds.push(cmp(xv.clone(), if inclusive { "<=" } else { "<" }, e));
        }
        return Some(
            conds
                .into_iter()
                .reduce(|a, b| Expr::bin(a, "&&", b))
                .unwrap_or_else(|| Expr::raw("true")),
        );
    }
    // `v.chunks_exact(k).remainder()`: the last `len % k` elements.
    // `rchunks_exact`: the first `len % k`.
    if name == "remainder"
        && let ast::Expr::MethodCallExpr(chunks) = strip_parens(&recv_ast)
        && matches!(method_name(&chunks).as_str(), "chunks_exact" | "rchunks_exact")
        && let (Some(v), Some(k)) = (chunks.receiver(), arg_exprs(&chunks).first().cloned())
    {
        let line = cx.line(m.syntax());
        let vt = cx.ty(&v);
        let v = expr::lower_expr(cx, &v, out);
        let v = ensure_simple(cx, v, &vt, line, out);
        let k = lower_arg(cx, &k, out);
        let k = ensure_simple(cx, k, &Ty::Int(Int::Usize), line, out);
        let (vs, ks) = (render(&v), render(&k));
        if method_name(&chunks) == "rchunks_exact" {
            return Some(Expr::raw(format!("{vs}.subList(0, {vs}.size() % {ks})")));
        }
        return Some(Expr::raw(format!(
            "{vs}.subList({vs}.size() - {vs}.size() % {ks}, {vs}.size())"
        )));
    }
    if let Some(local) = cx.local_of_path(&recv_ast)
        && let Some(items) = cx.pulled_iters.get(&local).cloned()
    {
        return Some(lower_pulled(
            cx,
            m,
            &name,
            &recv_ast,
            Some(&items),
            recv_ty,
            out,
        ));
    }
    if is_iter_field(cx, &recv_ast) {
        return Some(lower_pulled(cx, m, &name, &recv_ast, None, recv_ty, out));
    }
    // Set algebra streams: std_map builds the stream from the two sets.
    if matches!(recv_ty.strip_refs(), Ty::Set { .. })
        && matches!(
            name.as_str(),
            "union" | "intersection" | "difference" | "symmetric_difference"
        )
    {
        return None;
    }
    // A user type with `impl Iterator`: stream() then the usual adaptor lowering.
    if matches!(recv_ty.strip_refs(), Ty::Adt { .. })
        && (key.starts_with("Iterator::") || key.starts_with("IntoIterator::"))
        && let Some(item) = traits::iter_item(cx, recv_ty)
    {
        let recv = expr::lower_expr(cx, &recv_ast, out);
        if key == "IntoIterator::into_iter" {
            return Some(recv);
        }
        // `it.by_ref()` already lowered to `it.stream()`.
        let src = match strip_parens(&recv_ast) {
            ast::Expr::MethodCallExpr(r) if method_name(&r) == "by_ref" => recv,
            _ => Expr::call(recv, "stream", vec![]),
        };
        let line = cx.line(m.syntax());
        return Some(lower_adaptor_core(
            cx, m, &name, src, item, &ret_ty, line, out,
        ));
    }
    if !matches!(recv_ty.strip_refs(), Ty::Iter(_)) {
        if let (Ty::Vec(t), "join" | "concat") = (recv_ty.strip_refs(), name.as_str())
            && t.is_str()
        {
            let v = expr::lower_expr(cx, &recv_ast, out);
            let sep = match arg_exprs(m).first() {
                Some(a) => lower_arg(cx, a, out),
                None => Expr::str_lit(""),
            };
            return Some(Expr::call(Expr::raw("String"), "join", vec![sep, v]));
        }
        if !matches!(ret_ty, Ty::Iter(_)) {
            return None;
        }
        return Some(lower_source(cx, m, &name, recv_ty, &recv_ast, out));
    }
    Some(lower_adaptor(
        cx, m, &name, &recv_ast, recv_ty, &ret_ty, out,
    ))
}

fn lower_source(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    recv_ty: &Ty,
    recv_ast: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(m.syntax());
    let mut recv = expr::lower_expr(cx, recv_ast, out);
    let mut recv_ty = recv_ty.clone();
    if let Some(target) = traits::deref_target(cx, &recv_ty) {
        recv = Expr::call(recv, "deref", vec![]);
        recv_ty = target;
    }
    let args = arg_exprs(m);
    match (recv_ty.strip_refs(), name) {
        (t @ (Ty::Map { ordered: true, .. } | Ty::Set { ordered: true, .. }), "range") => {
            let is_map = matches!(t, Ty::Map { .. });
            let Some(view) = ordered_range_view(cx, m, recv, is_map, out) else {
                return cx.unsupported(m.syntax(), "BTreeMap/BTreeSet range bounds");
            };
            if is_map {
                Expr::call(Expr::call(view, "entrySet", vec![]), "stream", vec![])
            } else {
                Expr::call(view, "stream", vec![])
            }
        }
        (Ty::Map { .. }, "iter" | "into_iter" | "iter_mut") => {
            Expr::call(Expr::call(recv, "entrySet", vec![]), "stream", vec![])
        }
        (Ty::Map { .. }, "keys" | "into_keys") => {
            Expr::call(Expr::call(recv, "keySet", vec![]), "stream", vec![])
        }
        (Ty::Map { .. }, "values" | "into_values") => {
            Expr::call(Expr::call(recv, "values", vec![]), "stream", vec![])
        }
        (
            Ty::Vec(_) | Ty::Set { .. } | Ty::Deque(_) | Ty::Heap(_) | Ty::Option(_),
            "iter" | "into_iter",
        ) => Expr::call(recv, "stream", vec![]),
        (Ty::Result(..), "iter") => Expr::call(Expr::call(recv, "ok", vec![]), "stream", vec![]),
        (Ty::Vec(t) | Ty::Deque(t), "iter_mut") if !super::is_cell_value_ty(t) => {
            Expr::call(recv, "stream", vec![])
        }
        (Ty::Vec(t), "drain") => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(m.syntax(), "`drain` inside a closure");
            }
            // Copy the drained part, clear it from the source, then stream the copy.
            let part = match args.first().map(strip_parens) {
                Some(ast::Expr::RangeExpr(r)) if r.syntax().text() != ".." => {
                    let (s, e, incl) = range_bounds(cx, &r, out);
                    let e = if incl {
                        Expr::bin(e, "+", Expr::raw("1"))
                    } else {
                        e
                    };
                    Expr::call(recv.clone(), "subList", vec![s, e])
                }
                _ => recv.clone(),
            };
            let tmp = cx.fresh("drained");
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: format!("List<{}>", t.boxed()),
                    name: tmp.clone(),
                    init: Some(Expr::New {
                        ty: "ArrayList<>".into(),
                        args: vec![part.clone()],
                    }),
                },
            ));
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::call(part, "clear", vec![])),
            ));
            Expr::call(Expr::name(tmp), "stream", vec![])
        }
        // `subList` views write through, so the `_mut` forms are the same.
        (Ty::Vec(_), "chunks_mut" | "chunks_exact_mut") => {
            let name = name.trim_end_matches("_mut");
            let v = ensure_simple(cx, recv.clone(), &recv_ty, line, out);
            let k = lower_arg(cx, &args[0], out);
            let k = ensure_simple(cx, k, &Ty::Int(Int::Usize), line, out);
            let i = cx.fresh("i");
            let (vs, ks) = (render(&v), render(&k));
            let count = if name == "chunks" { format!("({vs}.size() + {ks} - 1) / {ks}") } else { format!("{vs}.size() / {ks}") };
            Expr::raw(format!(
                "IntStream.range(0, {count}).mapToObj({i} -> {vs}.subList({i} * {ks}, Math.min({i} * {ks} + {ks}, {vs}.size())))"
            ))
        }
        (Ty::Vec(_), "array_windows") => {
            let Some(n) = super::std_map::coll::const_arg(m) else {
                return cx.unsupported(m.syntax(), "array_windows without a `::<N>` length");
            };
            let v = ensure_simple(cx, recv.clone(), &recv_ty, line, out);
            let i = cx.fresh("i");
            let vs = render(&v);
            Expr::raw(format!("IntStream.rangeClosed(0, {vs}.size() - {n}).mapToObj({i} -> {vs}.subList({i}, {i} + {n}))"))
        }
        (Ty::Vec(_), "rchunks" | "rchunks_mut" | "rchunks_exact" | "rchunks_exact_mut") => {
            super::fmt::file_helper(cx, super::std_map::coll::SLICE_VIEW_HELPERS);
            let k = lower_arg(cx, &args[0], out);
            let exact = Expr::raw(if name.contains("exact") { "true" } else { "false" });
            Expr::call(Expr::static_call("rchunks", vec![recv, k, exact]), "stream", vec![])
        }
        (Ty::Vec(_), "chunk_by" | "chunk_by_mut") => {
            super::fmt::file_helper(cx, super::std_map::coll::SLICE_VIEW_HELPERS);
            let f = lower_arg(cx, &args[0], out);
            Expr::call(Expr::static_call("chunk_by", vec![recv, f]), "stream", vec![])
        }
        // Splits at the items a predicate picks: `split`, `splitn`, `rsplit`, `rsplitn`,
        // `split_inclusive`, and their `_mut` forms.
        (Ty::Vec(_), "split" | "split_mut" | "splitn" | "splitn_mut" | "rsplit" | "rsplit_mut" | "rsplitn" | "rsplitn_mut"
            | "split_inclusive" | "split_inclusive_mut") => {
            super::fmt::file_helper(cx, super::std_map::coll::SLICE_VIEW_HELPERS);
            let base = name.trim_end_matches("_mut");
            let (n, p) = if base.ends_with('n') {
                (lower_arg(cx, &args[0], out), lower_arg(cx, &args[1], out))
            } else {
                (Expr::raw("-1"), lower_arg(cx, &args[0], out))
            };
            let rev = Expr::raw(if base.starts_with('r') { "true" } else { "false" });
            let inclusive = Expr::raw(if base == "split_inclusive" { "true" } else { "false" });
            Expr::call(Expr::static_call("slice_split", vec![recv, p, n, rev, inclusive]), "stream", vec![])
        }
        // `[u8]::escape_ascii` yields the bytes of the escaped text.
        (Ty::Vec(_), "escape_ascii") => {
            super::fmt::file_helper(cx, super::std_map::coll::BYTES_HELPERS);
            Expr::call(Expr::call(Expr::static_call("esc_ascii", vec![recv]), "chars", vec![]), "boxed", vec![])
        }
        (Ty::Vec(_), "utf8_chunks") => {
            super::fmt::file_helper(cx, super::std_map::coll::BYTES_HELPERS);
            let chunks = Expr::call(Expr::static_call("utf8_chunks", vec![recv]), "stream", vec![]);
            // Simple mode names the pair type: rebuild each entry as that record.
            let parts = [Ty::Str, Ty::Vec(Box::new(Ty::Int(Int::U8)))];
            match crate::tuples::named(&parts) {
                Some(n) => {
                    lambda_params!(cx; e);
                    Expr::call(chunks, "map", vec![Expr::raw(format!("{e} -> new {}({e}.getKey(), {e}.getValue())", n.name))])
                }
                None => chunks,
            }
        }
        (Ty::Vec(_), "windows" | "chunks" | "chunks_exact") => {
            let v = ensure_simple(cx, recv.clone(), &recv_ty, line, out);
            let k = lower_arg(cx, &args[0], out);
            let k = ensure_simple(cx, k, &Ty::Int(Int::Usize), line, out);
            let i = cx.fresh("i");
            let (vs, ks) = (render(&v), render(&k));
            if name == "windows" {
                Expr::raw(format!(
                    "IntStream.rangeClosed(0, {vs}.size() - {ks}).mapToObj({i} -> {vs}.subList({i}, {i} + {ks}))"
                ))
            } else if name == "chunks_exact" {
                Expr::raw(format!(
                    "IntStream.range(0, {vs}.size() / {ks}).mapToObj({i} -> {vs}.subList({i} * {ks}, {i} * {ks} + {ks}))"
                ))
            } else {
                Expr::raw(format!(
                    "IntStream.range(0, ({vs}.size() + {ks} - 1) / {ks}).mapToObj({i} -> {vs}.subList({i} * {ks}, Math.min({vs}.size(), {i} * {ks} + {ks})))"
                ))
            }
        }
        (Ty::Str, "chars") => {
            let c = cx.fresh("c");
            Expr::call(
                Expr::call(recv, "chars", vec![]),
                "mapToObj",
                vec![Expr::raw(format!("{c} -> (char) {c}"))],
            )
        }
        (Ty::Str, "bytes") => boxed(Expr::call(recv, "chars", vec![])),
        // `c.to_uppercase()` iterates the (possibly several) chars of the case mapping.
        (Ty::Char, "to_uppercase" | "to_lowercase") => {
            let c = cx.fresh("c");
            let f = if name == "to_uppercase" {
                "toUpperCase"
            } else {
                "toLowerCase"
            };
            let text = Expr::call(
                Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
                f,
                vec![Expr::raw("Locale.ROOT")],
            );
            Expr::call(
                Expr::call(text, "chars", vec![]),
                "mapToObj",
                vec![Expr::raw(format!("{c} -> (char) {c}"))],
            )
        }
        (Ty::Str, "char_indices") => {
            let s = ensure_simple(cx, recv.clone(), &recv_ty, line, out);
            let i = cx.fresh("i");
            let ss = render(&s);
            Expr::raw(format!(
                "IntStream.range(0, {ss}.length()).mapToObj({i} -> Map.entry({i}, {ss}.charAt({i})))"
            ))
        }
        (Ty::Str, "lines") => Expr::call(recv, "lines", vec![]),
        (Ty::Other(_), "lines" | "bytes") if super::std_map::is_reader_source(&recv_ty) => {
            if name == "bytes" {
                super::std_map::bytes_stream(cx, recv)
            } else {
                let line_mode = super::std_map::io::is_line_stdin(cx, &recv_ast);
                let plain = super::std_map::io::plain_lines(cx, m);
                super::std_map::lines_stream(cx, recv, line_mode, plain)
            }
        }
        (Ty::Str, "split_whitespace" | "split_ascii_whitespace") => {
            let w = cx.fresh("w");
            let parts = Expr::call(
                Expr::call(recv, "strip", vec![]),
                "split",
                vec![Expr::str_lit("\\s+")],
            );
            Expr::call(
                Expr::call(Expr::raw("Arrays"), "stream", vec![parts]),
                "filter",
                vec![Expr::raw(format!("{w} -> !{w}.isEmpty()"))],
            )
        }
        // `escape_*` iterate the chars of the escaped text.
        (Ty::Char | Ty::Str, "escape_debug" | "escape_default" | "escape_unicode") => {
            super::fmt::file_helper(cx, super::std_map::ESC_HELPER);
            let text = if recv_ty.strip_refs() == &Ty::Char {
                Expr::call(Expr::raw("String"), "valueOf", vec![recv])
            } else {
                recv
            };
            let mode = match name { "escape_debug" => "'d'", "escape_default" => "'e'", _ => "'u'" };
            lambda_params!(cx; c);
            let escaped = Expr::static_call("esc", vec![text, Expr::raw(mode)]);
            Expr::call(Expr::call(escaped, "chars", vec![]), "mapToObj", vec![Expr::raw(format!("{c} -> (char) {c}"))])
        }
        (Ty::Str, "encode_utf16") => {
            lambda_params!(cx; c);
            Expr::call(Expr::call(recv, "chars", vec![]), "mapToObj", vec![Expr::raw(format!("{c} -> (short) {c}"))])
        }
        // Matches of a string, char or char predicate: positions, pieces, or the text split there.
        (Ty::Str, "match_indices" | "rmatch_indices" | "rmatches" | "split_inclusive" | "rsplit_terminator") => {
            super::fmt::file_helper(cx, super::std_map::STR_MATCH_HELPERS);
            let pat_ast = args[0].clone();
            let pat_ty = cx.ty(&pat_ast);
            let p = lower_arg(cx, &pat_ast, out);
            let pat = match pat_ty.strip_refs() {
                Ty::Str => p,
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                _ => Expr::Cast("Predicate<Character>".into(), Box::new(p)),
            };
            let s = ensure_simple(cx, recv, &recv_ty, line, out);
            let rev = Expr::raw(if name.starts_with('r') { "true" } else { "false" });
            match name {
                "split_inclusive" => Expr::call(
                    Expr::static_call("str_split", vec![s, pat, Expr::raw("true")]),
                    "stream",
                    vec![],
                ),
                "rsplit_terminator" => {
                    let parts = super::std_map::push_local(
                        cx,
                        line,
                        "List<String>".into(),
                        Some(Expr::static_call("str_split", vec![s, pat, Expr::raw("false")])),
                        out,
                        "parts",
                    );
                    Expr::raw(format!(
                        "({parts}.getLast().isEmpty() ? {parts}.subList(0, {parts}.size() - 1) : {parts}).reversed().stream()"
                    ))
                }
                _ => {
                    lambda_params!(cx; mm = "m");
                    let piece = Expr::raw(format!("{}.substring({mm}[0], {mm}[1])", render(&s)));
                    let item = if name == "rmatches" {
                        piece
                    } else {
                        let item_ty = match cx.ty(&ast::Expr::MethodCallExpr(m.clone())) {
                            Ty::Iter(t) => *t,
                            t => t,
                        };
                        expr::tuple_expr(cx, &item_ty, vec![Expr::raw(format!("{mm}[0]")), piece])
                    };
                    let found = Expr::static_call("str_matches", vec![s, pat, rev]);
                    Expr::call(
                        Expr::call(found, "stream", vec![]),
                        "map",
                        vec![Expr::Lambda { params: vec![mm], body: LambdaBody::Expr(Box::new(item)) }],
                    )
                }
            }
        }
        // Non-overlapping occurrences of a literal pattern, or the chars a predicate accepts.
        (Ty::Str, "matches") => {
            let pat_ast = args[0].clone();
            let pat_ty = cx.ty(&pat_ast);
            let p = lower_arg(cx, &pat_ast, out);
            let pat = match pat_ty.strip_refs() {
                Ty::Str => p,
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                Ty::Fn { .. } => {
                    lambda_params!(cx; c);
                    let chars = Expr::call(
                        Expr::call(recv, "chars", vec![]),
                        "mapToObj",
                        vec![Expr::raw(format!("{c} -> (char) {c}"))],
                    );
                    let kept = Expr::call(chars, "filter", vec![p]);
                    return Expr::call(kept, "map", vec![Expr::raw("String::valueOf")]);
                }
                _ => return cx.unsupported(m.syntax(), "matches() pattern"),
            };
            let quoted = regex_of(cx, literal_pattern(&pat_ast), pat);
            cx.import("java.util.regex.Pattern");
            let matcher = Expr::call(
                Expr::call(Expr::raw("Pattern"), "compile", vec![quoted]),
                "matcher",
                vec![recv],
            );
            lambda_params!(cx; r);
            Expr::call(
                Expr::call(matcher, "results", vec![]),
                "map",
                vec![Expr::raw(format!("{r} -> {r}.group()"))],
            )
        }
        // `rsplit`: the `split` pieces, last first.
        (Ty::Str, "rsplit") => {
            let pat_ast = args[0].clone();
            let pat_ty = cx.ty(&pat_ast);
            let p = lower_arg(cx, &pat_ast, out);
            let pat = match pat_ty.strip_refs() {
                Ty::Str => p,
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                _ => return cx.unsupported(m.syntax(), "rsplit by closure/char set"),
            };
            let quoted = regex_of(cx, literal_pattern(&pat_ast), pat);
            let parts = Expr::call(recv, "split", vec![quoted, Expr::raw("-1")]);
            let list = Expr::call(Expr::raw("Arrays"), "asList", vec![parts]);
            Expr::call(Expr::call(list, "reversed", vec![]), "stream", vec![])
        }
        // `rsplitn(n, sep)`: split the reversed text by the reversed separator, then un-reverse.
        (Ty::Str, "rsplitn") => {
            let limit = lower_arg(cx, &args[0], out);
            let pat_ast = args[1].clone();
            let pat_ty = cx.ty(&pat_ast);
            let p = lower_arg(cx, &pat_ast, out);
            let rev = |e: Expr| {
                Expr::call(
                    Expr::call(
                        Expr::New {
                            ty: "StringBuilder".into(),
                            args: vec![e],
                        },
                        "reverse",
                        vec![],
                    ),
                    "toString",
                    vec![],
                )
            };
            let pat = match pat_ty.strip_refs() {
                Ty::Str => rev(p),
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                _ => return cx.unsupported(m.syntax(), "rsplitn by closure/char set"),
            };
            let reversed = literal_pattern(&pat_ast).map(|s| s.chars().rev().collect());
            let quoted = regex_of(cx, reversed, pat);
            let parts = Expr::call(rev(recv), "split", vec![quoted, limit]);
            let x = cx.fresh_param("part");
            let back = Expr::Lambda {
                params: vec![x.clone()],
                body: LambdaBody::Expr(Box::new(rev(Expr::name(&x)))),
            };
            cx.release_param(&x);
            Expr::call(
                Expr::call(Expr::raw("Arrays"), "stream", vec![parts]),
                "map",
                vec![back],
            )
        }
        // `split_terminator`: `split` without one trailing empty piece.
        (Ty::Str, "split_terminator") if cx.lambda_depth == 0 => {
            let pat_ast = args[0].clone();
            let pat_ty = cx.ty(&pat_ast);
            let p = lower_arg(cx, &pat_ast, out);
            let pat = match pat_ty.strip_refs() {
                Ty::Str => p,
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                _ => return cx.unsupported(m.syntax(), "split_terminator by closure/char set"),
            };
            let quoted = regex_of(cx, literal_pattern(&pat_ast), pat);
            let parts = super::std_map::push_local(
                cx,
                line,
                "String[]".into(),
                Some(Expr::call(recv, "split", vec![quoted, Expr::raw("-1")])),
                out,
                "parts",
            );
            let n = format!("{parts}.length");
            Expr::raw(format!(
                "Arrays.stream({parts}, 0, {parts}[{n} - 1].isEmpty() ? {n} - 1 : {n})"
            ))
        }
        (Ty::Str, "split" | "splitn") => {
            let (limit, pat_ast) = if name == "splitn" {
                (lower_arg(cx, &args[0], out), args[1].clone())
            } else {
                (Expr::raw("-1"), args[0].clone())
            };
            // `&['-', '_']` is a set of separators, i.e. one regex character class.
            if let Some(chars) = char_set(&pat_ast) {
                let class = Expr::str_lit(&char_class(&chars));
                let parts = Expr::call(recv, "split", vec![class, limit]);
                return Expr::call(Expr::raw("Arrays"), "stream", vec![parts]);
            }
            let pat_ty = cx.ty(&pat_ast);
            let by_pred = matches!(strip_parens(&pat_ast), ast::Expr::ClosureExpr(_))
                || matches!(pat_ty.strip_refs(), Ty::Fn { .. });
            let p = lower_arg(cx, &pat_ast, out);
            if by_pred {
                return split_predicate(cx, m, recv, p, limit, line, out);
            }
            let pat = match pat_ty.strip_refs() {
                Ty::Str => p,
                Ty::Char => Expr::call(Expr::raw("String"), "valueOf", vec![p]),
                _ => return cx.unsupported(m.syntax(), "split by closure/char set"),
            };
            let quoted = regex_of(cx, literal_pattern(&pat_ast), pat);
            Expr::call(
                Expr::raw("Arrays"),
                "stream",
                vec![Expr::call(recv, "split", vec![quoted, limit])],
            )
        }
        _ => cx.unsupported(
            m.syntax(),
            &format!("iterator source `{name}` on `{}`", recv_ty.describe()),
        ),
    }
}

// Literal chars of `&['-', '_']` / `['-', '_']` / `&['-', '_'][..]`, if it is one.
fn char_set(e: &ast::Expr) -> Option<Vec<char>> {
    use ast::RangeItem;
    let e = match strip_parens(e) {
        ast::Expr::RefExpr(r) => r.expr()?,
        other => other,
    };
    let array = match strip_parens(&e) {
        ast::Expr::IndexExpr(ix) => match ix.index().map(|i| strip_parens(&i)) {
            Some(ast::Expr::RangeExpr(r)) if r.start().is_none() && r.end().is_none() => {
                ix.base()?
            }
            _ => return None,
        },
        other => other,
    };
    let ast::Expr::ArrayExpr(array) = array else {
        return None;
    };
    let mut chars = Vec::new();
    for item in array.exprs() {
        let ast::Expr::Literal(lit) = strip_parens(&item) else {
            return None;
        };
        let ast::LiteralKind::Char(c) = lit.kind() else {
            return None;
        };
        chars.push(c.value().ok()?);
    }
    (!chars.is_empty()).then_some(chars)
}

// The text of a string or char literal pattern (`","`, `&","`, `' '`).
fn literal_pattern(e: &ast::Expr) -> Option<String> {
    let e = match strip_parens(e) {
        ast::Expr::RefExpr(r) => strip_parens(&r.expr()?),
        other => other,
    };
    let ast::Expr::Literal(lit) = e else {
        return None;
    };
    match lit.kind() {
        ast::LiteralKind::Char(c) => c.value().ok().map(String::from),
        ast::LiteralKind::String(s) => s.value().ok().map(|v| v.to_string()),
        _ => None,
    }
}

// A literal with no regex metacharacters is its own regex; anything else goes through `Pattern.quote`.
fn regex_of(cx: &mut Cx, literal: Option<String>, pat: Expr) -> Expr {
    if let Some(s) = literal
        && !s.is_empty()
        && !s.chars().any(|c| "\\^$.|?*+()[]{}".contains(c))
    {
        return Expr::str_lit(&s);
    }
    cx.import("java.util.regex.Pattern");
    Expr::call(Expr::raw("Pattern"), "quote", vec![pat])
}

// Regex character class matching any of `chars`.
fn char_class(chars: &[char]) -> String {
    let mut text = String::from("[");
    for c in chars {
        match c {
            '\\' | ']' | '^' | '-' => {
                text.push('\\');
                text.push(*c);
            }
            '\n' => text.push_str("\\n"),
            '\r' => text.push_str("\\r"),
            '\t' => text.push_str("\\t"),
            c if (*c as u32) < 0x20 => text.push_str(&format!("\\x{:02x}", *c as u32)),
            c => text.push(*c),
        }
    }
    text.push(']');
    text
}

// `s.split(|c| ...)`: cut the string wherever the predicate holds (no regex for that).
fn split_predicate(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: Expr,
    pred: Expr,
    limit: Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    if cx.lambda_depth > 0 {
        return cx.unsupported(m.syntax(), "split by a predicate inside a closure");
    }
    let s = ensure_simple(cx, recv, &Ty::Str, line, out);
    let parts = cx.fresh("parts");
    let from = cx.fresh("from");
    let i = cx.fresh("i");
    let sv = render(&s);
    let pv = format!("((Predicate<Character>) {})", render(&pred));
    // `splitn` keeps the rest of the string in its last element.
    let cap = if matches!(&limit, Expr::Raw(l) if l == "-1") {
        String::new()
    } else {
        let n = ensure_simple(cx, limit, &Ty::Int(Int::Usize), line, out);
        format!(" && {parts}.size() < {} - 1", render(&n))
    };
    out.push(Stmt::new(
        line,
        StmtKind::Raw(format!(
            "List<String> {parts} = new ArrayList<>(); int {from} = 0;"
        )),
    ));
    out.push(Stmt::new(line, StmtKind::Raw(format!(
        "for (int {i} = 0; {i} < {sv}.length(){cap}; {i}++) if ({pv}.test({sv}.charAt({i}))) {{ {parts}.add({sv}.substring({from}, {i})); {from} = {i} + 1; }}"
    ))));
    out.push(Stmt::new(
        line,
        StmtKind::Raw(format!("{parts}.add({sv}.substring({from}));")),
    ));
    Expr::call(Expr::name(parts), "stream", vec![])
}

// The explicit type a `map` needs when its closure builds different variants (`new Op.Addx(..)`
// and `new Op.Noop()`, `new Ok<>(..)` and `new Err<>(..)`): javac would infer their intersection.
pub(super) fn map_witness(out_item: &Ty, f: &Expr) -> Option<String> {
    let text = render(f);
    let needed = match out_item.strip_refs() {
        Ty::Adt { java, args, .. } if args.is_empty() => text.contains(&format!("new {java}.")),
        Ty::Result(..) => text.contains("new Ok<") || text.contains("new Err<"),
        _ => false,
    };
    (needed && out_item.is_nameable()).then(|| out_item.strip_refs().boxed())
}

fn item_of(t: &Ty) -> Ty {
    match t.strip_refs() {
        Ty::Iter(i) => (**i).clone(),
        _ => Ty::Other("?".into()),
    }
}

// The string in `s.chars().rev()`, if `e` is exactly that.
fn chars_rev_of(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let ast::Expr::MethodCallExpr(rev) = strip_parens(e) else {
        return None;
    };
    if method_name(&rev) != "rev" {
        return None;
    }
    let ast::Expr::MethodCallExpr(chars) = strip_parens(&rev.receiver()?) else {
        return None;
    };
    if method_name(&chars) != "chars" {
        return None;
    }
    let s = chars.receiver()?;
    cx.ty(&s).is_str().then_some(s)
}

fn lower_adaptor(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    recv_ast: &ast::Expr,
    recv_ty: &Ty,
    ret_ty: &Ty,
    out: &mut Vec<Stmt>,
) -> Expr {
    use ast::RangeItem;
    let line = cx.line(m.syntax());
    let args = arg_exprs(m);
    let item = item_of(recv_ty);

    // `a.iter().zip(&b).map(|(x, y)| ..)`: an index stream whose closure reads the lists.
    if let Some(e) = super::indexed::fused(cx, m, name, recv_ast, out) {
        return e;
    }

    // `s.chars().rev().collect::<String>()`: reverse the string itself.
    if name == "collect"
        && matches!(ret_ty.strip_refs(), Ty::Str)
        && let Some(s) = chars_rev_of(cx, recv_ast)
    {
        let s = expr::lower_expr(cx, &s, out);
        let reversed = Expr::call(
            Expr::New {
                ty: "StringBuilder".into(),
                args: vec![s],
            },
            "reverse",
            vec![],
        );
        return Expr::call(reversed, "toString", vec![]);
    }

    // Range specials, before the receiver is turned into a stream.
    if let ast::Expr::RangeExpr(r) = strip_parens(recv_ast)
        && r.end().is_some()
        && matches!(name, "rev" | "step_by")
    {
        let class = if range_is_long(cx, &r) {
            "LongStream"
        } else {
            "IntStream"
        };
        let (s, e, inclusive) = range_bounds(cx, &r, out);
        let i = cx.fresh("i");
        let (ss, es) = (render(&s), render(&e));
        return if name == "rev" {
            let first = if inclusive { es } else { format!("{es} - 1") };
            boxed(Expr::raw(format!(
                "{class}.iterate({first}, {i} -> {i} >= {ss}, {i} -> {i} - 1)"
            )))
        } else {
            let k = render(&lower_arg(cx, &args[0], out));
            let cmp = if inclusive { "<=" } else { "<" };
            boxed(Expr::raw(format!(
                "{class}.iterate({ss}, {i} -> {i} {cmp} {es}, {i} -> {i} + {k})"
            )))
        };
    }
    let direct_list = matches!(strip_parens(recv_ast), ast::Expr::MethodCallExpr(_))
        .then(|| list_receiver_of_iter(cx, recv_ast))
        .flatten();
    // A slice iterator's `nth`/`last`/`skip` are O(1) in Rust; a stream would walk the list.
    // (`skip` only matters when what follows stops early.)
    let short_after_skip = || {
        m.syntax()
            .parent()
            .and_then(ast::MethodCallExpr::cast)
            .is_some_and(|p| {
                matches!(
                    method_name(&p).as_str(),
                    "next" | "nth" | "take" | "find" | "any" | "position" | "take_while"
                )
            })
    };
    if (matches!(name, "nth" | "last") || (name == "skip" && short_after_skip()))
        && let Some(list_ast) = &direct_list
    {
        let v = expr::lower_expr(cx, list_ast, out);
        if name == "last" {
            return Expr::call(
                Expr::call(Expr::call(v, "reversed", vec![]), "stream", vec![]),
                "findFirst",
                vec![],
            );
        }
        let lt = cx.ty(list_ast);
        let v = ensure_simple(cx, v, &lt, line, out);
        let args = arg_exprs(m);
        let i = lower_arg(cx, &args[0], out);
        let size = Expr::call(v.clone(), "size", vec![]);
        if name == "skip" {
            let from = Expr::call(Expr::raw("Math"), "min", vec![i, size.clone()]);
            return Expr::call(Expr::call(v, "subList", vec![from, size]), "stream", vec![]);
        }
        let i = ensure_simple(cx, i, &Ty::Int(crate::ty::Int::Usize), line, out);
        let empty = match ret_ty.strip_refs() {
            Ty::Option(t) if t.is_nameable() => format!("Optional.<{}>empty()", t.boxed()),
            _ => "Optional.empty()".into(),
        };
        return Expr::Ternary(
            Box::new(Expr::bin(i.clone(), "<", size)),
            Box::new(Expr::call(
                Expr::raw("Optional"),
                "of",
                vec![Expr::call(v, "get", vec![i])],
            )),
            Box::new(Expr::raw(empty)),
        );
    }
    // `v.iter().rev()` on a list: reversed view.
    if name == "rev"
        && let Some(list_ast) = &direct_list
    {
        let v = expr::lower_expr(cx, list_ast, out);
        return Expr::call(Expr::call(v, "reversed", vec![]), "stream", vec![]);
    }
    if name == "next_back"
        && let ast::Expr::MethodCallExpr(source_call) = strip_parens(recv_ast)
        && let Some(source) = source_call.receiver()
    {
        let source_name = method_name(&source_call);
        let source_ty = cx.ty(&source);
        let view = match (source_ty.strip_refs(), source_name.as_str()) {
            (Ty::Map { ordered: true, .. }, "iter" | "keys" | "values") => {
                let map = expr::lower_expr(cx, &source, out);
                let map = Expr::call(map, "descendingMap", vec![]);
                Some(Expr::call(
                    map,
                    match source_name.as_str() {
                        "keys" => "keySet",
                        "values" => "values",
                        _ => "entrySet",
                    },
                    vec![],
                ))
            }
            (Ty::Set { ordered: true, .. }, "iter") => {
                let set = expr::lower_expr(cx, &source, out);
                Some(Expr::call(set, "descendingSet", vec![]))
            }
            (t @ (Ty::Map { ordered: true, .. } | Ty::Set { ordered: true, .. }), "range") => {
                let is_map = matches!(t, Ty::Map { .. });
                let recv = expr::lower_expr(cx, &source, out);
                ordered_range_view(cx, &source_call, recv, is_map, out).map(|v| {
                    if is_map {
                        Expr::call(Expr::call(v, "descendingMap", vec![]), "entrySet", vec![])
                    } else {
                        Expr::call(v, "descendingSet", vec![])
                    }
                })
            }
            (Ty::Vec(_), "iter" | "into_iter") => {
                let list = expr::lower_expr(cx, &source, out);
                Some(Expr::call(list, "reversed", vec![]))
            }
            _ => None,
        };
        if let Some(view) = view {
            return Expr::call(Expr::call(view, "stream", vec![]), "findFirst", vec![]);
        }
    }
    // `v.iter().enumerate()` on a list: index stream.
    if name == "enumerate"
        && let Some(list_ast) = &direct_list
    {
        let v = expr::lower_expr(cx, list_ast, out);
        let lt = cx.ty(list_ast);
        let v = ensure_simple(cx, v, &lt, line, out);
        let i = cx.fresh("i");
        let vs = render(&v);
        return Expr::raw(format!(
            "IntStream.range(0, {vs}.size()).mapToObj({i} -> Map.entry({i}, {vs}.get({i})))"
        ));
    }

    let src = expr::lower_expr(cx, recv_ast, out);
    // `lines().flatten()`, `lines().map(|l| l.unwrap())`: the items are already plain Strings.
    if super::std_map::io::plain_adaptor(cx, name, m, recv_ast) {
        return src;
    }
    lower_adaptor_core(cx, m, name, src, item, ret_ty, line, out)
}

// Lexicographic comparison of two iterators; a `null` element comparison (NaN) makes the whole
// comparison undefined, like Rust's `partial_cmp`.
const LEX_CMP: &str = "static <T> Integer lex_cmp(List<T> a, List<T> b, BiFunction<T, T, Integer> c) { \
int n = Math.min(a.size(), b.size()); \
for (int i = 0; i < n; i++) { Integer r = c.apply(a.get(i), b.get(i)); if (r == null || r != 0) return r; } \
return Integer.compare(a.size(), b.size()); }";

pub(crate) const IS_SORTED: &str = "static <T> boolean is_sorted(List<T> v, BiPredicate<T, T> le) { \
for (int i = 1; i < v.size(); i++) if (!le.test(v.get(i - 1), v.get(i))) return false; \
return true; }";

// `is_sorted`'s `<=` on `t`; typed parameters, since an empty `Vec::new()` source is a `List<Object>`.
pub(crate) fn sorted_le(cx: &mut Cx, t: &Ty) -> Expr {
        lambda_params!(cx; x, y);
    let b = t.boxed();
    Expr::raw(match t.strip_refs() {
        t if t.is_primitive() => format!("({b} {x}, {b} {y}) -> {x} <= {y}"),
        t => format!("({b} {x}, {b} {y}) -> {} <= 0", super::std_map::coll::compare_call(cx, t, &x, &y)),
    })
}

// Display of a char iterator (case maps, escapes) or of `escape_ascii`'s ASCII bytes: the text.
pub(crate) fn char_iter_text(cx: &mut Cx, stream: Expr, item: &Ty) -> Expr {
    let to_text = if item.strip_refs() == &Ty::Char {
        "String::valueOf".to_string()
    } else {
        lambda_params!(cx; b);
        format!("{b} -> String.valueOf((char) (int) {b})")
    };
    Expr::call(Expr::call(stream, "map", vec![Expr::raw(to_text)]), "collect", vec![Expr::raw("Collectors.joining()")])
}

// The type a key closure returns (`|p| p.1`).
pub(crate) fn closure_body_ty(cx: &Cx, e: &ast::Expr) -> Option<Ty> {
    match strip_parens(e) {
        ast::Expr::ClosureExpr(c) => c.body().map(|b| cx.ty(&b)),
        _ => None,
    }
}

// `try_fold` stops at the first `None` / `Err` the closure returns.
const TRY_FOLD_OPT: &str = "static <T, B> Optional<B> try_fold_some(Stream<T> s, B init, BiFunction<B, T, Optional<B>> f) { \
B acc = init; \
for (Iterator<T> it = s.iterator(); it.hasNext();) { Optional<B> r = f.apply(acc, it.next()); if (r.isEmpty()) return r; acc = r.get(); } \
return Optional.of(acc); }";

const TRY_FOLD_RES: &str = "static <T, B, E> Result<B, E> try_fold_ok(Stream<T> s, B init, BiFunction<B, T, Result<B, E>> f) { \
B acc = init; \
for (Iterator<T> it = s.iterator(); it.hasNext();) { Result<B, E> r = f.apply(acc, it.next()); if (r instanceof Err) return r; acc = r.unwrap(); } \
return new Ok<>(acc); }";

const TRY_FOR_EACH: &str = "static <T, E> Result<Void, E> try_for_each(Stream<T> s, Function<T, Result<Void, E>> f) { \
for (Iterator<T> it = s.iterator(); it.hasNext();) { Result<Void, E> r = f.apply(it.next()); if (r instanceof Err) return r; } \
return new Ok<>(null); }";

// `(x, y) -> ..` comparing two items as `partial_cmp` would: an Integer, or null for NaN.
fn partial_cmp_fn(cx: &mut Cx, item: &Ty) -> String {
        lambda_params!(cx; x, y);
    let body = match item.strip_refs() {
        Ty::F64 => format!("{x}.isNaN() || {y}.isNaN() ? null : Double.compare({x}, {y})"),
        Ty::F32 => format!("{x}.isNaN() || {y}.isNaN() ? null : Float.compare({x}, {y})"),
        t => super::std_map::coll::compare_call(cx, t, &x, &y),
    };
    format!("({x}, {y}) -> {body}")
}

// Rust's `size_hint()` of an iterator chain, from its source size and each adapter's rule, as Java
// int expressions; `None` for chains whose hint depends on the items (`flat_map`).
fn size_hint(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Option<(String, Option<String>)> {
    let e = strip_parens(e);
    // A collection passed to `zip` / `chain` iterates all its items.
    if matches!(cx.ty(&e).strip_refs(), Ty::Vec(_) | Ty::Deque(_) | Ty::Map { .. } | Ty::Set { .. }) {
        let n = format!("{}.size()", render(&expr::lower_expr(cx, &e, out)));
        return Some((n.clone(), Some(n)));
    }
    if let ast::Expr::RangeExpr(r) = &e {
        let (a, b, inclusive) = range_bounds(cx, r, out);
        let (a, b) = (render(&a), render(&b));
        let n = if inclusive { format!("Math.max(0, {b} - {a} + 1)") } else { format!("Math.max(0, {b} - {a})") };
        return Some((n.clone(), Some(n)));
    }
    let ast::Expr::MethodCallExpr(m) = &e else { return None };
    let name = method_name(m);
    let recv = m.receiver()?;
    let arg = |cx: &mut Cx, out: &mut Vec<Stmt>| -> Option<String> {
        let a = m.arg_list()?.args().next()?;
        Some(render(&expr::lower_expr(cx, &a, out)))
    };
    let rty = cx.ty(&recv);
    let collection = matches!(
        rty.strip_refs(),
        Ty::Vec(_) | Ty::Deque(_) | Ty::Map { .. } | Ty::Set { .. } | Ty::Heap(_)
    );
    match name.as_str() {
        "iter" | "iter_mut" | "into_iter" | "keys" | "values" | "values_mut" if collection => {
            let n = format!("{}.size()", render(&expr::lower_expr(cx, &recv, out)));
            Some((n.clone(), Some(n)))
        }
        "chars" | "char_indices" | "bytes" if rty.is_str() => {
            cx.import("java.nio.charset.StandardCharsets");
            let b = format!("{}.getBytes(StandardCharsets.UTF_8).length", render(&expr::lower_expr(cx, &recv, out)));
            if name == "bytes" {
                Some((b.clone(), Some(b)))
            } else {
                Some((format!("({b} + 3) / 4"), Some(b)))
            }
        }
        "map" | "inspect" | "rev" | "copied" | "cloned" | "enumerate" | "fuse" | "by_ref" | "peekable" => {
            size_hint(cx, &recv, out)
        }
        "filter" | "filter_map" | "take_while" | "skip_while" | "map_while" | "scan" => {
            let (_, hi) = size_hint(cx, &recv, out)?;
            Some(("0".into(), hi))
        }
        "skip" => {
            let (lo, hi) = size_hint(cx, &recv, out)?;
            let k = arg(cx, out)?;
            Some((format!("Math.max(0, {lo} - {k})"), hi.map(|h| format!("Math.max(0, {h} - {k})"))))
        }
        "take" => {
            let (lo, hi) = size_hint(cx, &recv, out)?;
            let k = arg(cx, out)?;
            let hi = match hi {
                Some(h) => format!("Math.min({h}, {k})"),
                None => k.clone(),
            };
            Some((format!("Math.min({lo}, {k})"), Some(hi)))
        }
        "step_by" => {
            let (lo, hi) = size_hint(cx, &recv, out)?;
            let s = arg(cx, out)?;
            let step = |n: String| format!("({n} == 0 ? 0 : 1 + ({n} - 1) / {s})");
            Some((step(lo), hi.map(step)))
        }
        "chain" | "zip" => {
            let (lo1, hi1) = size_hint(cx, &recv, out)?;
            let other = m.arg_list()?.args().next()?;
            let (lo2, hi2) = size_hint(cx, &other, out)?;
            if name == "chain" {
                let hi = hi1.zip(hi2).map(|(a, b)| format!("{a} + {b}"));
                return Some((format!("{lo1} + {lo2}"), hi));
            }
            let hi = match (hi1, hi2) {
                (Some(a), Some(b)) => Some(format!("Math.min({a}, {b})")),
                (a, b) => a.or(b),
            };
            Some((format!("Math.min({lo1}, {lo2})"), hi))
        }
        _ => None,
    }
}

fn lower_adaptor_core(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    src: Expr,
    item: Ty,
    ret_ty: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let args = arg_exprs(m);
    let call = |name: &str, args: Vec<Expr>| Expr::call(src.clone(), name, args);
    match name {
        "map" | "filter" | "for_each" | "inspect" | "take_while" | "skip_while" | "any" | "all" => {
            let f = lower_arg(cx, &args[0], out);
            let jname = match name {
                "for_each" => "forEach",
                "inspect" => "peek",
                "take_while" => "takeWhile",
                "skip_while" => "dropWhile",
                "any" => "anyMatch",
                "all" => "allMatch",
                n => n,
            };
            // `|c| c.to_vec()` makes a Stream<ArrayList<T>>, which won't collect into List<List<T>>.
            if name == "map"
                && let Expr::Lambda {
                    body: LambdaBody::Expr(body),
                    ..
                } = &f
                && (matches!(&**body, Expr::New { ty, .. } if ty.starts_with("ArrayList") || ty.starts_with("Hash") || ty.starts_with("Tree") || ty.starts_with("ArrayDeque"))
                    || render(body).ends_with("::new))"))
                && let Ty::Iter(out_item) = ret_ty.strip_refs()
                && out_item.is_nameable()
            {
                return call(&format!("<{}>map", out_item.boxed()), vec![f]);
            }
            if name == "map"
                && let Ty::Iter(out_item) = ret_ty.strip_refs()
                && let Some(w) = map_witness(out_item, &f)
            {
                return call(&format!("<{w}>map"), vec![f]);
            }
            call(jname, vec![f])
        }
        "filter_map" => {
            let f = lower_arg(cx, &args[0], out);
            Expr::call(
                call("map", vec![f]),
                "flatMap",
                vec![Expr::raw("Optional::stream")],
            )
        }
        "map_while" => {
            let f = lower_arg(cx, &args[0], out);
            let mapped = Expr::call(
                call("map", vec![f]),
                "takeWhile",
                vec![Expr::raw("Optional::isPresent")],
            );
            Expr::call(mapped, "map", vec![Expr::raw("Optional::get")])
        }
        "flat_map" => {
            let inner_ty = match strip_parens(&args[0]) {
                ast::Expr::ClosureExpr(c) => {
                    c.body().map(|b| cx.ty(&b)).unwrap_or(Ty::Other("?".into()))
                }
                _ => Ty::Other("?".into()),
            };
            let f = lower_arg(cx, &args[0], out);
            match to_stream_fn(cx, &inner_ty) {
                Some(ts) => Expr::call(call("map", vec![f]), "flatMap", vec![ts]),
                None => cx.unsupported(m.syntax(), "flat_map result type"),
            }
        }
        "flatten" => match item.strip_refs() {
            // `vec![Some(1), None]` infers `List<Optional<Object>>`; pin the element type.
            Ty::Option(inner) => {
                let raw = Expr::Cast("Stream".into(), Box::new(src.clone()));
                let src = Expr::Cast(
                    format!("Stream<Optional<{}>>", inner.boxed()),
                    Box::new(raw),
                );
                Expr::call(src, "flatMap", vec![Expr::raw("Optional::stream")])
            }
            // A `Result` yields its `Ok` value and drops the error, like Rust's `flatten`.
            Ty::Result(..) => {
                let r = cx.fresh("r");
                let ok = call("filter", vec![Expr::raw(format!("{r} -> {r}.is_ok()"))]);
                Expr::call(ok, "map", vec![Expr::raw(format!("{r} -> {r}.unwrap()"))])
            }
            _ => match to_stream_fn(cx, &item) {
                Some(ts) => call("flatMap", vec![ts]),
                None => cx.unsupported(m.syntax(), "flatten item type"),
            },
        },
        "enumerate" => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(
                    m.syntax(),
                    "enumerate() on a non-list iterator inside a closure",
                );
            }
            cx.import("java.util.concurrent.atomic.AtomicInteger");
            let idx = cx.fresh("index");
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: "AtomicInteger".into(),
                    name: idx.clone(),
                    init: Some(Expr::New {
                        ty: "AtomicInteger".into(),
                        args: vec![],
                    }),
                },
            ));
            let x = cx.fresh("x");
            call(
                "map",
                vec![Expr::raw(format!(
                    "{x} -> Map.entry({idx}.getAndIncrement(), {x})"
                ))],
            )
        }
        "zip" => lower_zip(cx, m, &args[0], src, &item, line, out),
        "rev" => Expr::call(
            Expr::call(call("toList", vec![]), "reversed", vec![]),
            "stream",
            vec![],
        ),
        "skip" => call("skip", vec![lower_arg(cx, &args[0], out)]),
        "take" => call("limit", vec![lower_arg(cx, &args[0], out)]),
        "step_by" => {
            let k = lower_arg(cx, &args[0], out);
            if cx.lambda_depth > 0 {
                return cx.unsupported(
                    m.syntax(),
                    "step_by() on a non-range iterator inside a closure",
                );
            }
            // A counter keeps it lazy: the source may be endless (`Fib.step_by(2).take(5)`).
            let k = if matches!(&k, Expr::Raw(r) if r.chars().all(|c| c.is_ascii_digit())) {
                k
            } else {
                Expr::name(super::std_map::push_local(
                    cx,
                    line,
                    "int".into(),
                    Some(k),
                    out,
                    "step",
                ))
            };
            let seen = super::std_map::push_local(
                cx,
                line,
                "int[]".into(),
                Some(Expr::raw("{0}")),
                out,
                "seen",
            );
            lambda_params!(cx; x);
            call(
                "filter",
                vec![Expr::raw(format!(
                    "{x} -> {seen}[0]++ % {} == 0",
                    render(&k)
                ))],
            )
        }
        // The state lives in a local (a one-element array for numbers/strings/tuples) that the
        // closure's `&mut` state parameter names; the first `None` ends the iteration.
        "scan" => {
            let ast::Expr::ClosureExpr(closure) = strip_parens(&args[1]) else {
                return cx.unsupported(m.syntax(), "scan without a closure");
            };
            let Some(ast::Pat::IdentPat(st)) = closure
                .param_list()
                .and_then(|pl| pl.params().next())
                .and_then(|p| p.pat())
            else {
                return cx.unsupported(m.syntax(), "scan state pattern");
            };
            let state_ty = cx.ty(&args[0]);
            let init = lower_arg(cx, &args[0], out);
            let cell = super::is_mut_ref_cell_ty(&state_ty);
            let (ty, init) = if cell {
                let elem = state_ty.java();
                let raw = elem.split('<').next().unwrap_or(&elem).to_string();
                (
                    format!("{elem}[]"),
                    Expr::raw(format!("new {raw}[]{{{}}}", render(&init))),
                )
            } else {
                (stmt::decl_type(&state_ty), init)
            };
            let state = super::std_map::push_local(cx, line, ty, Some(init), out, "state");
            if cell {
                cx.mark_cell(&st);
            }
            let Expr::Lambda { params, body } = lower_arg(cx, &args[1], out) else {
                return cx.unsupported(m.syntax(), "scan closure");
            };
            let bind = Stmt::new(
                line,
                StmtKind::Local {
                    ty: "var".into(),
                    name: params[0].clone(),
                    init: Some(Expr::name(state)),
                },
            );
            let mut stmts = vec![bind];
            let close_line = match body {
                crate::jir::LambdaBody::Expr(e) => {
                    stmts.push(Stmt::new(line, StmtKind::Return(Some(*e))));
                    line
                }
                crate::jir::LambdaBody::Block(b) => {
                    stmts.extend(b.stmts);
                    b.close_line
                }
            };
            let step = Expr::Lambda {
                params: params[1..].to_vec(),
                body: crate::jir::LambdaBody::Block(crate::jir::Block { stmts, close_line }),
            };
            let present = Expr::call(
                call("map", vec![step]),
                "takeWhile",
                vec![Expr::raw("Optional::isPresent")],
            );
            Expr::call(present, "map", vec![Expr::raw("Optional::get")])
        }
        // `(Vec, Vec)` is a `Map.Entry` of two lists.
        "partition" => {
            let f = lower_arg(cx, &args[0], out);
            lambda_params!(cx; p = "parts");
            let parts = format!(
                "Collectors.collectingAndThen(Collectors.partitioningBy({}), {p} -> Map.entry({p}.get(true), {p}.get(false)))",
                render(&f)
            );
            call("collect", vec![Expr::raw(parts)])
        }
        "unzip" => {
            lambda_params!(cx; e);
            let side = |get: &str| {
                format!(
                    "Collectors.mapping({e} -> {e}.{get}(), Collectors.toCollection(ArrayList::new))"
                )
            };
            let both = format!(
                "Collectors.teeing({}, {}, Map::entry)",
                side("getKey"),
                side("getValue")
            );
            call("collect", vec![Expr::raw(both)])
        }
        // Element-wise equality of two iterators.
        "eq" | "ne" => {
            let other_ty = cx.ty(&args[0]);
            let other = into_stream(lower_arg(cx, &args[0], out), &other_ty);
            let same = Expr::call(
                call("toList", vec![]),
                "equals",
                vec![Expr::call(other, "toList", vec![])],
            );
            if name == "eq" {
                same
            } else {
                Expr::Unary("!", Box::new(same))
            }
        }
        "chain" => {
            let other_ty = cx.ty(&args[0]);
            let other = lower_arg(cx, &args[0], out);
            Expr::call(
                Expr::raw("Stream"),
                "concat",
                vec![src, into_stream(other, &other_ty)],
            )
        }
        // Lexicographic order against another iterator.
        "cmp" | "partial_cmp" | "lt" | "le" | "gt" | "ge" => {
            let other_ty = cx.ty(&args[0]);
            let other = into_stream(lower_arg(cx, &args[0], out), &other_ty);
            super::fmt::file_helper(cx, LEX_CMP);
            let c = partial_cmp_fn(cx, &item);
            let r = Expr::static_call(
                "lex_cmp",
                vec![call("toList", vec![]), Expr::call(other, "toList", vec![]), Expr::raw(c)],
            );
            match name {
                "cmp" => r,
                "partial_cmp" => Expr::call(Expr::raw("Optional"), "ofNullable", vec![r]),
                _ => {
                    let sym = match name { "lt" => "<", "le" => "<=", "gt" => ">", _ => ">=" };
                    let v = cx.fresh("order");
                    Expr::raw(format!("({} instanceof Integer {v} && {v} {sym} 0)", render(&r)))
                }
            }
        }
        // The items again and again; none at all if there are none.
        "cycle" => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(m.syntax(), "cycle() inside a closure");
            }
            let t = item.boxed();
            let items = super::std_map::push_local(cx, line, format!("List<{t}>"), Some(call("toList", vec![])), out, "items");
            Expr::raw(format!(
                "({items}.isEmpty() ? Stream.<{t}>empty() : Stream.generate(() -> {items}).flatMap(List::stream))"
            ))
        }
        "size_hint" => match m.receiver().and_then(|r| size_hint(cx, &r, out)) {
            Some((lo, hi)) => {
                let hi = match hi {
                    Some(h) => format!("Optional.of({h})"),
                    None => "Optional.<Integer>empty()".into(),
                };
                expr::tuple_expr(cx, ret_ty, vec![Expr::raw(lo), Expr::raw(hi)])
            }
            None => cx.unsupported(m.syntax(), "size_hint() of this iterator"),
        },
        "is_sorted" | "is_sorted_by" | "is_sorted_by_key" => {
            super::fmt::file_helper(cx, IS_SORTED);
            let (items, test) = match name {
                "is_sorted" => (call("toList", vec![]), sorted_le(cx, &item)),
                "is_sorted_by" => (call("toList", vec![]), lower_arg(cx, &args[0], out)),
                _ => {
                    let Some(key) = closure_body_ty(cx, &args[0]) else {
                        return cx.unsupported(m.syntax(), "is_sorted_by_key without a closure");
                    };
                    let le = sorted_le(cx, &key);
                    let f = lower_arg(cx, &args[0], out);
                    let keys = Expr::call(call(&format!("<{}>map", key.boxed()), vec![f]), "toList", vec![]);
                    (keys, le)
                }
            };
            Expr::static_call("is_sorted", vec![items, test])
        }
        "try_fold" => {
            let init = lower_arg(cx, &args[0], out);
            let f = lower_arg(cx, &args[1], out);
            let (helper, java) = match ret_ty.strip_refs() {
                Ty::Option(_) => (TRY_FOLD_OPT, "try_fold_some"),
                Ty::Result(..) => {
                    cx.needs_result = true;
                    (TRY_FOLD_RES, "try_fold_ok")
                }
                _ => return cx.unsupported(m.syntax(), "try_fold() returning this type"),
            };
            super::fmt::file_helper(cx, helper);
            Expr::static_call(java, vec![src.clone(), init, f])
        }
        "try_for_each" => {
            if !matches!(ret_ty.strip_refs(), Ty::Result(..)) {
                return cx.unsupported(m.syntax(), "try_for_each() returning this type");
            }
            cx.needs_result = true;
            super::fmt::file_helper(cx, TRY_FOR_EACH);
            let f = lower_arg(cx, &args[0], out);
            Expr::static_call("try_for_each", vec![src.clone(), f])
        }
        // From the back: the forward method on the reversed items.
        "nth_back" | "rfind" | "rfold" | "try_rfold" => {
            let rev = Expr::call(Expr::call(call("toList", vec![]), "reversed", vec![]), "stream", vec![]);
            let fwd = match name {
                "nth_back" => "nth",
                "rfind" => "find",
                "rfold" => "fold",
                _ => "try_fold",
            };
            lower_adaptor_core(cx, m, fwd, rev, item, ret_ty, line, out)
        }
        "copied" | "cloned" | "into_iter" | "iter" | "by_ref" | "peekable" | "fuse" => src,
        "count" | "len" => Expr::Cast("int".into(), Box::new(call("count", vec![]))),
        // `c.to_uppercase().to_string()`: the Display text of a char iterator.
        "to_string" if matches!(item.strip_refs(), Ty::Char | Ty::Int(Int::U8)) => char_iter_text(cx, src, &item),
        "sum" | "product" => lower_sum_product(cx, m.syntax(), name == "product", src, ret_ty),
        "min" | "max" => {
            if item.is_primitive() || item.is_str() || name == "min" {
                let cmp = super::std_map::coll::ord_comparator(cx, &item)
                    .unwrap_or_else(|| "Comparator.naturalOrder()".into());
                call(name, vec![Expr::raw(cmp)])
            } else {
                // Rust's max returns the last of equal maxima; Java's returns the first.
                let (a, b) = (cx.fresh("a"), cx.fresh("b"));
                let cmp = super::std_map::coll::compare_call(cx, &item, &b, &a);
                call(
                    "reduce",
                    vec![Expr::raw(format!("({a}, {b}) -> {cmp} >= 0 ? {b} : {a}"))],
                )
            }
        }
        "min_by_key" | "max_by_key" => {
            let f = typed_lambda(cx, &args[0], &item, out);
            let order = match strip_parens(&args[0]) {
                ast::Expr::ClosureExpr(c) => c.body().and_then(|b| {
                    let t = cx.ty(&b);
                    super::std_map::coll::key_comparator(cx, &b, &t)
                }),
                _ => None,
            };
            let cmp = match order {
                Some(o) => format!("Comparator.comparing({}, {o})", render(&f)),
                None => format!("Comparator.comparing({})", render(&f)),
            };
            // Simple mode takes Java's `max`, which keeps the first of equal maxima, not the last.
            if name == "min_by_key" || crate::mode::simple() {
                call(if name == "min_by_key" { "min" } else { "max" }, vec![Expr::raw(cmp)])
            } else {
                let (a, b) = (cx.fresh("a"), cx.fresh("b"));
                call(
                    "reduce",
                    vec![Expr::raw(format!(
                        "({a}, {b}) -> ({cmp}).compare({b}, {a}) >= 0 ? {b} : {a}"
                    ))],
                )
            }
        }
        "min_by" | "max_by" => {
            let f = lower_arg(cx, &args[0], out);
            if name == "min_by" || crate::mode::simple() {
                call(if name == "min_by" { "min" } else { "max" }, vec![f])
            } else {
                let cmp = render(&Expr::Cast(
                    format!("Comparator<{}>", item.boxed()),
                    Box::new(f),
                ));
                let (a, b) = (cx.fresh("a"), cx.fresh("b"));
                call(
                    "reduce",
                    vec![Expr::raw(format!(
                        "({a}, {b}) -> ({cmp}).compare({b}, {a}) >= 0 ? {b} : {a}"
                    ))],
                )
            }
        }
        "fold" => {
            let init_ty = cx.ty(&args[0]);
            let init = match expr::lower_expr_as(cx, &args[0], &init_ty, out) {
                // `fold(HashMap::new(), ..)`: `reduce` gives the diamond no target type.
                Expr::New { ty, args } if ty.ends_with("<>") && init_ty.is_nameable() => {
                    let java = init_ty.java();
                    let targs = java.find('<').map(|i| &java[i..]).unwrap_or("<>");
                    Expr::New {
                        ty: format!("{}{targs}", ty.trim_end_matches("<>")),
                        args,
                    }
                }
                e => e,
            };
            let f = lower_arg(cx, &args[1], out);
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            // The combiner is never called on a sequential stream.
            call(
                "reduce",
                vec![init, f, Expr::raw(format!("({a}, {b}) -> {a}"))],
            )
        }
        "find" => Expr::call(
            call("filter", vec![lower_arg(cx, &args[0], out)]),
            "findFirst",
            vec![],
        ),
        "find_map" => {
            let f = lower_arg(cx, &args[0], out);
            let mapped = Expr::call(
                call("map", vec![f]),
                "flatMap",
                vec![Expr::raw("Optional::stream")],
            );
            Expr::call(mapped, "findFirst", vec![])
        }
        "position" | "rposition" => {
            let f = lower_arg(cx, &args[0], out);
            let find = if name == "position" {
                "indexOf"
            } else {
                "lastIndexOf"
            };
            let idx = Expr::call(
                Expr::call(call("map", vec![f]), "toList", vec![]),
                find,
                vec![Expr::raw("true")],
            );
            let i = cx.fresh("i");
            Expr::call(
                Expr::call(Expr::raw("Optional"), "of", vec![idx]),
                "filter",
                vec![Expr::raw(format!("{i} -> {i} >= 0"))],
            )
        }
        "last" => {
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            call("reduce", vec![Expr::raw(format!("({a}, {b}) -> {b}"))])
        }
        // Both return the fold of all items, empty for no items.
        "reduce" => call("reduce", vec![lower_arg(cx, &args[0], out)]),
        "nth" => Expr::call(
            call("skip", vec![lower_arg(cx, &args[0], out)]),
            "findFirst",
            vec![],
        ),
        // `next()` on a temporary iterator: its first element.
        "next" => call("findFirst", vec![]),
        "collect" => lower_collect(cx, m, src, &item, ret_ty, line, out),
        _ => cx.unsupported(m.syntax(), &format!("iterator method `{name}`")),
    }
}

// `node` is the `sum()` call, or `Sum::sum(it)`.
pub(super) fn lower_sum_product(
    cx: &mut Cx,
    node: &ra_ap_syntax::SyntaxNode,
    product: bool,
    src: Expr,
    ret_ty: &Ty,
) -> Expr {
    let call = |name: &str, args: Vec<Expr>| Expr::call(src.clone(), name, args);
    let widened = Ty::Int(crate::ty::Int::I64);
    let ret_ty = if cx.wide.ops.contains(node) {
        &widened
    } else {
        ret_ty
    };
    match ret_ty.strip_refs() {
        Ty::Int(i) if !product => {
            let to = if i.is_long() { "mapToLong" } else { "mapToInt" };
            let x = cx.fresh("x");
            Expr::call(
                call(to, vec![Expr::raw(format!("{x} -> {x}"))]),
                "sum",
                vec![],
            )
        }
        Ty::Int(i) => {
            let one = if i.is_long() { "1L" } else { "1" };
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            call(
                "reduce",
                vec![
                    Expr::raw(one),
                    Expr::raw(format!("({a}, {b}) -> {a} * {b}")),
                ],
            )
        }
        // Plain left fold: DoubleStream.sum() uses compensated summation and can differ.
        Ty::F64 if !product => call("reduce", vec![Expr::raw("0.0"), Expr::raw("Double::sum")]),
        Ty::F32 if !product => call("reduce", vec![Expr::raw("0.0f"), Expr::raw("Float::sum")]),
        t @ (Ty::F64 | Ty::F32) => {
            let one = if *t == Ty::F32 { "1.0f" } else { "1.0" };
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            call(
                "reduce",
                vec![
                    Expr::raw(one),
                    Expr::raw(format!("({a}, {b}) -> {a} * {b}")),
                ],
            )
        }
        // A crate type implements `Sum`/`Product` itself: its static `sum(Stream)` / `product(Stream)`.
        Ty::Adt { java, .. } => Expr::call(
            Expr::raw(java.clone()),
            if product { "product" } else { "sum" },
            vec![src.clone()],
        ),
        _ => cx.unsupported(node, "sum/product of this type"),
    }
}

// Function turning a value of `ty` into a Stream, for flatMap.
fn to_stream_fn(cx: &mut Cx, ty: &Ty) -> Option<Expr> {
    Some(match ty.strip_refs() {
        Ty::Vec(_) | Ty::Set { .. } | Ty::Deque(_) => Expr::raw("Collection::stream"),
        Ty::Option(_) => Expr::raw("Optional::stream"),
        // A Result iterates its Ok value.
        Ty::Result(..) => {
            lambda_params!(cx; r);
            Expr::raw(format!("{r} -> {r}.ok().stream()"))
        }
        Ty::Iter(_) => {
            let s = cx.fresh("s");
            Expr::raw(format!("{s} -> {s}"))
        }
        _ => return None,
    })
}

pub(crate) fn into_stream(v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Iter(_) => v,
        Ty::Map { .. } => Expr::call(Expr::call(v, "entrySet", vec![]), "stream", vec![]),
        _ => Expr::call(v, "stream", vec![]),
    }
}

// Closure argument with explicit parameter types, for contexts where javac can't infer them
// (e.g. `Comparator.comparing`).
fn typed_lambda(cx: &mut Cx, a: &ast::Expr, param: &Ty, out: &mut Vec<Stmt>) -> Expr {
    match lower_arg(cx, a, out) {
        Expr::Lambda { params, body } if params.len() == 1 => Expr::Lambda {
            params: vec![format!("{} {}", param.boxed(), params[0])],
            body,
        },
        other => other,
    }
}

fn lower_zip(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    other_ast: &ast::Expr,
    src: Expr,
    item: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    use ast::RangeItem;
    let other_ty = cx.ty(other_ast);
    // `.zip(a..)`: pair each element with a counter.
    if let ast::Expr::RangeExpr(r) = strip_parens(other_ast)
        && r.end().is_none()
    {
        if cx.lambda_depth > 0 {
            return cx.unsupported(m.syntax(), "zip with an open range inside a closure");
        }
        let start = r
            .start()
            .map(|s| expr::lower_expr(cx, &s, out))
            .unwrap_or(Expr::raw("0"));
        cx.import("java.util.concurrent.atomic.AtomicInteger");
        let idx = cx.fresh("index");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "AtomicInteger".into(),
                name: idx.clone(),
                init: Some(Expr::New {
                    ty: "AtomicInteger".into(),
                    args: vec![start],
                }),
            },
        ));
        let x = cx.fresh("x");
        return Expr::call(
            src,
            "map",
            vec![Expr::raw(format!(
                "{x} -> Map.entry({x}, {idx}.getAndIncrement())"
            ))],
        );
    }
    if cx.lambda_depth > 0 {
        return cx.unsupported(m.syntax(), "zip inside a closure");
    }
    let other = lower_arg(cx, other_ast, out);
    let other_stream = into_stream(other, &other_ty);
    let other_item = match other_ty.strip_refs() {
        Ty::Iter(t) | Ty::Vec(t) => (**t).clone(),
        // A user type implementing `Iterator`.
        t if let Some(item) = traits::iter_item(cx, t) => item,
        _ => Ty::Other("?".into()),
    };
    // Pulled in step through iterators, so an endless side (`fib().zip(..)`) stays lazy.
    let (a, b) = (cx.fresh("zip_a"), cx.fresh("zip_b"));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("Iterator<{}>", item.boxed()),
            name: a.clone(),
            init: Some(Expr::call(src, "iterator", vec![])),
        },
    ));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("Iterator<{}>", other_item.boxed()),
            name: b.clone(),
            init: Some(Expr::call(other_stream, "iterator", vec![])),
        },
    ));
    let i = cx.fresh("i");
    Expr::raw(format!(
        "Stream.iterate(0, {i} -> {a}.hasNext() && {b}.hasNext(), {i} -> {i}).map({i} -> Map.entry({a}.next(), {b}.next()))"
    ))
}

fn lower_collect(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    src: Expr,
    item: &Ty,
    target: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let collect = |c: String| Expr::call(src.clone(), "collect", vec![Expr::raw(c)]);
    match target.strip_refs() {
        Ty::Vec(_) => {
            if collected_read_only(cx, m) {
                Expr::call(src, "toList", vec![])
            } else {
                collect("Collectors.toCollection(ArrayList::new)".into())
            }
        }
        Ty::Str => match item.strip_refs() {
            Ty::Char => Expr::call(
                Expr::call(src, "map", vec![Expr::raw("String::valueOf")]),
                "collect",
                vec![Expr::raw("Collectors.joining()")],
            ),
            _ => collect("Collectors.joining()".into()),
        },
        Ty::Map { ordered, k, .. } => {
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            let new = match ordered
                .then(|| super::std_map::coll::ord_comparator(cx, k))
                .flatten()
            {
                Some(cmp) => format!("() -> new TreeMap<>({cmp})"),
                None if *ordered => "TreeMap::new".into(),
                None => "HashMap::new".into(),
            };
            collect(format!(
                "Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue, ({a}, {b}) -> {b}, {new})"
            ))
        }
        Ty::Set { ordered, t } => {
            let new = match ordered
                .then(|| super::std_map::coll::ord_comparator(cx, t))
                .flatten()
            {
                Some(cmp) => format!("() -> new TreeSet<>({cmp})"),
                None if *ordered => "TreeSet::new".into(),
                None => "HashSet::new".into(),
            };
            collect(format!("Collectors.toCollection({new})"))
        }
        Ty::Deque(_) => collect("Collectors.toCollection(ArrayDeque::new)".into()),
        Ty::Heap(t) => {
            // `Reverse` is erased from the type; a min-heap shows in the source text.
            let reverse = m.syntax().text().to_string().contains("Reverse")
                || m.syntax()
                    .ancestors()
                    .find_map(ast::LetStmt::cast)
                    .and_then(|l| l.ty())
                    .is_some_and(|t| t.syntax().text().to_string().contains("Reverse"));
            let natural = super::std_map::coll::ord_comparator(cx, t);
            let cmp = if reverse {
                natural.unwrap_or_else(|| "Comparator.naturalOrder()".into())
            } else {
                super::std_map::coll::reversed_comparator(natural)
            };
            // Explicit element type: `reverseOrder()` gives the diamond nothing to infer from.
            collect(format!(
                "Collectors.toCollection(() -> new PriorityQueue<{}>({cmp}))",
                t.boxed()
            ))
        }
        Ty::Result(ok, err) if matches!(ok.strip_refs(), Ty::Vec(_)) => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(m.syntax(), "collect into Result inside a closure");
            }
            cx.needs_result = true;
            let Ty::Vec(t) = ok.strip_refs() else {
                unreachable!()
            };
            let (tb, eb) = (t.boxed(), err.boxed());
            let (list, res, it) = (cx.fresh("items"), cx.fresh("collected"), cx.fresh("r"));
            out.push(Stmt::new(line, StmtKind::Raw(format!(
                "List<{tb}> {list} = new ArrayList<>(); Result<List<{tb}>, {eb}> {res} = new Ok<>({list}); \
                 for (Result<{tb}, {eb}> {it} : (Iterable<Result<{tb}, {eb}>>) {}::iterator) {{ \
                 if ({it} instanceof Err<{tb}, {eb}>(var e)) {{ {res} = new Err<>(e); break; }} {list}.add({it}.unwrap()); }}",
                render(&src)
            ))));
            Expr::name(res)
        }
        // `Result<String, E>` from `Result<char | String, E>` items: append until the first Err.
        Ty::Result(ok, err) if ok.is_str() => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(m.syntax(), "collect into Result inside a closure");
            }
            cx.needs_result = true;
            let Ty::Result(piece, _) = item.strip_refs() else {
                return cx.unsupported(m.syntax(), "collect into Result<String> from non-Result items");
            };
            let (tb, eb) = (piece.boxed(), err.boxed());
            let (text, res, it) = (cx.fresh("text"), cx.fresh("collected"), cx.fresh("r"));
            out.push(Stmt::new(line, StmtKind::Raw(format!(
                "StringBuilder {text} = new StringBuilder(); Result<String, {eb}> {res} = null; \
                 for (Result<{tb}, {eb}> {it} : (Iterable<Result<{tb}, {eb}>>) {}::iterator) {{ \
                 if ({it} instanceof Err<{tb}, {eb}>(var e)) {{ {res} = new Err<>(e); break; }} {text}.append({it}.unwrap()); }} \
                 if ({res} == null) {res} = new Ok<>({text}.toString());",
                render(&src)
            ))));
            Expr::name(res)
        }
        Ty::Option(inner) if matches!(inner.strip_refs(), Ty::Vec(_)) => {
            if cx.lambda_depth > 0 {
                return cx.unsupported(m.syntax(), "collect into Option inside a closure");
            }
            let Ty::Vec(t) = inner.strip_refs() else {
                unreachable!()
            };
            let tb = t.boxed();
            let (list, res, it) = (cx.fresh("items"), cx.fresh("collected"), cx.fresh("o"));
            out.push(Stmt::new(line, StmtKind::Raw(format!(
                "List<{tb}> {list} = new ArrayList<>(); Optional<List<{tb}>> {res} = Optional.of({list}); \
                 for (Optional<{tb}> {it} : (Iterable<Optional<{tb}>>) {}::iterator) {{ \
                 if ({it}.isEmpty()) {{ {res} = Optional.empty(); break; }} {list}.add({it}.get()); }}",
                render(&src)
            ))));
            Expr::name(res)
        }
        _ => cx.unsupported(m.syntax(), &format!("collect into `{}`", target.describe())),
    }
}

// A collect result that is never mutated can be the unmodifiable `toList()`.
fn collected_read_only(cx: &Cx, m: &ast::MethodCallExpr) -> bool {
    let Some(parent) = m.syntax().parent() else {
        return false;
    };
    if let Some(l) = ast::LetStmt::cast(parent.clone()) {
        return matches!(l.pat(), Some(ast::Pat::IdentPat(ip)) if cx.binding_read_only(&ip));
    }
    // A `{:?}` argument is only streamed by the format helper.
    if ast::FormatArgsArg::can_cast(parent.kind()) {
        return true;
    }
    // A temporary receiver is only read by methods that cannot mutate it.
    ast::MethodCallExpr::cast(parent).is_some_and(|p| {
        p.receiver().is_some_and(|r| r.syntax() == m.syntax())
            && matches!(
                method_name(&p).as_str(),
                "join"
                    | "concat"
                    | "len"
                    | "is_empty"
                    | "iter"
                    | "into_iter"
                    | "first"
                    | "last"
                    | "get"
                    | "contains"
            )
    })
}

// `self.chars`: an iterator stored in a struct field, which is a `ListIterator` (see items).
fn is_iter_field(cx: &Cx, e: &ast::Expr) -> bool {
    matches!(strip_parens(e), ast::Expr::FieldExpr(_))
        && matches!(cx.ty(e).strip_refs(), Ty::Iter(_))
}

// `it.next().unwrap()` on a pulled iterator: `ListIterator.next()` already throws when empty.
pub fn is_unwrapped_next(cx: &Cx, m: &ast::MethodCallExpr) -> bool {
    method_name(m) == "next"
        && m.receiver().is_some_and(|r| {
            cx.local_of_path(&r)
                .is_some_and(|l| cx.pulled_iters.contains_key(&l))
                || is_iter_field(cx, &r)
        })
        && m.syntax()
            .parent()
            .and_then(ast::MethodCallExpr::cast)
            .is_some_and(|p| {
                matches!(method_name(&p).as_str(), "unwrap" | "expect")
                    && p.receiver().is_some_and(|r| r.syntax() == m.syntax())
            })
}

// Methods on an iterator materialized as `ListIterator` (`items` is the backing list).
fn lower_pulled(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    recv_ast: &ast::Expr,
    items: Option<&str>,
    recv_ty: &Ty,
    out: &mut Vec<Stmt>,
) -> Expr {
    let it = render(&expr::lower_expr(cx, recv_ast, out));
    let item = item_of(recv_ty);
    // Explicit type arguments: the `?:` has no target type at a call's receiver.
    let t = item.boxed();
    let args = arg_exprs(m);
    // `it.next();` only skips an item: a guarded call, not a discarded Optional.
    if name == "next"
        && m.syntax()
            .parent()
            .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
    {
        let line = cx.line(m.syntax());
        out.push(Stmt::new(
            line,
            StmtKind::Raw(format!("if ({it}.hasNext()) {it}.next();")),
        ));
        return Expr::raw("");
    }
    // A field's ListIterator has no backing list in scope: step forward and back to peek.
    let Some(items) = items else {
        return match name {
            "next" if is_unwrapped_next(cx, m) => Expr::call(Expr::raw(it), "next", vec![]),
            "next" => Expr::raw(format!(
                "({it}.hasNext() ? Optional.<{t}>of({it}.next()) : Optional.<{t}>empty())"
            )),
            "peek" => {
                lambda_params!(cx; x);
                Expr::raw(format!(
                    "({it}.hasNext() ? Optional.<{t}>of({it}.next()).map({x} -> {it}.previous()) : Optional.<{t}>empty())"
                ))
            }
            "by_ref" | "peekable" => Expr::raw(it),
            _ => cx.unsupported(
                m.syntax(),
                &format!("`{name}` on an iterator kept in a struct field"),
            ),
        };
    };
    let peeked = format!("{items}.get({it}.nextIndex())");
    // `it.hasNext() ? Optional.<T>of(v) : Optional.<T>empty()`, which `unwrap_or` and friends
    // can take apart (`option::guarded_some`).
    let guarded = |v: String| {
        Expr::Ternary(
            Box::new(Expr::call(Expr::raw(&it), "hasNext", vec![])),
            Box::new(Expr::call(Expr::raw("Optional"), &format!("<{t}>of"), vec![Expr::raw(v)])),
            Box::new(Expr::raw(format!("Optional.<{t}>empty()"))),
        )
    };
    match name {
        "next" if is_unwrapped_next(cx, m) => Expr::call(Expr::raw(it), "next", vec![]),
        "next" => guarded(format!("{it}.next()")),
        "peek" => guarded(peeked),
        "next_if_eq" => {
            let x = render(&lower_arg(cx, &args[0], out));
            Expr::raw(format!(
                "({it}.hasNext() && {peeked}.equals({x}) ? Optional.<{t}>of({it}.next()) : Optional.<{t}>empty())"
            ))
        }
        "next_if" => {
            let f = lower_arg(cx, &args[0], out);
            let p = render(&Expr::Cast(
                format!("Predicate<{}>", item.boxed()),
                Box::new(f),
            ));
            Expr::raw(format!(
                "({it}.hasNext() && ({p}).test({peeked}) ? Optional.<{t}>of({it}.next()) : Optional.<{t}>empty())"
            ))
        }
        // These take `&mut self` and leave the rest of the iterator usable afterwards.
        "nth" | "any" | "all" | "find" | "find_map" | "position" | "try_fold" | "try_for_each"
        | "by_ref" => cx.unsupported(
            m.syntax(),
            &format!("`{name}` on an iterator that is also pulled with next()"),
        ),
        // `chars.as_str()`: the text not yet pulled.
        "as_str" => Expr::raw(format!(
            "{items}.subList({it}.nextIndex(), {items}.size()).stream().map(String::valueOf).collect(Collectors.joining())"
        )),
        // By-value methods consume the iterator: stream what is left of the backing list.
        _ => {
            let rest = Expr::raw(format!("{items}.subList({it}.nextIndex(), {items}.size())"));
            let src = Expr::call(rest, "stream", vec![]);
            let ret_ty = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
            let line = cx.line(m.syntax());
            lower_adaptor_core(cx, m, name, src, item, &ret_ty, line, out)
        }
    }
}

// `m.range(a..b)` as a NavigableMap/NavigableSet view: subMap/tailMap/headMap (or the Set forms).
fn ordered_range_view(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: Expr,
    is_map: bool,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    use ast::RangeItem;
    let args = arg_exprs(m);
    let Some(ast::Expr::RangeExpr(range)) = args.first().map(strip_parens) else {
        return None;
    };
    let start = range.start().map(|e| lower_arg(cx, &e, out));
    let end = range.end().map(|e| lower_arg(cx, &e, out));
    let inclusive = Expr::raw((range.op_kind() == Some(ast::RangeOp::Inclusive)).to_string());
    let (sub, tail, head) = if is_map {
        ("subMap", "tailMap", "headMap")
    } else {
        ("subSet", "tailSet", "headSet")
    };
    Some(match (start, end) {
        (Some(start), Some(end)) => {
            Expr::call(recv, sub, vec![start, Expr::raw("true"), end, inclusive])
        }
        (Some(start), None) => Expr::call(recv, tail, vec![start, Expr::raw("true")]),
        (None, Some(end)) => Expr::call(recv, head, vec![end, inclusive]),
        (None, None) => recv,
    })
}
