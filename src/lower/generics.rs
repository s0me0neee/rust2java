// `T::default()` has no Java spelling: no static call on a type variable. A function that needs
// one takes a `Supplier<T>` per type parameter, and its callers pass it down (a `() -> 0` literal
// where the type is concrete), so this pre-pass decides which functions need which suppliers.

use std::collections::HashMap;

use ra_ap_hir::{Function, ModuleDef, PathResolution};
use ra_ap_syntax::{AstNode, SyntaxNode, ast, ast::HasArgList, ast::HasGenericParams};

use super::Cx;
use crate::jir::Expr;
use crate::ty::Ty;

pub fn analyze(cx: &mut Cx, root: &SyntaxNode) {
    let fns: Vec<(Function, ast::Fn)> = root
        .descendants()
        .filter_map(ast::Fn::cast)
        .filter_map(|f| Some((cx.sema.to_def(&f)?, f)))
        .collect();
    // `T::default()` where `T` is the function's own type parameter.
    let mut need: HashMap<Function, Vec<String>> = HashMap::new();
    for (def, f) in &fns {
        let params = type_params(f);
        for name in default_params(&params, f) {
            let slot = need.entry(*def).or_default();
            if !slot.contains(&name) {
                slot.push(name);
            }
        }
    }
    // A function passing a supplier on to another must offer its own type parameter.
    loop {
        let mut changed = false;
        for (def, f) in &fns {
            let mine = type_params(f);
            let Some(body) = f.body() else { continue };
            for (callee, arg_tys) in calls(cx, body.syntax()) {
                let Some(theirs) = need.get(&callee).cloned() else { continue };
                let subst = substitute(cx, callee, &arg_tys);
                for name in theirs {
                    let Some(Ty::Param(to)) = subst.get(&name).map(|t| t.strip_refs().clone()) else {
                        continue;
                    };
                    if !mine.contains(&to) {
                        continue;
                    }
                    let slot = need.entry(*def).or_default();
                    if !slot.contains(&to) {
                        slot.push(to);
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
    cx.supplier_fns = need;
}

// The type parameters of a function, from its own list and its `impl`/`trait` block.
fn type_params(f: &ast::Fn) -> Vec<String> {
    let mut out: Vec<String> = f
        .generic_param_list()
        .into_iter()
        .flat_map(|l| l.type_or_const_params())
        .filter_map(|p| Some(p.name()?.text().to_string()))
        .collect();
    let owner = f.syntax().ancestors().find(|a| {
        ast::Impl::can_cast(a.kind()) || ast::Trait::can_cast(a.kind())
    });
    if let Some(owner) = owner {
        let list = ast::Impl::cast(owner.clone())
            .and_then(|i| i.generic_param_list())
            .or_else(|| ast::Trait::cast(owner).and_then(|t| t.generic_param_list()));
        for p in list.into_iter().flat_map(|l| l.type_or_const_params()) {
            if let Some(n) = p.name() {
                out.push(n.text().to_string());
            }
        }
    }
    out
}

// Which of `params` the body asks `default()` for.
fn default_params(params: &[String], f: &ast::Fn) -> Vec<String> {
    let Some(body) = f.body() else { return Vec::new() };
    let mut out = Vec::new();
    for p in body.syntax().descendants().filter_map(ast::PathExpr::cast) {
        let Some(path) = p.path() else { continue };
        let text = path.syntax().text().to_string();
        let Some(q) = text.strip_suffix("::default").map(|q| q.rsplit("::").next().unwrap_or(q))
        else {
            continue;
        };
        if params.iter().any(|p| p == q) && !out.iter().any(|o| o == q) {
            out.push(q.to_string());
        }
    }
    out
}

// Calls in a body with the argument types at the call site (a method's receiver first).
fn calls(cx: &Cx, body: &SyntaxNode) -> Vec<(Function, Vec<Ty>)> {
    let mut out = Vec::new();
    for n in body.descendants() {
        if let Some(c) = ast::CallExpr::cast(n.clone()) {
            let Some(ast::Expr::PathExpr(pe)) = c.expr() else { continue };
            let Some(PathResolution::Def(ModuleDef::Function(f))) =
                pe.path().and_then(|p| cx.sema.resolve_path(&p))
            else {
                continue;
            };
            let tys = c.arg_list().map(|a| a.args().map(|x| cx.ty(&x)).collect()).unwrap_or_default();
            out.push((f, tys));
        } else if let Some(m) = ast::MethodCallExpr::cast(n) {
            let Some(f) = cx.sema.resolve_method_call(&m) else { continue };
            let mut tys: Vec<Ty> = m.receiver().map(|r| vec![cx.ty(&r)]).unwrap_or_default();
            tys.extend(m.arg_list().map(|a| a.args().map(|x| cx.ty(&x)).collect::<Vec<_>>()).unwrap_or_default());
            out.push((f, tys));
        }
    }
    out
}

// The callee's type parameters as inferred from the call's argument types.
pub(super) fn substitute(cx: &Cx, f: Function, arg_tys: &[Ty]) -> HashMap<String, Ty> {
    let mut subst = HashMap::new();
    for (param, arg) in f.assoc_fn_params(cx.db).iter().zip(arg_tys) {
        unify(&cx.convert(&param.ty()), arg, &mut subst);
    }
    subst
}

// `T` against `i32` inside `[T; 3]` against `[i32; 3]`.
fn unify(pattern: &Ty, actual: &Ty, out: &mut HashMap<String, Ty>) {
    match (pattern.strip_refs(), actual.strip_refs()) {
        // A `Param` against a `Param` passes the supplier down; a concrete type wins over it.
        (Ty::Param(p), a) => {
            if !matches!(a, Ty::Param(_)) || !out.contains_key(p) {
                out.insert(p.clone(), a.clone());
            }
        }
        (Ty::Vec(x), Ty::Vec(y)) | (Ty::Option(x), Ty::Option(y)) | (Ty::Iter(x), Ty::Iter(y)) => {
            unify(x, y, out)
        }
        (Ty::Tuple(xs), Ty::Tuple(ys)) | (Ty::Adt { args: xs, .. }, Ty::Adt { args: ys, .. }) => {
            for (x, y) in xs.iter().zip(ys) {
                unify(x, y, out);
            }
        }
        (Ty::Map { k: k1, v: v1, .. }, Ty::Map { k: k2, v: v2, .. }) => {
            unify(k1, k2, out);
            unify(v1, v2, out);
        }
        (Ty::Result(a, b), Ty::Result(c, d)) => {
            unify(a, c, out);
            unify(b, d, out);
        }
        _ => {}
    }
}

// A supplier literal for a concrete type, as the extra argument at a call site.
pub(super) fn default_supplier(ty: &Ty) -> Option<String> {
    Some(match ty.strip_refs() {
        Ty::Int(_) => "() -> 0".to_string(),
        Ty::F64 => "() -> 0.0".to_string(),
        Ty::F32 => "() -> 0.0f".to_string(),
        Ty::Bool => "() -> false".to_string(),
        Ty::Char => "() -> '\\0'".to_string(),
        Ty::Str => "() -> \"\"".to_string(),
        _ => return None,
    })
}

// The extra arguments a call to `f` needs: a `Supplier<T>` per type parameter it asks for.
pub(super) fn supplier_args(cx: &mut Cx, f: Function, arg_tys: &[Ty], node: &SyntaxNode) -> Vec<Expr> {
    let Some(names) = cx.supplier_fns.get(&f).cloned() else { return Vec::new() };
    let subst = substitute(cx, f, arg_tys);
    let mut out = Vec::new();
    for name in names {
        let java = match subst.get(&name).map(|t| t.strip_refs().clone()) {
            Some(Ty::Param(p)) => cx.suppliers.get(&p).map(|j| Expr::raw(j.clone())),
            Some(t) => default_supplier(&t).map(Expr::raw),
            None => None,
        };
        match java {
            Some(e) => out.push(e),
            None => {
                cx.unsupported(node, "`default()` on a type parameter with no known type");
            }
        }
    }
    out
}
