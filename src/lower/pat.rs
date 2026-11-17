// Patterns: match, if-let / while-let / let-chains, let-else, destructuring, `?`.

use ra_ap_syntax::{AstNode, ast, ast::HasName};

use super::stmt::Dest;
use super::{Cx, expr};
use crate::jir::{Expr, Stmt, StmtKind};
use crate::ty::Ty;

// Lowers an `if`/`while` condition. Bindings introduced by `let` patterns are pushed to `binds`
// (to be placed at the start of the body); statements needed before the test go to `out`.
pub fn lower_condition(cx: &mut Cx, cond: &ast::Expr, out: &mut Vec<Stmt>, binds: &mut Vec<Stmt>) -> Expr {
    if let ast::Expr::LetExpr(l) = cond {
        return lower_let_condition(cx, l, out, binds);
    }
    if expr::contains_let(cond) {
        let _ = binds;
        return cx.unsupported(cond.syntax(), "let chains");
    }
    expr::lower_expr(cx, cond, out)
}

// `if let Some(x) = e` / `while let Some(&x) = e`: bind a temporary, test it, destructure in `binds`.
fn lower_let_condition(cx: &mut Cx, l: &ast::LetExpr, out: &mut Vec<Stmt>, binds: &mut Vec<Stmt>) -> Expr {
    let (Some(p), Some(src)) = (l.pat(), l.expr()) else {
        return cx.unsupported(l.syntax(), "let condition without pattern/value");
    };
    let ty = cx.ty(&src);
    if !matches!(ty.strip_refs(), Ty::Option(_)) {
        return cx.unsupported(l.syntax(), "`if let` on a non-Option value");
    }
    let line = cx.line(l.syntax());
    let e = expr::lower_expr(cx, &src, out);
    let tmp = cx.fresh("opt");
    out.push(Stmt::new(line, StmtKind::Local { ty: ty.java(), name: tmp.clone(), init: Some(e) }));
    match option_pat_kind(&p) {
        Some(("Some", Some(inner))) => {
            bind_pattern(cx, &inner, Expr::raw(format!("{tmp}.get()")), binds);
            Expr::call(Expr::name(tmp), "isPresent", vec![])
        }
        Some(("Some", None)) => Expr::call(Expr::name(tmp), "isPresent", vec![]),
        Some(("None", _)) => Expr::call(Expr::name(tmp), "isEmpty", vec![]),
        _ => cx.unsupported(p.syntax(), "`if let` pattern"),
    }
}

fn option_pat_kind(p: &ast::Pat) -> Option<(&'static str, Option<ast::Pat>)> {
    match p {
        ast::Pat::TupleStructPat(ts) => {
            let name = ts.path()?.segment()?.name_ref()?.text().to_string();
            let inner = ts.fields().next();
            match name.as_str() {
                "Some" => Some(("Some", inner)),
                _ => None,
            }
        }
        ast::Pat::PathPat(pp) => {
            let name = pp.path()?.segment()?.name_ref()?.text().to_string();
            match name.as_str() {
                "None" => Some(("None", None)),
                _ => None,
            }
        }
        _ => None,
    }
}

pub fn lower_match_stmt(cx: &mut Cx, m: &ast::MatchExpr, dest: &Dest, out: &mut Vec<Stmt>) {
    let _ = dest;
    let line = cx.line(m.syntax());
    let e = cx.unsupported(m.syntax(), "match");
    out.push(Stmt::new(line, StmtKind::Expr(e)));
}

// `match` in value position as a Java switch expression, when every arm allows it.
pub fn try_match_expr(cx: &mut Cx, m: &ast::MatchExpr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let _ = (cx, m, out);
    None
}

pub fn lower_try(cx: &mut Cx, t: &ast::TryExpr, out: &mut Vec<Stmt>) -> Expr {
    // `write!(f, ...)?` writes into the StringBuilder; there is no Result at runtime.
    if cx.in_display
        && let Some(inner) = t.expr()
    {
        return expr::lower_expr(cx, &inner, out);
    }
    cx.unsupported(t.syntax(), "`?` operator")
}

pub fn lower_let_else(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let _ = out;
    cx.unsupported(l.syntax(), "let-else");
}

// `let (a, b) = e;` and other refutable-free destructuring lets.
pub fn lower_destructuring_let(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let (Some(p), Some(init)) = (l.pat(), l.initializer()) else {
        cx.unsupported(l.syntax(), "destructuring let without initializer");
        return;
    };
    let line = cx.line(l.syntax());
    let v = expr::lower_expr(cx, &init, out);
    let src = match v {
        Expr::Name(_) => v,
        other => {
            let tmp = cx.fresh("tmp");
            let ty = cx.ty(&init);
            out.push(Stmt::new(line, StmtKind::Local { ty: super::stmt::decl_type(&ty), name: tmp.clone(), init: Some(other) }));
            Expr::name(tmp)
        }
    };
    bind_pattern(cx, &p, src, out);
}

// Binds an irrefutable pattern against `value`, declaring Java locals.
pub fn bind_pattern(cx: &mut Cx, p: &ast::Pat, value: Expr, out: &mut Vec<Stmt>) {
    let line = cx.line(p.syntax());
    match p {
        ast::Pat::IdentPat(ip) if ip.pat().is_none() => {
            let ty = cx.pat_ty(p);
            let n = ip.name().map(|n| n.text().to_string()).unwrap_or_else(|| "_".into());
            let name = cx.declare(&n);
            out.push(Stmt::new(line, StmtKind::Local { ty: super::stmt::decl_type(&ty), name, init: Some(value) }));
        }
        ast::Pat::WildcardPat(_) => {}
        ast::Pat::TuplePat(tp) => {
            let ty = cx.pat_ty(p);
            let arity = match ty.strip_refs() {
                Ty::Tuple(ts) => ts.len(),
                _ => tp.fields().count(),
            };
            for (i, f) in tp.fields().enumerate() {
                bind_pattern(cx, &f, expr::tuple_field(value.clone(), i, arity), out);
            }
        }
        ast::Pat::RefPat(rp) => {
            if let Some(inner) = rp.pat() {
                bind_pattern(cx, &inner, value, out);
            }
        }
        _ => {
            cx.unsupported(p.syntax(), "pattern kind in binding position");
        }
    }
}
