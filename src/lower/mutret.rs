// Simple mode: `fn bump(n: &mut i32) { *n += 1 }` returns the new value instead of writing
// through a one-element array: `static int bump(int n) { n += 1; return n; }`, called as
// `x = bump(x);` (DESIGN §22.3, "Cells for `&mut` primitives").

use std::collections::HashMap;

use ra_ap_hir::{Function, ModuleDef, PathResolution};
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast, ast::HasArgList, ast::HasGenericParams};

use super::{Cx, strip_parens};
use crate::jir::{ArmBody, Block, Expr, Stmt, StmtKind};

// Free functions returning `()` with exactly one `&mut <primitive>` parameter that is only read
// and written through `*n`, and called only as statements with `&mut local` / `&mut a.field`.
// Maps each to that parameter's index.
pub fn analyze(cx: &Cx, root: &SyntaxNode) -> HashMap<Function, usize> {
    let mut out = HashMap::new();
    let expanded = cx.expanded_descendants(root);
    for f in root.descendants().filter_map(ast::Fn::cast) {
        if let Some((def, idx)) = candidate(cx, &f)
            && call_sites_ok(cx, def, idx, &expanded)
        {
            out.insert(def, idx);
        }
    }
    out
}

fn candidate(cx: &Cx, f: &ast::Fn) -> Option<(Function, usize)> {
    let free = f
        .syntax()
        .parent()
        .is_some_and(|p| matches!(p.kind(), SyntaxKind::SOURCE_FILE | SyntaxKind::ITEM_LIST))
        && !f.syntax().ancestors().any(|a| a.kind() == SyntaxKind::IMPL);
    let unit = f
        .ret_type()
        .and_then(|r| r.ty())
        .is_none_or(|t| t.syntax().text() == "()");
    if !free || !unit || f.generic_param_list().is_some() {
        return None;
    }
    let params = f.param_list()?;
    if params.self_param().is_some() {
        return None;
    }
    let mut found = None;
    for (i, p) in params.params().enumerate() {
        let Some(ast::Type::RefType(r)) = p.ty() else {
            continue;
        };
        if r.mut_token().is_none() {
            continue;
        }
        let inner = cx.type_ref(&r.ty()?);
        if !inner.is_primitive() || found.is_some() {
            return None;
        }
        let Some(ast::Pat::IdentPat(ip)) = p.pat() else {
            return None;
        };
        if ip.mut_token().is_some() || ip.ref_token().is_some() {
            return None;
        }
        found = Some((i, ip));
    }
    let (idx, ip) = found?;
    let local = cx.sema.to_def(&ip)?;
    let body = f.body()?;
    // Every use is `*n`, outside closures (a lambda can't write a Java local).
    for pe in body.syntax().descendants().filter_map(ast::PathExpr::cast) {
        if cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) != Some(local) {
            continue;
        }
        let deref = pe
            .syntax()
            .parent()
            .and_then(ast::PrefixExpr::cast)
            .is_some_and(|p| p.op_kind() == Some(ast::UnaryOp::Deref));
        let in_closure = pe
            .syntax()
            .ancestors()
            .any(|a| ast::ClosureExpr::can_cast(a.kind()));
        if !deref || in_closure {
            return None;
        }
    }
    Some((cx.sema.to_def(f)?, idx))
}

fn call_sites_ok(cx: &Cx, def: Function, idx: usize, nodes: &[SyntaxNode]) -> bool {
    for pe in nodes.iter().cloned().filter_map(ast::PathExpr::cast) {
        let Some(path) = pe.path() else { continue };
        if !matches!(cx.sema.resolve_path(&path), Some(PathResolution::Def(ModuleDef::Function(f))) if f == def)
        {
            continue;
        }
        let call = pe.syntax().parent().and_then(ast::CallExpr::cast);
        let Some(call) = call.filter(|c| c.expr().is_some_and(|e| e.syntax() == pe.syntax()))
        else {
            return false;
        };
        let stmt = call
            .syntax()
            .parent()
            .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()));
        let arg = call.arg_list().and_then(|a| a.args().nth(idx));
        let place = match arg.map(|a| strip_parens(&a)) {
            Some(ast::Expr::RefExpr(r)) if r.mut_token().is_some() => {
                r.expr().map(|e| strip_parens(&e))
            }
            _ => None,
        };
        let plain = match place {
            Some(ast::Expr::PathExpr(p)) => cx.local_of_path(&ast::Expr::PathExpr(p)).is_some(),
            Some(ast::Expr::FieldExpr(_)) => true,
            _ => false,
        };
        if !stmt || !plain {
            return false;
        }
    }
    true
}

// The converted function `call` targets, and its `&mut` argument's place.
pub fn call_place(cx: &Cx, call: &ast::CallExpr) -> Option<ast::Expr> {
    let ast::Expr::PathExpr(pe) = call.expr()? else {
        return None;
    };
    let Some(PathResolution::Def(ModuleDef::Function(f))) = cx.sema.resolve_path(&pe.path()?)
    else {
        return None;
    };
    let idx = *cx.mut_returns.get(&f)?;
    match strip_parens(&call.arg_list()?.args().nth(idx)?) {
        ast::Expr::RefExpr(r) => r.expr(),
        _ => None,
    }
}

// `&mut x` passed to a converted function: `x` stays a plain local.
pub fn is_returned_arg(cx: &Cx, r: &ast::RefExpr) -> bool {
    r.syntax()
        .parent()
        .and_then(|a| a.parent())
        .and_then(ast::CallExpr::cast)
        .and_then(|c| call_place(cx, &c))
        .is_some_and(|p| r.expr().is_some_and(|e| e.syntax() == p.syntax()))
}

// The converted parameter's index for this function's definition.
pub fn param_of(cx: &Cx, f: &ast::Fn) -> Option<usize> {
    let def = cx.sema.to_def(f)?;
    cx.mut_returns.get(&def).copied()
}

// Every `return;` returns the parameter, and so does falling off the end.
pub fn return_param(body: &mut Block, name: &str) {
    returns_in(body, name);
    let ends = matches!(
        body.stmts.last().map(|s| &s.kind),
        Some(StmtKind::Return(_) | StmtKind::Throw(_))
    );
    if !ends {
        let line = body.close_line;
        body.stmts
            .push(Stmt::new(line, StmtKind::Return(Some(Expr::name(name)))));
    }
}

fn returns_in(b: &mut Block, name: &str) {
    for s in &mut b.stmts {
        stmt_returns(s, name);
    }
}

fn stmt_returns(s: &mut Stmt, name: &str) {
    match &mut s.kind {
        StmtKind::Return(r @ None) => *r = Some(Expr::name(name)),
        StmtKind::If { then, els, .. } => {
            returns_in(then, name);
            if let Some(e) = els {
                stmt_returns(e, name);
            }
        }
        StmtKind::While { body, .. }
        | StmtKind::For { body, .. }
        | StmtKind::ForEach { body, .. }
        | StmtKind::Labeled { body, .. }
        | StmtKind::Block(body) => returns_in(body, name),
        StmtKind::Try { body, handler, .. } => {
            returns_in(body, name);
            returns_in(handler, name);
        }
        StmtKind::Switch { arms, .. } => {
            for a in arms {
                if let ArmBody::Block(b) = &mut a.body {
                    returns_in(b, name);
                }
            }
        }
        _ => {}
    }
}
