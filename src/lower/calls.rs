// Function, associated-function, constructor and method calls. Std calls are routed to std_map/iter.

use ra_ap_hir::{AsAssocItem, AssocItemContainer, Function, ModuleDef, PathResolution};
use ra_ap_syntax::{AstNode, ast};

use super::{Cx, escape_keyword, expr, items, iter, std_map};
use crate::frontend::module_class_from;
use crate::jir::{Expr, Stmt};
use crate::ty::{Ty, fn_call_method};

pub fn is_local_fn(cx: &Cx, f: Function) -> bool {
    f.module(cx.db).krate(cx.db).origin(cx.db).is_local()
}

// Stable key for a std function, e.g. `Vec::new`, `process::exit`, `Default::default`.
pub fn std_fn_key(cx: &Cx, f: Function) -> String {
    let db = cx.db;
    let name = f.name(db).as_str().to_string();
    if let Some(assoc) = f.as_assoc_item(db) {
        match assoc.container(db) {
            AssocItemContainer::Impl(imp) => {
                let self_ty = imp.self_ty(db);
                let owner = if let Some(adt) = self_ty.as_adt() {
                    adt.name(db).as_str().to_string()
                } else if let Some(b) = self_ty.as_builtin() {
                    b.name().as_str().to_string()
                } else if self_ty.is_str() || self_ty.strip_references().is_str() {
                    "str".into()
                } else if self_ty.as_slice().is_some() || self_ty.strip_references().as_slice().is_some() {
                    "slice".into()
                } else {
                    "?".into()
                };
                return format!("{owner}::{name}");
            }
            AssocItemContainer::Trait(t) => return format!("{}::{name}", t.name(db).as_str()),
        }
    }
    let module = f.module(db);
    match module.name(db) {
        Some(m) => format!("{}::{name}", m.as_str()),
        None => name,
    }
}

pub fn lower_call(cx: &mut Cx, c: &ast::CallExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(callee) = c.expr() else { return cx.unsupported(c.syntax(), "call without callee") };
    if let ast::Expr::PathExpr(pe) = &callee
        && let Some(path) = pe.path()
    {
        match cx.sema.resolve_path(&path) {
            Some(PathResolution::Def(ModuleDef::Function(f))) => return lower_fn_call(cx, c, f, &path, out),
            Some(PathResolution::Def(ModuleDef::Adt(_))) | Some(PathResolution::Def(ModuleDef::EnumVariant(_))) => {
                let args = expr::call_args(cx, c, out);
                return items::lower_ctor_call(cx, c, &path, args, out);
            }
            _ => {}
        }
    }
    // Calling a closure / fn value.
    let fty = cx.ty(&callee);
    let f = expr::lower_expr(cx, &callee, out);
    let args = expr::call_args(cx, c, out);
    match fty.strip_refs() {
        Ty::Fn { params, ret } => {
            let m = fn_call_method(params.len(), ret.is_unit());
            Expr::call(f, m, args)
        }
        _ => {
            // Fall back on rust-analyzer's callable info (e.g. `impl Fn` params).
            let ret_unit = cx.ty(&ast::Expr::CallExpr(c.clone())).is_unit();
            Expr::call(f, fn_call_method(args.len(), ret_unit), args)
        }
    }
}

fn lower_fn_call(cx: &mut Cx, c: &ast::CallExpr, f: Function, path: &ast::Path, out: &mut Vec<Stmt>) -> Expr {
    if !is_local_fn(cx, f) {
        let key = std_fn_key(cx, f);
        return std_map::lower_std_fn_call(cx, c, &key, out);
    }
    let db = cx.db;
    let name = escape_keyword(f.name(db).as_str());
    let mut args = expr::call_args(cx, c, out);
    if let Some(assoc) = f.as_assoc_item(db) {
        if let AssocItemContainer::Impl(imp) = assoc.container(db)
            && let Some(e) = items::lower_foreign_static_call(cx, imp, &name, args.clone())
        {
            return e;
        }
        if f.self_param(db).is_some() {
            // UFCS `Type::method(x, ...)` -> `x.method(...)`.
            if args.is_empty() {
                return cx.unsupported(c.syntax(), "method call without receiver");
            }
            let recv = args.remove(0);
            return Expr::call(recv, name, args);
        }
        return match assoc.container(db) {
            AssocItemContainer::Impl(imp) => {
                let owner = items::impl_owner_java(cx, imp);
                Expr::call(Expr::raw(owner), name, args)
            }
            AssocItemContainer::Trait(_) => {
                if let Some(e) = items::lower_trait_static_call(cx, path, f, args.clone()) {
                    return e;
                }
                let qualifier = path.qualifier().map(|q| q.syntax().text().to_string()).unwrap_or_default();
                cx.unsupported(c.syntax(), &format!("trait static call `{qualifier}::{name}`"))
            }
        };
    }
    let module = f.module(db);
    // Block-local items have no module name; they live in the enclosing class.
    if module == cx.module || module.name(db).is_none() {
        Expr::static_call(name, args)
    } else {
        Expr::call(Expr::raw(module_class_from(db, cx.module, module)), name, args)
    }
}

pub fn lower_method_call(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(recv_ast) = m.receiver() else { return cx.unsupported(m.syntax(), "method call") };
    let name = m.name_ref().map(|n| n.text().to_string()).unwrap_or_default();
    let target = cx.sema.resolve_method_call(m);
    if let Some(f) = target
        && is_local_fn(cx, f)
    {
        if let Some(e) = items::lower_foreign_impl_call(cx, m, f, out) {
            return e;
        }
        let recv = expr::lower_expr(cx, &recv_ast, out);
        let args = expr::call_args(cx, m, out);
        return Expr::call(recv, escape_keyword(&name), args);
    }
    // A field holding a closure, called as `(self.f)(x)`, is a CallExpr; here it's a real method.
    let recv_ty = cx.ty(&recv_ast);
    let key = target.map(|f| std_fn_key(cx, f)).unwrap_or_else(|| format!("?::{name}"));
    if let Some(e) = iter::lower_iter_method(cx, m, &recv_ty, &key, out) {
        return e;
    }
    std_map::lower_std_method(cx, m, &recv_ty, &key, out)
}

// `*x`: references are transparent in Java; `&mut` cells (one-element arrays) read `[0]`.
pub fn deref_place(cx: &mut Cx, inner: &ast::Expr, v: Expr) -> Expr {
    if super::items::is_cell(cx, inner) {
        return Expr::raw(format!("{}[0]", crate::emit::render_expr(&v)));
    }
    v
}
