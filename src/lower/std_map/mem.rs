// Std mem::{swap, replace, take, drop}, size_of.

use ra_ap_syntax::ast::HasGenericArgs;

use super::*;

// Hook for std free functions (`key` as built by `calls::std_fn_key`, e.g. `Rc::new`).
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (ret, out);
    if key != "mem::size_of" {
        return None;
    }
    let ty = sizeof_type(cx, c)?;
    Some(size_of(cx, c, &ty))
}

// The primitive sizes Rust guarantees; everything else depends on the ABI.
fn size_of(cx: &mut Cx, c: &ast::CallExpr, ty: &Ty) -> Expr {
    let size = match ty.strip_refs() {
        Ty::Unit => Some(0),
        Ty::Bool | Ty::Int(Int::I8) | Ty::Int(Int::U8) => Some(1),
        Ty::Int(Int::I16) | Ty::Int(Int::U16) => Some(2),
        Ty::Char | Ty::F32 | Ty::Int(Int::I32) | Ty::Int(Int::U32) => Some(4),
        Ty::F64
        | Ty::Int(Int::I64)
        | Ty::Int(Int::U64)
        | Ty::Int(Int::Isize)
        | Ty::Int(Int::Usize) => Some(8),
        Ty::Int(Int::I128) | Ty::Int(Int::U128) => Some(16),
        // A reference is a plain 64-bit pointer on the supported targets.
        Ty::Ref { .. } => Some(8),
        _ => None,
    };
    match size {
        Some(n) => Expr::raw(n.to_string()),
        None => cx.unsupported(c.syntax(), "mem::size_of for this type"),
    }
}

fn sizeof_type(cx: &Cx, c: &ast::CallExpr) -> Option<Ty> {
    let ast::Expr::PathExpr(pe) = c.expr()? else {
        return None;
    };
    let seg = pe.path()?.segment()?;
    let args = seg.generic_arg_list()?;
    let t = args.generic_args().find_map(|a| match a {
        ast::GenericArg::TypeArg(ta) => ta.ty(),
        _ => None,
    })?;
    Some(cx.type_ref(&t))
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
        default_value_cx(cx, &ty)
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

// `mem::size_of::<T>()`: Rust's sizes for the primitive (and pointer) types.
pub(super) fn mem_size_of(cx: &mut Cx, c: &ast::CallExpr) -> Expr {
    let ty = c
        .expr()
        .and_then(|e| match e {
            ast::Expr::PathExpr(p) => p.path(),
            _ => None,
        })
        .and_then(|p| p.segment())
        .and_then(|s| s.generic_arg_list())
        .and_then(|list| {
            list.generic_args().find_map(|a| match a {
                ast::GenericArg::TypeArg(t) => t.ty(),
                _ => None,
            })
        })
        .map(|t| cx.type_ref(&t));
    let Some(ty) = ty else {
        return cx.unsupported(c.syntax(), "mem::size_of without a type argument");
    };
    match rust_size(&ty) {
        Some(n) => Expr::raw(format!("{n}")),
        None => cx.unsupported(c.syntax(), "mem::size_of for this type"),
    }
}

fn rust_size(ty: &Ty) -> Option<u64> {
    Some(match ty {
        Ty::Unit => 0,
        Ty::Never => 0,
        Ty::Bool => 1,
        Ty::Char => 4,
        Ty::Int(i) => match i {
            Int::I8 | Int::U8 => 1,
            Int::I16 | Int::U16 => 2,
            Int::I32 | Int::U32 => 4,
            Int::I64 | Int::U64 => 8,
            Int::I128 | Int::U128 => 16,
            Int::Isize | Int::Usize => 8,
        },
        Ty::F32 => 4,
        Ty::F64 => 8,
        Ty::Ref { .. } => 8,
        _ => return None,
    })
}
