// Std mem::{swap, replace, take, drop}, size_of.

use super::*;

// Hook for std free functions (`key` as built by `calls::std_fn_key`, e.g. `Rc::new`).
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, c, key, ret, out);
    None
}

// Hook for std methods, before the receiver is lowered (`key` like `RefCell::borrow`).
pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, m, recv_ty, key, out);
    None
}

// ---- mem::{swap, replace, take} ----

pub(super) fn ref_inner(e: &ast::Expr) -> ast::Expr {
    match e {
        ast::Expr::RefExpr(r) => r.expr().unwrap_or_else(|| e.clone()),
        _ => e.clone(),
    }
}

pub(super) fn mem_swap(cx: &mut Cx, c: &ast::CallExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let asts = arg_asts(c);
    let (Some(a_ast), Some(b_ast)) = (asts.first(), asts.get(1)) else {
        return cx.unsupported(c.syntax(), "mem::swap without two arguments");
    };
    let (a_place, b_place) = (ref_inner(a_ast), ref_inner(b_ast));
    let a_val = expr::lower_expr(cx, &a_place, out);
    let b_val = expr::lower_expr(cx, &b_place, out);
    let ty = cx.ty(&a_place);
    let tmp = push_local(
        cx,
        line,
        super::super::stmt::decl_type(&ty),
        Some(a_val),
        out,
        "swap",
    );
    let to_b = expr::place_assign(cx, &a_place, "=", b_val, out);
    out.push(Stmt::new(line, StmtKind::Expr(to_b)));
    let to_a = expr::place_assign(cx, &b_place, "=", Expr::name(tmp), out);
    out.push(Stmt::new(line, StmtKind::Expr(to_a)));
    Expr::raw("null")
}

pub(super) fn mem_replace(
    cx: &mut Cx,
    c: &ast::CallExpr,
    ret: &Ty,
    take: bool,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(c.syntax());
    let asts = arg_asts(c);
    let Some(a_ast) = asts.first() else {
        return cx.unsupported(c.syntax(), "mem::replace without argument");
    };
    let place_ast = ref_inner(a_ast);
    let ty = cx.ty(&place_ast);
    let old = expr::lower_expr(cx, &place_ast, out);
    let new = if take {
        default_value(&ty)
    } else {
        expr::lower_expr(cx, asts.get(1).unwrap_or(a_ast), out)
    };
    let tmp = push_local(
        cx,
        line,
        super::super::stmt::decl_type(&ty),
        Some(old),
        out,
        "old",
    );
    let assign = expr::place_assign(cx, &place_ast, "=", new, out);
    out.push(Stmt::new(line, StmtKind::Expr(assign)));
    let _ = ret;
    Expr::name(tmp)
}
