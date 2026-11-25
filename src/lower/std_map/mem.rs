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
    let _ = ret;
    match key {
        "mem::size_of" | "mem::align_of" => {
            let Some(ty) = sizeof_type(cx, c) else {
                return Some(cx.unsupported(c.syntax(), &format!("{key} without a type argument")));
            };
            Some(match prim_size(&ty) {
                Some(n) => Expr::raw(n.to_string()),
                None => cx.unsupported(c.syntax(), &format!("{key} for this type")),
            })
        }
        // `size_of_val(&v)`: an array is its items, a `str` its UTF-8 bytes.
        "mem::size_of_val" | "mem::align_of_val" => {
            let arg = ref_inner(arg_asts(c).first()?);
            let t = cx.sema.type_of_expr(&arg)?.original.strip_references();
            let (elem, len) = match t.as_array(cx.db) {
                Some((e, n)) => (cx.convert(&e), n as u64),
                None => (cx.convert(&t), 1),
            };
            if key == "mem::size_of_val" && elem.is_str() {
                cx.import("java.nio.charset.StandardCharsets");
                let s = expr::lower_expr(cx, &arg, out);
                return Some(Expr::raw(format!("{}.getBytes(StandardCharsets.UTF_8).length", render(&s))));
            }
            let n = prim_size(&elem)?;
            Some(Expr::raw(if key == "mem::size_of_val" { n * len } else { n }.to_string()))
        }
        // Equal for the same variant: a fieldless enum's constant, else the variant's record class.
        "mem::discriminant" => {
            let arg = ref_inner(arg_asts(c).first()?);
            let unit_only = cx
                .sema
                .type_of_expr(&arg)
                .and_then(|t| t.original.strip_references().as_adt())
                .and_then(|a| match a {
                    ra_ap_hir::Adt::Enum(e) => Some(e),
                    _ => None,
                })
                .is_some_and(|e| e.variants(cx.db).iter().all(|v| v.kind(cx.db) == ra_ap_hir::StructKind::Unit));
            let v = expr::lower_expr(cx, &arg, out);
            Some(if unit_only { v } else { Expr::call(v, "getClass", vec![]) })
        }
        _ => None,
    }
}

// The primitive sizes (and alignments, the same for these) Rust guarantees on 64-bit targets.
fn prim_size(ty: &Ty) -> Option<u64> {
    Some(match ty {
        // A reference is a plain 64-bit pointer on the supported targets.
        Ty::Ref { .. } => 8,
        Ty::Unit | Ty::Never => 0,
        Ty::Bool | Ty::Int(Int::I8) | Ty::Int(Int::U8) => 1,
        Ty::Int(Int::I16) | Ty::Int(Int::U16) => 2,
        Ty::Char | Ty::F32 | Ty::Int(Int::I32) | Ty::Int(Int::U32) => 4,
        Ty::F64 | Ty::Int(Int::I64) | Ty::Int(Int::U64) | Ty::Int(Int::Isize) | Ty::Int(Int::Usize) => 8,
        Ty::Int(Int::I128) | Ty::Int(Int::U128) => 16,
        _ => return None,
    })
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

