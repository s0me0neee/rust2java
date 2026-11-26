// Simple mode: `fn emit(out: &mut String, ..)` takes a `StringBuilder` instead of a one-element
// `String[]`, and the locals handed to it (`let mut s = String::new(); emit(&mut s, ..)`) are
// builders too. The parameters and locals are chosen together: every hand-off is between them.

use std::collections::HashSet;

use ra_ap_hir::{Function, Local, ModuleDef, PathResolution};
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast, ast::HasArgList};

use super::{Cx, strip_parens};
use crate::ty::Ty;

pub struct Builders<'db> {
    pub params: HashSet<(Function, usize)>,
    pub locals: HashSet<Local<'db>>,
}

enum Use {
    Ok,
    Fail,
    // Handed on as `&mut` to parameter `.1` of `.0`.
    Pass(Function, usize),
}

struct Var<'db> {
    local: Local<'db>,
    // `Some` for a parameter: its function and index.
    param: Option<(Function, usize)>,
    uses: Vec<Use>,
}

pub fn analyze<'db>(cx: &Cx<'_, 'db>, root: &SyntaxNode) -> Builders<'db> {
    let mut vars = Vec::new();
    for f in root.descendants().filter_map(ast::Fn::cast) {
        let Some(def) = cx.sema.to_def(&f) else {
            continue;
        };
        let Some(body) = f.body() else { continue };
        let nodes = cx.expanded_descendants(body.syntax());
        let in_trait_impl = f
            .syntax()
            .ancestors()
            .find_map(ast::Impl::cast)
            .is_some_and(|i| i.trait_().is_some());
        if let Some(pl) = f.param_list()
            && !in_trait_impl
        {
            for (i, p) in pl.params().enumerate() {
                let Some(ast::Type::RefType(r)) = p.ty() else {
                    continue;
                };
                let Some(ast::Pat::IdentPat(ip)) = p.pat() else {
                    continue;
                };
                if r.mut_token().is_none()
                    || ip.mut_token().is_some()
                    || ip.ref_token().is_some()
                    || !r.ty().is_some_and(|t| matches!(cx.type_ref(&t), Ty::Str))
                {
                    continue;
                }
                let Some(local) = cx.sema.to_def(&ip) else {
                    continue;
                };
                let uses = uses_of(cx, local, true, &nodes, &f);
                vars.push(Var {
                    local,
                    param: Some((def, i)),
                    uses,
                });
            }
        }
        for ip in nodes.iter().cloned().filter_map(ast::IdentPat::cast) {
            let is_let = ip
                .syntax()
                .parent()
                .is_some_and(|p| ast::LetStmt::can_cast(p.kind()));
            if !is_let || ip.mut_token().is_none() || ip.ref_token().is_some() {
                continue;
            }
            let Some(local) = cx.sema.to_def(&ip) else {
                continue;
            };
            if !matches!(cx.convert(&local.ty(cx.db)), Ty::Str) {
                continue;
            }
            let uses = uses_of(cx, local, false, &nodes, &f);
            // Only locals handed to a parameter; the others keep the usual builder rule.
            if uses.iter().any(|u| matches!(u, Use::Pass(..))) {
                vars.push(Var {
                    local,
                    param: None,
                    uses,
                });
            }
        }
    }
    let calls = call_args(cx, root);
    let mut alive: Vec<bool> = vars
        .iter()
        .map(|v| !v.uses.iter().any(|u| matches!(u, Use::Fail)))
        .collect();
    loop {
        let params: HashSet<(Function, usize)> = vars
            .iter()
            .zip(&alive)
            .filter(|(_, a)| **a)
            .filter_map(|(v, _)| v.param)
            .collect();
        let locals: HashSet<Local> = vars
            .iter()
            .zip(&alive)
            .filter(|(_, a)| **a)
            .map(|(v, _)| v.local)
            .collect();
        let mut changed = false;
        for (v, a) in vars.iter().zip(alive.iter_mut()) {
            if !*a {
                continue;
            }
            let passes_ok = v.uses.iter().all(|u| match u {
                Use::Pass(f, i) => params.contains(&(*f, *i)),
                _ => true,
            });
            // Each caller hands over a builder: `&mut s` or another builder parameter.
            let callers_ok = v.param.is_none_or(|(f, i)| {
                calls.iter().filter(|(g, ..)| *g == f).all(|(_, args, ok)| {
                    *ok && args
                        .get(i)
                        .and_then(|a| a.as_ref())
                        .is_some_and(|l| locals.contains(l))
                })
            });
            if !passes_ok || !callers_ok {
                *a = false;
                changed = true;
            }
        }
        if !changed {
            return Builders { params, locals };
        }
    }
}

// How each mention of `local` uses it.
fn uses_of(cx: &Cx, local: Local, param: bool, nodes: &[SyntaxNode], f: &ast::Fn) -> Vec<Use> {
    let mut out = Vec::new();
    for pe in nodes.iter().cloned().filter_map(ast::PathExpr::cast) {
        if cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) != Some(local) {
            continue;
        }
        let closure = cx
            .sema
            .ancestors_with_macros(pe.syntax().clone())
            .take_while(|n| n != f.syntax())
            .any(|n| ast::ClosureExpr::can_cast(n.kind()));
        out.push(if closure {
            Use::Fail
        } else {
            classify(cx, &pe, param)
        });
    }
    out
}

fn classify(cx: &Cx, pe: &ast::PathExpr, param: bool) -> Use {
    let mut user = pe.syntax().clone();
    let mut mut_ref = false;
    while let Some(p) = user.parent() {
        match p.kind() {
            SyntaxKind::PAREN_EXPR => {}
            SyntaxKind::PREFIX_EXPR
                if ast::PrefixExpr::cast(p.clone())
                    .is_some_and(|e| e.op_kind() == Some(ast::UnaryOp::Deref)) => {}
            SyntaxKind::REF_EXPR => {
                mut_ref |= ast::RefExpr::cast(p.clone()).is_some_and(|r| r.mut_token().is_some())
            }
            _ => break,
        }
        user = p;
    }
    let Some(parent) = user.parent() else {
        return Use::Ok;
    };
    if let Some(m) = ast::MethodCallExpr::cast(parent.clone())
        && m.receiver().is_some_and(|r| r.syntax() == &user)
    {
        let stmt = m
            .syntax()
            .parent()
            .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()));
        let name = m
            .name_ref()
            .map(|n| n.text().to_string())
            .unwrap_or_default();
        return match name.as_str() {
            "push" | "push_str" | "write_fmt" | "write_str" | "len" | "is_empty" | "clear" => {
                Use::Ok
            }
            "pop" | "truncate" if stmt => Use::Ok,
            "pop"
            | "insert"
            | "insert_str"
            | "remove"
            | "retain"
            | "truncate"
            | "drain"
            | "extend"
            | "replace_range"
            | "make_ascii_uppercase"
            | "make_ascii_lowercase"
            | "split_off"
            | "reserve"
            | "shrink_to_fit"
            | "as_mut_str" => Use::Fail,
            _ => Use::Ok,
        };
    }
    if let Some(b) = ast::BinExpr::cast(parent.clone())
        && b.lhs().is_some_and(|l| l.syntax() == &user)
    {
        return match b.op_kind() {
            Some(ast::BinaryOp::Assignment {
                op: Some(ast::ArithOp::Add),
            }) => Use::Ok,
            // Rebinding a parameter wouldn't reach the caller's builder.
            Some(ast::BinaryOp::Assignment { op: None }) if !param => Use::Ok,
            Some(ast::BinaryOp::Assignment { .. }) => Use::Fail,
            _ => Use::Ok,
        };
    }
    if ast::ArgList::can_cast(parent.kind()) {
        let reborrow = param && user == *pe.syntax();
        if !mut_ref && !reborrow {
            return Use::Ok;
        }
        let Some(pos) = parent.children().position(|c| c == user) else {
            return Use::Fail;
        };
        return match callee(cx, &parent.parent()) {
            Some(f) => Use::Pass(f, pos),
            None => Use::Fail,
        };
    }
    if mut_ref { Use::Fail } else { Use::Ok }
}

// The crate function a call (or method call) node targets.
fn callee(cx: &Cx, call: &Option<SyntaxNode>) -> Option<Function> {
    let call = call.clone()?;
    if let Some(m) = ast::MethodCallExpr::cast(call.clone()) {
        return cx.sema.resolve_method_call(&m);
    }
    let ast::Expr::PathExpr(p) = ast::CallExpr::cast(call)?.expr()? else {
        return None;
    };
    match cx.sema.resolve_path(&p.path()?)? {
        PathResolution::Def(ModuleDef::Function(f)) => Some(f),
        _ => None,
    }
}

// Every mention of a crate function: (function, the local each argument names through `&mut x`
// or a plain path, whether the mention is a direct call).
fn call_args<'db>(
    cx: &Cx<'_, 'db>,
    root: &SyntaxNode,
) -> Vec<(Function, Vec<Option<Local<'db>>>, bool)> {
    let mut out = Vec::new();
    let local_arg = |a: ast::Expr| {
        let inner = match strip_parens(&a) {
            ast::Expr::RefExpr(r) if r.mut_token().is_some() => {
                r.expr().map(|e| strip_parens(&e))?
            }
            other => other,
        };
        let inner = match inner {
            ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => p.expr()?,
            other => other,
        };
        cx.local_of_path(&inner)
    };
    for n in cx.expanded_descendants(root) {
        if let Some(m) = ast::MethodCallExpr::cast(n.clone())
            && let Some(f) = cx.sema.resolve_method_call(&m)
        {
            let args = m
                .arg_list()
                .map(|a| a.args().map(local_arg).collect())
                .unwrap_or_default();
            out.push((f, args, true));
            continue;
        }
        let Some(pe) = ast::PathExpr::cast(n) else {
            continue;
        };
        let Some(PathResolution::Def(ModuleDef::Function(f))) =
            pe.path().and_then(|p| cx.sema.resolve_path(&p))
        else {
            continue;
        };
        let call = pe
            .syntax()
            .parent()
            .and_then(ast::CallExpr::cast)
            .filter(|c| c.expr().is_some_and(|e| e.syntax() == pe.syntax()));
        match call {
            Some(c) => {
                let args = c
                    .arg_list()
                    .map(|a| a.args().map(local_arg).collect())
                    .unwrap_or_default();
                out.push((f, args, true));
            }
            None => out.push((f, Vec::new(), false)),
        }
    }
    out
}
