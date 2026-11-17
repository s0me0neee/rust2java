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

// `[a, b, c]` -> mutable list; `[x; n]` -> n independent copies when `x` is a mutable object.
pub fn lower_array_expr(cx: &mut Cx, a: &ast::ArrayExpr, out: &mut Vec<Stmt>) -> Expr {
    let exprs: Vec<ast::Expr> = a.exprs().collect();
    if a.semicolon_token().is_some() && exprs.len() == 2 {
        return lower_repeat(cx, a.syntax(), &exprs[0], &exprs[1], out);
    }
    let items: Vec<Expr> = exprs.iter().map(|e| expr::lower_expr(cx, e, out)).collect();
    if items.is_empty() {
        return Expr::New { ty: "ArrayList<>".into(), args: vec![] };
    }
    let list = Expr::call(Expr::raw("List"), "of", items);
    Expr::New { ty: "ArrayList<>".into(), args: vec![list] }
}

// `[x; n]` / `vec![x; n]`.
pub fn lower_repeat(cx: &mut Cx, node: &ra_ap_syntax::SyntaxNode, elem: &ast::Expr, count: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    {
        let exprs = [elem.clone(), count.clone()];
        let elem_ty = cx.ty(&exprs[0]);
        let n = expr::lower_expr(cx, &exprs[1], out);
        if crate::lower::is_cell_value_ty(&elem_ty) || matches!(elem_ty.strip_refs(), Ty::Option(_)) {
            let x = expr::lower_expr(cx, &exprs[0], out);
            let copies = Expr::call(Expr::raw("Collections"), "nCopies", vec![n, x]);
            return Expr::New { ty: "ArrayList<>".into(), args: vec![copies] };
        }
        // Re-evaluate the element expression per slot: Rust clones it n times.
        let mut inner = Vec::new();
        cx.lambda_depth += 1;
        let x = expr::lower_expr(cx, &exprs[0], &mut inner);
        cx.lambda_depth -= 1;
        if !inner.is_empty() {
            return cx.unsupported(node, "repeat-array element needing statements");
        }
        let i = cx.fresh("i");
        let range = Expr::call(Expr::raw("IntStream"), "range", vec![Expr::raw("0"), n]);
        let mapped = Expr::call(range, "mapToObj", vec![Expr::Lambda { params: vec![i], body: crate::jir::LambdaBody::Expr(Box::new(x)) }]);
        Expr::call(mapped, "collect", vec![Expr::raw("Collectors.toCollection(ArrayList::new)")])
    }
}

// `&v[a..b]`, `&s[a..]`.
pub fn lower_slice_range(cx: &mut Cx, base: Expr, base_ty: &Ty, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    let _ = (base, base_ty, out);
    cx.unsupported(r.syntax(), "slice by range")
}
