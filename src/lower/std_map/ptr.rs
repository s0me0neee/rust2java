// Std Box/Rc/Arc/RefCell/Cell/Weak.

use ra_ap_hir::PathResolution;
use ra_ap_syntax::SyntaxNode;
use ra_ap_syntax::{
    AstNode, ast, ast::HasArgList, ast::HasGenericParams, ast::HasName, ast::HasTypeBounds,
};

use super::*;
use crate::lower::{calls, is_cell_value_ty, strip_parens};

// ---- representation --------------------------------------------------------------------------

// What `Cell`/`RefCell` hold, seeing through references and `Rc`/`Arc`/`Box`/`Pin`.
// `shared` marks `Rc`/`Arc`, whose clone must alias the same cell.
fn wrapper<'db>(cx: &Cx<'_, 'db>, ty: &ra_ap_hir::Type<'db>) -> Option<(Ty, bool)> {
    let mut t = ty.clone();
    let mut shared = false;
    loop {
        if t.is_reference() {
            t = t.as_reference()?.0;
            continue;
        }
        let adt = t.as_adt()?;
        match adt.name(cx.db).as_str() {
            "Rc" | "Arc" => shared = true,
            "Box" | "Pin" | "ManuallyDrop" => {}
            "Cell" | "RefCell" => {
                let inner = t.type_arguments().next()?;
                return Some((cx.convert(&inner), shared));
            }
            _ => return None,
        }
        let next = t.type_arguments().next()?;
        t = next;
    }
}

// The contents of a `Cell<T>`/`RefCell<T>` whose Java representation is a one-element array:
// locals, parameters, returns and `Rc`-shared values, but not bare `Cell` fields (plain Java fields).
pub fn array_cell<'db>(cx: &Cx<'_, 'db>, ty: &ra_ap_hir::Type<'db>) -> Option<Ty> {
    let (inner, _) = wrapper(cx, ty)?;
    is_cell_value_ty(&inner).then_some(inner)
}

// The array behind an expression, if its Java value is a cell array.
pub fn is_array(cx: &Cx, e: &ast::Expr) -> Option<Ty> {
    let t = cx.sema.type_of_expr(e)?.original.clone();
    if matches!(strip_parens(e), ast::Expr::FieldExpr(_)) {
        let (inner, shared) = wrapper(cx, &t)?;
        return (shared && is_cell_value_ty(&inner)).then_some(inner);
    }
    array_cell(cx, &t)
}

// The array declared for a `let` binding or parameter (`Cell<i32>` -> `int[]`).
pub fn array_cell_pat(cx: &Cx, p: &ast::IdentPat) -> Option<Ty> {
    array_cell(
        cx,
        &cx.sema
            .type_of_pat(&ast::Pat::IdentPat(p.clone()))?
            .original,
    )
}

// `let r = &mut x;`: the binding is itself the one-element array of `x`.
pub fn ref_mut_pat(cx: &Cx, p: &ast::IdentPat) -> Option<Ty> {
    let t = cx
        .sema
        .type_of_pat(&ast::Pat::IdentPat(p.clone()))?
        .original
        .clone();
    if !t.is_mutable_reference() {
        return None;
    }
    let inner = cx.convert(&t.as_reference()?.0);
    is_cell_value_ty(&inner).then_some(inner)
}

pub fn array_cell_type_ref(cx: &Cx, t: &ast::Type) -> Option<Ty> {
    array_cell(cx, &cx.sema.resolve_type(t)?)
}

// The array a function returns, if its return type is a cell.
pub fn array_cell_ret(cx: &Cx, f: &ast::Fn) -> Option<Ty> {
    let rt = f.ret_type()?;
    array_cell_type_ref(cx, &rt.ty()?)
}

// `dyn Fn`/`impl Fn` bounds: the functional interface a generic parameter stands for.
pub fn fn_bounds(cx: &Cx, f: &ast::Fn) -> Vec<(String, Ty)> {
    let bound_ty = |bound: &ast::TypeBound| -> Option<Ty> {
        let ty = bound.ty()?;
        let s = ty.syntax().text().to_string();
        (s.starts_with("Fn") || s.starts_with("dyn Fn")).then(|| cx.type_ref(&ty))
    };
    let mut result = Vec::new();
    if let Some(params) = f.generic_param_list() {
        for gp in params.generic_params() {
            let ast::GenericParam::TypeParam(tp) = gp else {
                continue;
            };
            let Some(name) = tp.name() else { continue };
            if let Some(bounds) = tp.type_bound_list() {
                for bound in bounds.bounds() {
                    if let Some(ty) = bound_ty(&bound) {
                        result.push((name.text().to_string(), ty));
                    }
                }
            }
        }
    }
    if let Some(clause) = f.where_clause() {
        for pred in clause.predicates() {
            let Some(ast::Type::PathType(pt)) = pred.ty() else {
                continue;
            };
            let Some(path) = pt.path() else { continue };
            if let Some(bounds) = pred.type_bound_list() {
                for bound in bounds.bounds() {
                    if let Some(ty) = bound_ty(&bound) {
                        result.push((path.syntax().text().to_string(), ty));
                    }
                }
            }
        }
    }
    result
}

fn first_arg(c: &ast::CallExpr) -> Option<ast::Expr> {
    c.arg_list()?.args().next()
}

fn local_key(cx: &Cx, c: &ast::CallExpr) -> String {
    let Some(ast::Expr::PathExpr(pe)) = c.expr() else {
        return String::new();
    };
    let Some(path) = pe.path() else {
        return String::new();
    };
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Def(ModuleDef::Function(f))) if !calls::is_local_fn(cx, f) => {
            calls::std_fn_key(cx, f)
        }
        _ => String::new(),
    }
}

fn method_key(cx: &Cx, m: &ast::MethodCallExpr) -> String {
    match cx.sema.resolve_method_call(m) {
        Some(f) if !calls::is_local_fn(cx, f) => calls::std_fn_key(cx, f),
        _ => String::new(),
    }
}

// Whether `e` hands out the array itself (a shared handle) rather than a copy of the contents.
pub fn is_handle(cx: &Cx, e: &ast::Expr) -> bool {
    match strip_parens(e) {
        ast::Expr::RefExpr(r) => r.expr().is_some_and(|i| is_handle(cx, &i)),
        ast::Expr::PathExpr(_) => cx.handle_path(e).is_some() || cx.cell_path(e).is_some(),
        ast::Expr::FieldExpr(_) => is_array(cx, e).is_some(),
        ast::Expr::CallExpr(c) => match local_key(cx, &c).as_str() {
            "Rc::clone" | "Arc::clone" | "Rc::new" | "Arc::new" | "Box::new" | "Pin::new" => {
                first_arg(&c).is_some_and(|a| is_handle(cx, &a))
            }
            k if k.starts_with("Cell::") || k.starts_with("RefCell::") => {
                matches!(k, "Cell::new" | "RefCell::new")
            }
            _ => true,
        },
        ast::Expr::MethodCallExpr(m) => match method_key(cx, &m).as_str() {
            "Rc::clone" | "Arc::clone" => m.receiver().is_some_and(|r| is_handle(cx, &r)),
            "Cell::clone" | "RefCell::clone" => false,
            _ => true,
        },
        _ => false,
    }
}

// The value for a `T[]` cell slot: the expression itself when it is already the array.
pub fn as_handle(cx: &Cx, e: &ast::Expr, v: Expr, inner: &Ty) -> Expr {
    if is_handle(cx, e) {
        return v;
    }
    Expr::raw(format!("new {}[]{{{}}}", inner.java(), render(&v)))
}

// The cell's contents: `x[0]` for an array cell, the value itself for a Java object.
pub fn contents(cx: &Cx, recv: &ast::Expr, v: Expr) -> Expr {
    match is_array(cx, recv) {
        Some(_) => Expr::raw(format!("{}[0]", render(&v))),
        None => v,
    }
}

// A bare `Cell`/`RefCell` field is a plain Java field, so it cannot be shared by reference.
pub fn bare_cell_field(cx: &Cx, e: &ast::Expr) -> bool {
    if !matches!(
        strip_parens(e),
        ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_)
    ) {
        return false;
    }
    let Some(t) = cx.sema.type_of_expr(e) else {
        return false;
    };
    let Some((inner, shared)) = wrapper(cx, &t.original.clone()) else {
        return false;
    };
    !shared && is_cell_value_ty(&inner)
}

// Debug/Display of a `Cell`/`RefCell` value itself (`Cell { value: 5 }` in Rust).
pub fn cell_value_type<'db>(cx: &Cx<'_, 'db>, ty: &ra_ap_hir::Type<'db>) -> bool {
    wrapper(cx, ty).is_some()
}

pub fn cell_value(cx: &Cx, e: &ast::Expr) -> bool {
    cx.sema
        .type_of_expr(e)
        .is_some_and(|t| cell_value_type(cx, &t.original.clone()))
}

// The contents of a cell-array operand (`c == d` compares the values, not the arrays).
pub fn content_value(cx: &Cx, e: &ast::Expr, v: Expr) -> Expr {
    if cx.handle_path(e).is_some() || (is_array(cx, e).is_some() && is_handle(cx, e)) {
        contents(cx, e, v)
    } else {
        v
    }
}

// A `Cell::new` stored straight into a field keeps the field's plain representation.
fn plain_slot(node: &SyntaxNode) -> bool {
    let mut node = node.clone();
    loop {
        let Some(parent) = node.parent() else {
            return false;
        };
        if ast::ParenExpr::can_cast(parent.kind()) {
            node = parent;
            continue;
        }
        if ast::RecordExprField::can_cast(parent.kind()) {
            return true;
        }
        if let Some(b) = ast::BinExpr::cast(parent) {
            let our_range = node.text_range();
            let is_rhs = b
                .rhs()
                .is_some_and(|r| r.syntax().text_range() == our_range);
            return is_rhs
                && b.op_kind()
                    .is_some_and(|op| matches!(op, ast::BinaryOp::Assignment { .. }))
                && b.lhs().is_some_and(|l| {
                    matches!(l, ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_))
                });
        }
        return false;
    }
}

// Java's standard functional interfaces support at most two arguments.
pub fn note_fn(cx: &mut Cx, node: &SyntaxNode, ty: &Ty) {
    if let Ty::Fn { params, .. } = ty.strip_refs()
        && params.len() > 2
    {
        cx.unsupported(node, "functional values with more than two arguments");
    }
}

// ---- the hook for free functions (`Cell::new`, `Rc::clone`, ...) -----------------------------

pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let (owner, name) = key.split_once("::")?;
    if !matches!(
        owner,
        "Cell" | "RefCell" | "Rc" | "Arc" | "Weak" | "Box" | "Pin"
    ) {
        return None;
    }
    let _ = ret;
    let args = arg_asts(c);
    let _line = cx.line(c.syntax());
    match (owner, name) {
        ("Cell" | "RefCell", "new") => {
            let a = args.first()?;
            let v = expr::lower_expr(cx, a, out);
            let inner = cx.ty(a);
            if is_cell_value_ty(&inner) && !plain_slot(c.syntax()) {
                return Some(Expr::raw(format!(
                    "new {}[]{{{}}}",
                    inner.java(),
                    render(&v)
                )));
            }
            Some(v)
        }
        // `Rc::from`, `Box::from` of a `&str`/slice: the value is its own representation.
        ("Rc" | "Arc" | "Box", "from") => args.first().map(|a| expr::lower_expr(cx, a, out)),
        ("Rc" | "Arc" | "Box", "from_iter") => {
            let v = expr::lower_expr(cx, args.first()?, out);
            Some(Expr::New {
                ty: "ArrayList<>".into(),
                args: vec![v],
            })
        }
        ("Rc" | "Arc" | "Weak", _) => Some(lower_rc(cx, c.syntax(), key, &args, out)),
        ("Box" | "Pin", _) => Some(lower_rc(cx, c.syntax(), key, &args, out)),
        // UFCS forms of the methods below, e.g. `Cell::get(&c)`.
        _ if !args.is_empty() => lower_cell_op(cx, c.syntax(), key, &args[0], &args[1..], out),
        _ => None,
    }
}

fn lower_rc(
    cx: &mut Cx,
    node: &SyntaxNode,
    key: &str,
    args: &[ast::Expr],
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(node);
    let name = key.rsplit("::").next().unwrap_or(key);
    match name {
        "clone" => match args.first() {
            Some(a) => expr::lower_expr(cx, a, out),
            None => cx.unsupported(node, "`Rc::clone` without argument"),
        },
        "ptr_eq" => {
            let (Some(a), Some(b)) = (args.first(), args.get(1)) else {
                return cx.unsupported(node, "`Rc::ptr_eq` without two arguments");
            };
            if is_array(cx, a).is_none() && is_cell_value_ty(&cx.ty(a)) {
                return cx.unsupported(node, "`Rc::ptr_eq` on erased immutable values");
            }
            let l = expr::lower_expr(cx, a, out);
            let r = expr::lower_expr(cx, b, out);
            Expr::bin(l, "==", r)
        }
        "new" => match args.first() {
            Some(a) => expr::lower_expr(cx, a, out),
            None => Expr::raw("null"),
        },
        "strong_count" | "weak_count" => cx.unsupported(
            node,
            "`Rc::strong_count`/`weak_count`: Java has no reference counts",
        ),
        "downgrade" | "upgrade" => cx.unsupported(node, "`Weak` pointers"),
        "get_mut" | "make_mut" | "try_unwrap" | "into_raw" | "from_raw" | "as_ptr" | "alloc" => {
            cx.unsupported(node, &format!("`{key}`"))
        }
        _ => {
            let _ = line;
            cx.unsupported(node, &format!("`{key}`"))
        }
    }
}

// ---- the hook for methods (`x.borrow_mut()`, `c.get()`, `x.clone()`) -------------------------

pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = recv_ty;
    let owner = key.split_once("::").map(|(o, _)| o)?;
    if !matches!(owner, "Cell" | "RefCell" | "Rc" | "Arc" | "Box" | "Weak") {
        return None;
    }
    let recv = m.receiver()?;
    let args = arg_asts(m);
    match owner {
        "Rc" | "Arc" | "Weak" | "Box" => Some(lower_rc(
            cx,
            m.syntax(),
            key,
            &[vec![recv], args].concat(),
            out,
        )),
        _ => lower_cell_op(cx, m.syntax(), key, &recv, &args, out),
    }
}

// Repeated place access is safe only when evaluating the receiver has no effects.
fn stable_place(e: &ast::Expr) -> bool {
    match strip_parens(e) {
        ast::Expr::PathExpr(_) => true,
        ast::Expr::RefExpr(r) => r.expr().is_some_and(|e| stable_place(&e)),
        ast::Expr::FieldExpr(f) => f.expr().is_some_and(|e| stable_place(&e)),
        _ => false,
    }
}

pub fn shared_object(cx: &Cx, e: &ast::Expr) -> bool {
    cx.sema.type_of_expr(e).is_some_and(|t| {
        wrapper(cx, &t.original.clone())
            .is_some_and(|(inner, shared)| shared && !is_cell_value_ty(&inner))
    })
}

// `Cell`/`RefCell` operations on a lowered receiver.
fn lower_cell_op(
    cx: &mut Cx,
    node: &SyntaxNode,
    key: &str,
    recv: &ast::Expr,
    args: &[ast::Expr],
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let line = cx.line(node);
    let name = key.rsplit("::").next().unwrap_or(key);
    if matches!(name, "set" | "replace" | "take" | "swap") && shared_object(cx, recv) {
        return Some(cx.unsupported(node, "replacing a shared object-backed cell"));
    }
    if matches!(name, "replace" | "take" | "update" | "swap")
        && (!stable_place(recv)
            || (name == "swap" && args.first().is_some_and(|e| !stable_place(e))))
    {
        return Some(cx.unsupported(node, "cell operation on a complex receiver"));
    }
    let inner = is_array(cx, recv).or_else(|| {
        let t = cx.sema.type_of_expr(recv)?.original.clone();
        wrapper(cx, &t).map(|(t, _)| t)
    });
    let place = |cx: &mut Cx, out: &mut Vec<Stmt>| {
        let v = expr::lower_expr(cx, recv, out);
        contents(cx, recv, v)
    };
    match name {
        "borrow" | "borrow_mut" | "get" | "into_inner" => {
            let v = expr::lower_expr(cx, recv, out);
            Some(contents(cx, recv, v))
        }
        "set" => {
            let inner = inner?;
            let p = place(cx, out);
            let v = expr::lower_expr_as(cx, args.first()?, &inner, out);
            Some(Expr::Assign(Box::new(p), "=", Box::new(v)))
        }
        "replace" => {
            let inner = inner?;
            let p = place(cx, out);
            let v = expr::lower_expr_as(cx, args.first()?, &inner, out);
            let value = push_local(
                cx,
                line,
                super::super::stmt::decl_type(&inner),
                Some(v),
                out,
                "value",
            );
            let tmp = push_local(
                cx,
                line,
                super::super::stmt::decl_type(&inner),
                Some(p.clone()),
                out,
                "old",
            );
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::Assign(Box::new(p), "=", Box::new(Expr::name(value)))),
            ));
            Some(Expr::name(tmp))
        }
        "take" => {
            let inner = inner?;
            let p = place(cx, out);
            let tmp = push_local(
                cx,
                line,
                super::super::stmt::decl_type(&inner),
                Some(p.clone()),
                out,
                "old",
            );
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::Assign(
                    Box::new(p),
                    "=",
                    Box::new(default_value(&inner)),
                )),
            ));
            Some(Expr::name(tmp))
        }
        "update" => {
            let p = place(cx, out);
            let arg = args.first()?;
            let f = expr::lower_expr(cx, arg, out);
            // A lambda has no target type when it is called directly; name its interface first.
            let f = if matches!(f, Expr::Lambda { .. }) {
                let fty = cx.ty(arg);
                let name = push_local(
                    cx,
                    line,
                    super::super::stmt::decl_type(&fty),
                    Some(f),
                    out,
                    "f",
                );
                Expr::name(name)
            } else {
                f
            };
            Some(Expr::Assign(
                Box::new(p.clone()),
                "=",
                Box::new(Expr::call(f, "apply", vec![p])),
            ))
        }
        "swap" => {
            let a = place(cx, out);
            let b = {
                let other = args.first()?;
                let v = expr::lower_expr(cx, other, out);
                contents(cx, other, v)
            };
            let inner = inner?;
            let tmp = push_local(
                cx,
                line,
                super::super::stmt::decl_type(&inner),
                Some(a.clone()),
                out,
                "tmp",
            );
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::Assign(Box::new(a), "=", Box::new(b.clone()))),
            ));
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::Assign(Box::new(b), "=", Box::new(Expr::name(tmp)))),
            ));
            Some(Expr::raw("null"))
        }
        "clone" => {
            let v = expr::lower_expr(cx, recv, out);
            let v = contents(cx, recv, v);
            let inner = inner?;
            Some(clone_expr(v, &inner))
        }
        "try_borrow" | "try_borrow_mut" | "get_mut" | "swap_borrow" | "undo_leak" | "as_ptr"
        | "as_slice_of_cells" | "from_mut" | "from_mut_slice" | "map" | "map_split"
        | "filter_map" | "leak" | "clone_into" => Some(cx.unsupported(node, &format!("`{key}`"))),
        _ => None,
    }
}
