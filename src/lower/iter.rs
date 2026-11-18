// `for` loops, ranges and iterator chains (-> counted loops / java.util.stream).
// Iterator values are `Stream<T>` (ranges are boxed); iterators pulled with `next()`/`peek()`
// become a `ListIterator` over a materialized list.

use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasLoopBody, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, LoopFrame, expr, strip_parens, traits};
use crate::emit::render_expr as render;
use crate::jir::{Block, Expr, Stmt, StmtKind};
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
        value_var: None,
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
    if let Some(name) = simple_ident(p)
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
        let counter = if captured {
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

    // `for (i, x) in v.iter().enumerate()`: index loop over the list.
    if let ast::Pat::TuplePat(tp) = p
        && tp.fields().count() == 2
        && let Some(list_ast) = enumerate_of_list(cx, iterable)
    {
        let fields: Vec<ast::Pat> = tp.fields().collect();
        let list = expr::lower_expr(cx, &list_ast, out);
        let list_ty = cx.ty(&list_ast);
        let list = ensure_simple(cx, list, &list_ty, line, out);
        let plain_idx = simple_ident(&fields[0])
            .filter(|_| !matches!(&fields[0], ast::Pat::IdentPat(ip) if cx.binding_captured(ip)));
        let idx = match &plain_idx {
            Some(n) => cx.declare(n),
            None => cx.fresh("i"),
        };
        let header = format!("int {idx} = 0; {idx} < {}.size(); {idx}++", render(&list));
        let mut binds = Vec::new();
        if plain_idx.is_none() {
            super::pat::bind_pattern(cx, &fields[0], Expr::name(idx.clone()), &mut binds);
        }
        super::pat::bind_pattern(
            cx,
            &fields[1],
            Expr::call(list, "get", vec![Expr::name(idx)]),
            &mut binds,
        );
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

    let elem_ty = cx.pat_ty(p);
    let it_ty = cx.ty(iterable);
    let iter = for_each_source(cx, iterable, &it_ty, &elem_ty, out);
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

// `v.iter().enumerate()` where `v` is a list: returns `v`.
fn enumerate_of_list(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let ast::Expr::MethodCallExpr(m) = strip_parens(e) else {
        return None;
    };
    if method_name(&m) != "enumerate" {
        return None;
    }
    list_receiver_of_iter(cx, &m.receiver()?)
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

// `v.iter()` / `v.iter_mut()` / `v.into_iter()` / `&v` where v is a list: returns `v`.
fn list_receiver_of_iter(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
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
    let src = expr::lower_expr(cx, iterable, out);
    // A user type with `impl Iterator` iterates through its generated `stream()`.
    if let Some(item) = traits::iter_item(cx, it_ty) {
        let stream = Expr::call(src, "stream", vec![]);
        return Expr::Cast(
            format!("Iterable<{}>", item.boxed()),
            Box::new(Expr::raw(format!("{}::iterator", render(&stream)))),
        );
    }
    match it_ty.strip_refs() {
        Ty::Iter(t) => Expr::Cast(
            format!("Iterable<{}>", t.boxed()),
            Box::new(Expr::raw(format!("{}::iterator", render(&src)))),
        ),
        other => collection_iterable(src, other),
    }
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

fn method_name(m: &ast::MethodCallExpr) -> String {
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
fn ensure_simple(cx: &mut Cx, e: Expr, ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let simple = match &e {
        Expr::Name(_) => true,
        Expr::Raw(s) => {
            s.starts_with('"')
                || s.starts_with('\'')
                || s.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']'))
        }
        _ => false,
    };
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
    let Some(scope) = ip
        .syntax()
        .ancestors()
        .find(|n| ast::Fn::can_cast(n.kind()))
    else {
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
    let src = expr::lower_expr(cx, &init, out);
    let name = cx.declare(rust_name);
    let items = cx.fresh(&format!("{name}_items"));
    let b = item.boxed();
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
    if let Some(local) = cx.local_of_path(&recv_ast)
        && let Some(items) = cx.pulled_iters.get(&local).cloned()
    {
        return Some(lower_pulled(cx, m, &name, &recv_ast, &items, recv_ty, out));
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
        let src = Expr::call(recv, "stream", vec![]);
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
        (Ty::Map { ordered: true, .. }, "range") => {
            use ast::RangeItem;
            let Some(ast::Expr::RangeExpr(range)) = args.first().map(strip_parens) else {
                return cx.unsupported(m.syntax(), "BTreeMap range bounds");
            };
            let start = range.start().map(|e| lower_arg(cx, &e, out));
            let end = range.end().map(|e| lower_arg(cx, &e, out));
            let inclusive = range.op_kind() == Some(ast::RangeOp::Inclusive);
            let bounds = match (start, end) {
                (Some(start), Some(end)) => Expr::call(
                    recv,
                    "subMap",
                    vec![
                        start,
                        Expr::raw("true"),
                        end,
                        Expr::raw(inclusive.to_string()),
                    ],
                ),
                (Some(start), None) => Expr::call(recv, "tailMap", vec![start, Expr::raw("true")]),
                (None, Some(end)) => {
                    Expr::call(recv, "headMap", vec![end, Expr::raw(inclusive.to_string())])
                }
                (None, None) => recv,
            };
            Expr::call(Expr::call(bounds, "entrySet", vec![]), "stream", vec![])
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
        (Ty::Vec(_), "windows" | "chunks") => {
            let v = ensure_simple(cx, recv.clone(), &recv_ty, line, out);
            let k = lower_arg(cx, &args[0], out);
            let k = ensure_simple(cx, k, &Ty::Int(Int::Usize), line, out);
            let i = cx.fresh("i");
            let (vs, ks) = (render(&v), render(&k));
            if name == "windows" {
                Expr::raw(format!(
                    "IntStream.rangeClosed(0, {vs}.size() - {ks}).mapToObj({i} -> {vs}.subList({i}, {i} + {ks}))"
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
                super::std_map::lines_stream(cx, recv)
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
            cx.import("java.util.regex.Pattern");
            let quoted = Expr::call(Expr::raw("Pattern"), "quote", vec![pat]);
            Expr::call(
                Expr::raw("Arrays"),
                "stream",
                vec![Expr::call(recv, "split", vec![quoted, limit])],
            )
        }
        _ => cx.unsupported(
            m.syntax(),
            &format!("iterator source `{name}` on {recv_ty:?}"),
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

fn item_of(t: &Ty) -> Ty {
    match t.strip_refs() {
        Ty::Iter(i) => (**i).clone(),
        _ => Ty::Other("?".into()),
    }
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
    lower_adaptor_core(cx, m, name, src, item, ret_ty, line, out)
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
            let tmp = cx.fresh("stepped");
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: format!("List<{}>", item.boxed()),
                    name: tmp.clone(),
                    init: Some(call("toList", vec![])),
                },
            ));
            let i = cx.fresh("i");
            Expr::raw(format!(
                "IntStream.iterate(0, {i} -> {i} < {tmp}.size(), {i} -> {i} + {}).mapToObj({tmp}::get)",
                render(&k)
            ))
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
        "copied" | "cloned" | "into_iter" | "iter" | "by_ref" | "peekable" | "fuse" => src,
        "count" => Expr::Cast("int".into(), Box::new(call("count", vec![]))),
        "sum" | "product" => lower_sum_product(cx, m, name == "product", src, ret_ty),
        "min" | "max" => {
            if item.is_primitive() || item.is_str() || name == "min" {
                call(name, vec![Expr::raw("Comparator.naturalOrder()")])
            } else {
                // Rust's max returns the last of equal maxima; Java's returns the first.
                let (a, b) = (cx.fresh("a"), cx.fresh("b"));
                call(
                    "reduce",
                    vec![Expr::raw(format!(
                        "({a}, {b}) -> {b}.compareTo({a}) >= 0 ? {b} : {a}"
                    ))],
                )
            }
        }
        "min_by_key" | "max_by_key" => {
            let f = typed_lambda(cx, &args[0], &item, out);
            let cmp = format!("Comparator.comparing({})", render(&f));
            if name == "min_by_key" {
                call("min", vec![Expr::raw(cmp)])
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
            if name == "min_by" {
                call("min", vec![f])
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
            let init = expr::lower_expr_as(cx, &args[0], &init_ty, out);
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
        "position" => {
            let f = lower_arg(cx, &args[0], out);
            let idx = Expr::call(
                Expr::call(call("map", vec![f]), "toList", vec![]),
                "indexOf",
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

fn lower_sum_product(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    product: bool,
    src: Expr,
    ret_ty: &Ty,
) -> Expr {
    let call = |name: &str, args: Vec<Expr>| Expr::call(src.clone(), name, args);
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
        _ => cx.unsupported(m.syntax(), "sum/product of this type"),
    }
}

// Function turning a value of `ty` into a Stream, for flatMap.
fn to_stream_fn(cx: &mut Cx, ty: &Ty) -> Option<Expr> {
    Some(match ty.strip_refs() {
        Ty::Vec(_) | Ty::Set { .. } | Ty::Deque(_) => Expr::raw("Collection::stream"),
        Ty::Option(_) => Expr::raw("Optional::stream"),
        Ty::Iter(_) => {
            let s = cx.fresh("s");
            Expr::raw(format!("{s} -> {s}"))
        }
        _ => return None,
    })
}

fn into_stream(v: Expr, ty: &Ty) -> Expr {
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
        _ => Ty::Other("?".into()),
    };
    let (a, b) = (cx.fresh("zip_a"), cx.fresh("zip_b"));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("List<{}>", item.boxed()),
            name: a.clone(),
            init: Some(Expr::call(src, "toList", vec![])),
        },
    ));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("List<{}>", other_item.boxed()),
            name: b.clone(),
            init: Some(Expr::call(other_stream, "toList", vec![])),
        },
    ));
    let i = cx.fresh("i");
    Expr::raw(format!(
        "IntStream.range(0, Math.min({a}.size(), {b}.size())).mapToObj({i} -> Map.entry({a}.get({i}), {b}.get({i})))"
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
            if collected_into_immutable_let(m) {
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
        Ty::Map { ordered, .. } => {
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            collect(format!(
                "Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue, ({a}, {b}) -> {b}, {}::new)",
                if *ordered { "TreeMap" } else { "HashMap" }
            ))
        }
        Ty::Set { ordered, .. } => collect(format!(
            "Collectors.toCollection({}::new)",
            if *ordered { "TreeSet" } else { "HashSet" }
        )),
        Ty::Deque(_) => collect("Collectors.toCollection(ArrayDeque::new)".into()),
        Ty::Heap(_) => collect(
            "Collectors.toCollection(() -> new PriorityQueue<>(Comparator.reverseOrder()))".into(),
        ),
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
        _ => cx.unsupported(m.syntax(), &format!("collect into {target:?}")),
    }
}

// `let xs: Vec<_> = ....collect();` without `mut`: an unmodifiable `toList()` is fine.
fn collected_into_immutable_let(m: &ast::MethodCallExpr) -> bool {
    let Some(parent) = m.syntax().parent() else {
        return false;
    };
    let Some(l) = ast::LetStmt::cast(parent) else {
        return false;
    };
    matches!(l.pat(), Some(ast::Pat::IdentPat(ip)) if ip.mut_token().is_none())
}

// `it.next().unwrap()` on a pulled iterator: `ListIterator.next()` already throws when empty.
pub fn is_unwrapped_next(cx: &Cx, m: &ast::MethodCallExpr) -> bool {
    method_name(m) == "next"
        && m.receiver()
            .and_then(|r| cx.local_of_path(&r))
            .is_some_and(|l| cx.pulled_iters.contains_key(&l))
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
    items: &str,
    recv_ty: &Ty,
    out: &mut Vec<Stmt>,
) -> Expr {
    let it = render(&expr::lower_expr(cx, recv_ast, out));
    let item = item_of(recv_ty);
    // Explicit type arguments: the `?:` has no target type at a call's receiver.
    let t = item.boxed();
    let peeked = format!("{items}.get({it}.nextIndex())");
    let args = arg_exprs(m);
    match name {
        "next" if is_unwrapped_next(cx, m) => Expr::call(Expr::raw(it), "next", vec![]),
        "next" => Expr::raw(format!(
            "({it}.hasNext() ? Optional.<{t}>of({it}.next()) : Optional.<{t}>empty())"
        )),
        "peek" => Expr::raw(format!(
            "({it}.hasNext() ? Optional.<{t}>of({peeked}) : Optional.<{t}>empty())"
        )),
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
