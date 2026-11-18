// `&str` locals that are always a suffix of one string, kept as (base string, start offset).
// Rust slices are free; Java `substring` copies, so a cursor that keeps slicing its rest would
// be quadratic. A view's `len`/`find`/`starts_with`/sub-slices work on the base with the offset;
// any other use reads `base.substring(start)`.

use ra_ap_syntax::ast::{HasArgList, RangeItem};
use ra_ap_syntax::{AstNode, SyntaxNode, ast};

use super::Cx;
use super::expr::lower_expr;
use crate::jir::{Expr, Stmt, StmtKind};

// (Java base string, Java start offset variable)
pub type View = (String, String);

// Suffix of `base`: `base[from..]`, optionally `.trim_start()`ed. `branch` is an
// `if c { &base[a..] } else { &base[b..] }`, whose offset is `c ? a : b` (`from` unused).
struct Suffix {
    base: ast::Expr,
    from: Option<ast::Expr>,
    trim: bool,
    branch: Option<(ast::Expr, Option<ast::Expr>, Option<ast::Expr>)>,
}

fn block_tail(b: ast::BlockExpr) -> Option<ast::Expr> {
    let list = b.stmt_list()?;
    if list.statements().next().is_some() {
        return None;
    }
    list.tail_expr()
}

// The offset a suffix starts at, relative to its base (`None`: at the start).
fn lower_from(cx: &mut Cx, s: &Suffix, out: &mut Vec<Stmt>) -> Option<Expr> {
    match &s.branch {
        Some((cond, a, b)) => {
            let c = lower_expr(cx, cond, out);
            let mut side = |e: &Option<ast::Expr>| match e {
                Some(e) => lower_expr(cx, e, out),
                None => Expr::raw("0"),
            };
            let (a, b) = (side(a), side(b));
            Some(Expr::Ternary(Box::new(c), Box::new(a), Box::new(b)))
        }
        None => s.from.as_ref().map(|f| lower_expr(cx, f, out)),
    }
}

fn suffix_form(e: &ast::Expr) -> Option<Suffix> {
    match e {
        ast::Expr::RefExpr(r) if r.mut_token().is_none() => suffix_form(&r.expr()?),
        ast::Expr::ParenExpr(p) => suffix_form(&p.expr()?),
        ast::Expr::MethodCallExpr(m)
            if m.name_ref()?.text() == "trim_start"
                && m.arg_list().is_some_and(|a| a.args().next().is_none()) =>
        {
            let recv = m.receiver()?;
            Some(match suffix_form(&recv) {
                Some(s) => Suffix { trim: true, ..s },
                None => Suffix {
                    base: recv,
                    from: None,
                    trim: true,
                    branch: None,
                },
            })
        }
        // `if c { &rest[a..] } else { &rest[b..] }`: both arms must be suffixes of one base.
        ast::Expr::IfExpr(i) => {
            let cond = i.condition()?;
            if matches!(cond, ast::Expr::LetExpr(_)) {
                return None;
            }
            let then = suffix_form(&block_tail(i.then_branch()?)?)?;
            let ast::ElseBranch::Block(else_block) = i.else_branch()? else {
                return None;
            };
            let other = suffix_form(&block_tail(else_block)?)?;
            let same_base =
                then.base.syntax().text().to_string() == other.base.syntax().text().to_string();
            if !same_base
                || then.trim != other.trim
                || then.branch.is_some()
                || other.branch.is_some()
            {
                return None;
            }
            Some(Suffix {
                base: then.base,
                from: None,
                trim: then.trim,
                branch: Some((cond, then.from, other.from)),
            })
        }
        ast::Expr::IndexExpr(ix) => {
            let ast::Expr::RangeExpr(r) = ix.index()? else {
                return None;
            };
            if r.end().is_some() || r.start().is_none() {
                return None;
            }
            Some(Suffix {
                base: ix.base()?,
                from: r.start(),
                trim: false,
                branch: None,
            })
        }
        _ => None,
    }
}

fn same_local(cx: &Cx, e: &ast::Expr, local: ra_ap_hir::Local) -> bool {
    cx.local_of_path(e).is_some_and(|l| l == local)
}

fn in_loop(cx: &Cx, node: &SyntaxNode, scope: &SyntaxNode) -> bool {
    cx.sema
        .ancestors_with_macros(node.clone())
        .take_while(|n| n != scope)
        .any(|n| {
            ast::ForExpr::can_cast(n.kind())
                || ast::WhileExpr::can_cast(n.kind())
                || ast::LoopExpr::can_cast(n.kind())
        })
}

// A view pays off when no use needs the copied rest, or when it is a cursor moved in a loop.
pub fn binding_is_view(cx: &Cx, ip: &ast::IdentPat, init: &ast::Expr) -> bool {
    let is_ref = cx
        .sema
        .type_of_pat(&ast::Pat::IdentPat(ip.clone()))
        .is_some_and(|t| t.original.is_reference());
    if ip.ref_token().is_some() || !is_ref || !cx.pat_ty(&ast::Pat::IdentPat(ip.clone())).is_str() {
        return false;
    }
    let (Some(local), Some(scope)) = (cx.sema.to_def(ip), cx.enclosing_fn(ip.syntax())) else {
        return false;
    };
    // Closures and loops only matter between a use and the binding's own block.
    let Some(home) = ip
        .syntax()
        .ancestors()
        .find(|n| ast::StmtList::can_cast(n.kind()))
    else {
        return false;
    };
    let mut copies = false;
    let mut moved_in_loop = false;
    for pe in cx
        .expanded_descendants(&scope)
        .into_iter()
        .filter_map(ast::PathExpr::cast)
    {
        let expr = ast::Expr::PathExpr(pe.clone());
        if !same_local(cx, &expr, local) {
            continue;
        }
        if cx
            .sema
            .ancestors_with_macros(pe.syntax().clone())
            .take_while(|n| n != &home)
            .any(|n| ast::ClosureExpr::can_cast(n.kind()))
        {
            return false;
        }
        let mut user = pe.syntax().clone();
        while let Some(p) = user.parent().filter(|p| ast::ParenExpr::can_cast(p.kind())) {
            user = p;
        }
        let parent = user.parent();
        if let Some(m) = parent.clone().and_then(ast::MethodCallExpr::cast)
            && m.receiver().is_some_and(|r| r.syntax() == &user)
        {
            let name = m
                .name_ref()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            if !matches!(
                name.as_str(),
                "len" | "is_empty" | "find" | "rfind" | "starts_with"
            ) {
                copies = true;
            }
            continue;
        }
        if let Some(ix) = parent.clone().and_then(ast::IndexExpr::cast)
            && ix.base().is_some_and(|b| b.syntax() == &user)
        {
            if !matches!(ix.index(), Some(ast::Expr::RangeExpr(_))) {
                copies = true;
            }
            continue;
        }
        if let Some(b) = parent.and_then(ast::BinExpr::cast)
            && b.lhs().is_some_and(|l| l.syntax() == &user)
        {
            if !matches!(b.op_kind(), Some(ast::BinaryOp::Assignment { op: None })) {
                return false;
            }
            // Only `x = &x[k..]` / `x = x.trim_start()` keep it a suffix of the same base.
            match b.rhs().as_ref().and_then(suffix_form) {
                Some(s) if same_local(cx, &s.base, local) => {
                    moved_in_loop |= in_loop(cx, b.syntax(), &home);
                }
                _ => return false,
            }
            continue;
        }
        copies = true;
    }
    (suffix_form(init).is_some() && !copies) || (ip.mut_token().is_some() && moved_in_loop)
}

fn skip_whitespace(base: &str, start: &str) -> String {
    format!(
        "while ({start} < {base}.length() && Character.isWhitespace({base}.charAt({start}))) {start}++;"
    )
}

pub fn lower_view_let(
    cx: &mut Cx,
    ip: &ast::IdentPat,
    rust_name: &str,
    init: &ast::Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) {
    let (base_ast, from, trim) = match suffix_form(init) {
        Some(s) => {
            let base = s.base.clone();
            // The base is evaluated first, as in Rust's `base[from..]`.
            let b = lower_expr(cx, &base, out);
            let from = lower_from(cx, &s, out);
            (Some(b), from, s.trim)
        }
        None => (None, None, false),
    };
    let base = match base_ast {
        Some(b) => b,
        None => lower_expr(cx, init, out),
    };
    // A computed base gets the Rust name; the offset is `<name>_start`.
    let base = match base {
        Expr::Name(n) => n,
        e => {
            let name = cx.declare(rust_name);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: "String".into(),
                    name: name.clone(),
                    init: Some(e),
                },
            ));
            name
        }
    };
    let start = cx.fresh(&format!("{rust_name}_start"));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: "int".into(),
            name: start.clone(),
            init: Some(from.unwrap_or_else(|| Expr::raw("0"))),
        },
    ));
    if trim {
        out.push(Stmt::new(
            line,
            StmtKind::Raw(skip_whitespace(&base, &start)),
        ));
    }
    if let Some(local) = cx.sema.to_def(ip) {
        cx.views.insert(local, (base, start));
    }
}

// `x = &x[k..]` / `x = x[k..].trim_start()` / `x = x.trim_start()`: move the offset.
pub fn view_assign(cx: &mut Cx, view: &View, rhs: &ast::Expr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let s = suffix_form(rhs)?;
    let line = cx.line(rhs.syntax());
    let (base, start) = view;
    if let Some(k) = lower_from(cx, &s, out) {
        out.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::Assign(Box::new(Expr::name(start)), "+=", Box::new(k))),
        ));
    }
    if s.trim {
        out.push(Stmt::new(line, StmtKind::Raw(skip_whitespace(base, start))));
    }
    Some(Expr::raw("null"))
}

// `x[a..b]` of a view: one substring of the base.
pub fn view_slice(cx: &mut Cx, view: &View, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    let (base, start) = view;
    let shift = |e: Option<Expr>| match e {
        Some(e) => Expr::bin(Expr::name(start), "+", e),
        None => Expr::name(start),
    };
    let from = shift(r.start().map(|a| lower_expr(cx, &a, out)));
    let inclusive = matches!(r.op_kind(), Some(ast::RangeOp::Inclusive));
    let to = r.end().map(|e| {
        let e = shift(Some(lower_expr(cx, &e, out)));
        if inclusive {
            Expr::bin(e, "+", Expr::raw("1"))
        } else {
            e
        }
    });
    Expr::call(
        Expr::name(base),
        "substring",
        std::iter::once(from).chain(to).collect(),
    )
}

// Any other read of a view: the rest as its own string.
pub fn view_read(view: &View) -> Expr {
    Expr::call(Expr::name(&view.0), "substring", vec![Expr::name(&view.1)])
}
