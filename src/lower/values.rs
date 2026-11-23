// Rust structs and arrays are values; their Java classes/lists are shared references. Two places
// differ: `*p = v` through a `&mut` must change the object `p` points at (fields are copied in),
// and copying a `Copy` struct or array must not share it when someone later changes one in place.

use std::collections::HashSet;

use ra_ap_hir::Adt;
use ra_ap_syntax::{AstNode, SyntaxNode, ast, ast::HasName};

use super::Cx;
use crate::jir::{Expr, Stmt, StmtKind};

fn struct_of<'db>(t: &ra_ap_hir::Type<'db>) -> Option<ra_ap_hir::Struct> {
    match t.as_adt()? {
        Adt::Struct(s) => Some(s),
        _ => None,
    }
}

// Struct and array types some code changes in place (`a.f = x`, `v[i] = x`, any `&mut` to one).
#[derive(Default)]
pub struct Mutated {
    structs: HashSet<ra_ap_hir::Struct>,
    // Structs with a field assigned or `&mut`-borrowed anywhere (they can't be Java records).
    field_written: HashSet<ra_ap_hir::Struct>,
    // Arrays by their `Ty` (arrays and `Vec` share it, but only arrays are `Copy`).
    arrays: HashSet<String>,
}

fn array_key<'db>(cx: &Cx<'_, 'db>, t: &ra_ap_hir::Type<'db>) -> Option<String> {
    t.is_array().then(|| format!("{:?}", cx.convert(t)))
}

fn mutated_types<'db>(cx: &Cx<'_, 'db>) -> Mutated {
    let mut found = Mutated::default();
    let note = |cx: &Cx<'_, 'db>, t: &ra_ap_hir::Type<'db>, found: &mut Mutated| {
        let t = t.strip_references();
        if let Some(s) = struct_of(&t) {
            found.structs.insert(s);
        }
        if let Some(k) = array_key(cx, &t) {
            found.arrays.insert(k);
        }
    };
    let krate = cx.module.krate(cx.db);
    for module in krate.modules(cx.db) {
        let src = module.definition_source(cx.db);
        if !matches!(src.value, ra_ap_hir::ModuleSource::SourceFile(_)) {
            continue;
        }
        let root = cx.sema.parse(src.file_id.original_file(cx.db));
        for node in root.syntax().descendants() {
            // `a.f = x`, `a.f += x`, `&mut a.f`: the struct owning `f` changes.
            let written = if let Some(b) = ast::BinExpr::cast(node.clone())
                && matches!(b.op_kind(), Some(ast::BinaryOp::Assignment { .. }))
            {
                b.lhs()
            } else if let Some(r) = ast::RefExpr::cast(node.clone())
                && r.mut_token().is_some()
            {
                r.expr()
            } else {
                None
            };
            if let Some(ast::Expr::FieldExpr(f)) = written.map(|w| super::strip_parens(&w))
                && let Some(ra_ap_hir::Variant::Struct(s)) = cx
                    .sema
                    .resolve_field(&f)
                    .and_then(|r| r.left())
                    .map(|fd| fd.parent_def(cx.db))
            {
                found.field_written.insert(s);
            }
            if let Some(b) = ast::BinExpr::cast(node.clone())
                && matches!(b.op_kind(), Some(ast::BinaryOp::Assignment { .. }))
            {
                // `a.b.c = x` changes `a.b` (and so `a`) in place.
                let mut place = b.lhs();
                while let Some(p) = place {
                    place = match &p {
                        ast::Expr::FieldExpr(f) => f.expr(),
                        ast::Expr::IndexExpr(i) => i.base(),
                        ast::Expr::PrefixExpr(d) => d.expr(),
                        ast::Expr::ParenExpr(e) => e.expr(),
                        _ => None,
                    };
                    if let Some(base) = &place
                        && let Some(t) = cx.sema.type_of_expr(base)
                    {
                        note(cx, &t.original, &mut found);
                    }
                }
            } else if let Some(e) = ast::Expr::cast(node)
                && let Some(t) = cx.sema.type_of_expr(&e)
            {
                let t = t.adjusted();
                if t.is_mutable_reference() {
                    note(cx, &t, &mut found);
                }
            }
        }
    }
    found
}

// Simple mode: a struct nothing changes in place becomes a Java record.
pub fn record_ok(cx: &mut Cx, s: ra_ap_hir::Struct) -> bool {
    if cx.mutated.is_none() {
        cx.mutated = Some(mutated_types(cx));
    }
    let m = cx.mutated.as_ref().unwrap();
    !m.structs.contains(&s) && !m.field_written.contains(&s)
}

// Whether a copy of a value of this type must be a fresh object: a `Copy` struct or array that
// is (or holds a struct that is) changed in place somewhere.
fn copy_needs_clone<'db>(cx: &mut Cx<'_, 'db>, t: &ra_ap_hir::Type<'db>) -> bool {
    if !t.is_copy(cx.db) {
        return false;
    }
    if cx.mutated.is_none() {
        cx.mutated = Some(mutated_types(cx));
    }
    let key = array_key(cx, t);
    let mutated = cx.mutated.as_ref().unwrap();
    if let Some(k) = key {
        return mutated.arrays.contains(&k);
    }
    let Some(s) = struct_of(t) else { return false };
    let mut stack = vec![s];
    let mut seen = HashSet::new();
    while let Some(s) = stack.pop() {
        if !seen.insert(s) {
            continue;
        }
        if mutated.structs.contains(&s) {
            return true;
        }
        stack.extend(
            s.fields(cx.db)
                .iter()
                .filter_map(|f| struct_of(&f.ty(cx.db))),
        );
    }
    false
}

// The syntax position decides whether the value is copied: receivers, `&x`, field bases,
// comparisons and printing only read it.
fn is_value_use(node: &SyntaxNode) -> bool {
    let mut child = node.clone();
    while let Some(parent) = child.parent() {
        if ast::ParenExpr::can_cast(parent.kind()) {
            child = parent;
            continue;
        }
        if let Some(m) = ast::MethodCallExpr::cast(parent.clone()) {
            return m.receiver().is_none_or(|r| r.syntax() != &child);
        }
        if let Some(b) = ast::BinExpr::cast(parent.clone()) {
            return match b.op_kind() {
                // `c += b` only reads `b`; `c = b` copies it.
                Some(ast::BinaryOp::Assignment { op: None }) => {
                    b.rhs().is_some_and(|r| r.syntax() == &child)
                }
                _ => false,
            };
        }
        return !(ast::FieldExpr::can_cast(parent.kind())
            || ast::IndexExpr::can_cast(parent.kind())
            || ast::RefExpr::can_cast(parent.kind())
            || ast::ExprStmt::can_cast(parent.kind())
            || ast::MatchExpr::can_cast(parent.kind())
            || ast::LetExpr::can_cast(parent.kind())
            || ast::FormatArgsArg::can_cast(parent.kind())
            || ast::PrefixExpr::can_cast(parent.kind()));
    }
    false
}

// `x`, `a.b`, `v[i]`, `*p` copied out of a place whose `Copy` type is changed in place: the
// lowered value `v` as a fresh object (`.clone()`, or a list copy for arrays).
pub fn copy_of(cx: &mut Cx, e: &ast::Expr, v: Expr) -> Expr {
    if !needs_copy(cx, e) {
        return v;
    }
    match cx.ty(e) {
        crate::ty::Ty::Vec(elem) => super::std_map::coll::clone_list(cx, v, &elem),
        _ => Expr::call(v, "clone", vec![]),
    }
}

fn needs_copy(cx: &mut Cx, e: &ast::Expr) -> bool {
    let is_place = match e {
        ast::Expr::PathExpr(_) | ast::Expr::FieldExpr(_) | ast::Expr::IndexExpr(_) => true,
        ast::Expr::PrefixExpr(p) => p.op_kind() == Some(ast::UnaryOp::Deref),
        _ => false,
    };
    if !is_place || !is_value_use(e.syntax()) {
        return false;
    }
    // `impl Clone { fn clone(&self) -> Self { *self } }` must not call itself.
    if e.syntax()
        .ancestors()
        .filter_map(ast::Fn::cast)
        .next()
        .is_some_and(|f| f.name().is_some_and(|n| n.text() == "clone"))
    {
        return false;
    }
    match cx.sema.type_of_expr(e) {
        Some(t) if !t.original.is_reference() => copy_needs_clone(cx, &t.original),
        _ => false,
    }
}

// `*p = v` where `p: &mut S` for a struct `S`: copy `v`'s fields into the object `p` names.
pub fn assign_through(
    cx: &mut Cx,
    target_ast: &ast::Expr,
    value: Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let t = cx.sema.type_of_expr(target_ast)?.original;
    if !t.is_mutable_reference() {
        return None;
    }
    let s = struct_of(&t.strip_references())?;
    let fields = s.fields(cx.db);
    let names = super::items::java_field_names(cx, &fields);
    let target = super::expr::lower_expr(cx, target_ast, out);
    let simple = |e: &Expr| {
        matches!(e, Expr::Name(_))
            || matches!(e, Expr::Raw(r) if r.chars().all(|c| c.is_alphanumeric() || c == '_'))
    };
    let target = if simple(&target) {
        target
    } else {
        let n = cx.fresh("target");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "var".into(),
                name: n.clone(),
                init: Some(target),
            },
        ));
        Expr::name(n)
    };
    let value = if simple(&value) || names.len() <= 1 {
        value
    } else {
        let n = cx.fresh("value");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "var".into(),
                name: n.clone(),
                init: Some(value),
            },
        ));
        Expr::name(n)
    };
    let mut last = Expr::raw("null");
    for (i, name) in names.iter().enumerate() {
        if i > 0 {
            out.push(Stmt::new(line, StmtKind::Expr(last)));
        }
        let field = |e: &Expr| Expr::raw(format!("{}.{name}", crate::emit::render_expr(e)));
        last = Expr::Assign(Box::new(field(&target)), "=", Box::new(field(&value)));
    }
    Some(last)
}
