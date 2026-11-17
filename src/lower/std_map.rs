// Std library calls, methods, paths and constructors -> Java standard library.

use ra_ap_hir::ModuleDef;
use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

use super::{Cx, expr};
use crate::jir::{Expr, Stmt};
use crate::ty::Ty;

// Free / associated std function calls, keyed like `Vec::new`, `process::exit`.
pub fn lower_std_fn_call(cx: &mut Cx, c: &ast::CallExpr, key: &str, out: &mut Vec<Stmt>) -> Expr {
    let ret = cx.ty(&ast::Expr::CallExpr(c.clone()));
    let mut args = expr::call_args(cx, c, out);
    match key {
        "String::new" => Expr::str_lit(""),
        "String::from" | "ToString::to_string" | "ToOwned::to_owned" => args.remove(0),
        "Vec::new" | "Vec::with_capacity" => Expr::New { ty: "ArrayList<>".into(), args: vec![] },
        "HashMap::new" => Expr::New { ty: "HashMap<>".into(), args: vec![] },
        "process::exit" => Expr::call(Expr::raw("System"), "exit", args),
        _ => {
            let _ = ret;
            cx.unsupported(c.syntax(), &format!("std function `{key}`"))
        }
    }
}

// Std method calls (receiver is a std type or primitive).
pub fn lower_std_method(cx: &mut Cx, m: &ast::MethodCallExpr, recv_ty: &Ty, key: &str, out: &mut Vec<Stmt>) -> Expr {
    let recv_ast = m.receiver().unwrap();
    let recv = expr::lower_expr(cx, &recv_ast, out);
    let args = match m.arg_list() {
        Some(al) => expr::lower_args(cx, al.args(), out),
        None => vec![],
    };
    let name = key.rsplit("::").next().unwrap_or(key);
    match (recv_ty.strip_refs(), name) {
        (Ty::Vec(_), "len") => Expr::call(recv, "size", vec![]),
        (Ty::Vec(_), "push") => Expr::call(recv, "add", args),
        (Ty::Str, "len") => Expr::call(recv, "length", vec![]),
        (_, "clone") if recv_ty.is_primitive() || recv_ty.is_str() => recv,
        (t, "to_string") if t.is_str() => recv,
        (t, "to_string") if t.is_primitive() => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        _ => cx.unsupported(m.syntax(), &format!("std method `{key}` on {recv_ty:?}")),
    }
}

// Paths in value position resolving to std items (`i32::MAX`, `None`, `f64::consts::PI`, ...).
pub fn lower_std_path(cx: &mut Cx, path: &ast::Path, def: ModuleDef) -> Expr {
    let _ = def;
    let text = path.syntax().text().to_string();
    match text.as_str() {
        "None" | "Option::None" => Expr::raw("Optional.empty()"),
        "i32::MAX" => Expr::raw("Integer.MAX_VALUE"),
        "i32::MIN" => Expr::raw("Integer.MIN_VALUE"),
        _ => cx.unsupported(path.syntax(), &format!("std path `{text}`")),
    }
}

// Std tuple-variant constructors: `Some(x)`, `Ok(x)`, `Err(e)`.
pub fn lower_std_ctor(cx: &mut Cx, c: &ast::CallExpr, path: &ast::Path, args: &[Expr]) -> Option<Expr> {
    let last = path.segment()?.name_ref()?.text().to_string();
    let _ = c;
    match last.as_str() {
        "Some" => Some(Expr::call(Expr::raw("Optional"), "of", args.to_vec())),
        "Ok" => {
            cx.needs_result = true;
            Some(Expr::New { ty: "Ok<>".into(), args: args.to_vec() })
        }
        "Err" => {
            cx.needs_result = true;
            Some(Expr::New { ty: "Err<>".into(), args: args.to_vec() })
        }
        _ => None,
    }
}

// `[a, b, c]` and `[x; n]`.
pub fn lower_array_expr(cx: &mut Cx, a: &ast::ArrayExpr, out: &mut Vec<Stmt>) -> Expr {
    let _ = out;
    cx.unsupported(a.syntax(), "array expression")
}

// `&v[a..b]`, `&s[a..]`.
pub fn lower_slice_range(cx: &mut Cx, base: Expr, base_ty: &Ty, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    let _ = (base, base_ty, out);
    cx.unsupported(r.syntax(), "slice by range")
}
