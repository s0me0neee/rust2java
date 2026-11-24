// Value-position expressions. Control flow in value position is hoisted into `out`.

use ra_ap_hir::PathResolution;
use ra_ap_syntax::{AstNode, AstToken, ast, ast::HasArgList, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, calls, items, iter, macros, pat, std_map};
use crate::jir::{Expr, LambdaBody, Stmt, StmtKind, java_char_literal, java_string_literal};
use crate::ty::{Int, Ty};

pub fn lower_expr(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    let v = lower_expr_inner(cx, e, out);
    // A `Copy` struct/array copied out of a place gets its own object if anyone changes one in place.
    super::values::copy_of(cx, e, v)
}

fn lower_expr_inner(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    match e {
        ast::Expr::Literal(l) => lower_literal(cx, l, e),
        ast::Expr::ParenExpr(p) => match p.expr() {
            Some(inner) => lower_expr(cx, &inner, out),
            None => Expr::raw("()"),
        },
        ast::Expr::PathExpr(p) => lower_path_expr(cx, p),
        ast::Expr::BinExpr(b) => lower_bin(cx, b, out),
        ast::Expr::PrefixExpr(p) => lower_prefix(cx, p, out),
        ast::Expr::RefExpr(r) if super::mutret::is_returned_arg(cx, r) => match r.expr() {
            Some(inner) => lower_expr(cx, &inner, out),
            None => Expr::raw("null"),
        },
        ast::Expr::RefExpr(r) => match r.expr() {
            // `&mut cell` passes the array itself so the callee can write through it.
            Some(inner) if r.mut_token().is_some() && cx.cell_path(&inner).is_some() => {
                Expr::name(cx.cell_path(&inner).unwrap())
            }
            // A `Cell`/`RefCell` handle is already the shared array.
            Some(inner) if cx.handle_path(&inner).is_some() => lower_expr(cx, &inner, out),
            Some(inner) if std_map::ptr::bare_cell_field(cx, &inner) => cx.unsupported(
                r.syntax(),
                "reference to a `Cell`/`RefCell` field (its Java field is a plain value)",
            ),
            Some(inner) if r.mut_token().is_some() && super::is_mut_ref_cell_ty(&cx.ty(&inner)) => {
                match write_back_cell(cx, r, &inner, out) {
                    Some(cell) => cell,
                    None => cx.unsupported(
                        r.syntax(),
                        "`&mut` to a field/element of immutable Java type",
                    ),
                }
            }
            Some(inner) => {
                // Simple mode: `&[1, 2]` / `&vec![..]` is only read, so `List.of(1, 2)`.
                cx.read_only_literal = crate::mode::simple()
                    && r.mut_token().is_none()
                    && super::stmt::is_list_literal(&inner);
                let v = lower_expr(cx, &inner, out);
                cx.read_only_literal = false;
                v
            }
            None => Expr::raw("null"),
        },
        ast::Expr::CallExpr(c) => calls::lower_call(cx, c, out),
        ast::Expr::MethodCallExpr(m) => calls::lower_method_call(cx, m, out),
        ast::Expr::MacroExpr(m) if macros::is_diverging(m) => {
            cx.unsupported(m.syntax(), "diverging macro in expression")
        }
        ast::Expr::MacroExpr(m) => macros::lower_macro_expr(cx, m, out),
        ast::Expr::FieldExpr(f) => lower_field(cx, f, out),
        ast::Expr::IndexExpr(i) => lower_index(cx, i, out),
        ast::Expr::CastExpr(c) => lower_cast(cx, c, out),
        ast::Expr::ClosureExpr(c) => lower_closure(cx, c, out),
        ast::Expr::TupleExpr(t) => {
            let ty = cx.ty(e);
            let args: Vec<Expr> = t
                .fields()
                .map(|f| {
                    let v = lower_expr(cx, &f, out);
                    // `Map.entry(x, 1)` can't be a `Map.Entry<Integer, Short>`.
                    match cx.ty(&f).strip_refs() {
                        Ty::Int(i)
                            if matches!(i.java(), "short" | "byte")
                                && !matches!(v, Expr::Cast(..)) =>
                        {
                            Expr::Cast(i.java().into(), Box::new(v))
                        }
                        _ => v,
                    }
                })
                .collect();
            tuple_expr(cx, &ty, args)
        }
        ast::Expr::ArrayExpr(a) => std_map::lower_array_expr(cx, a, out),
        ast::Expr::RecordExpr(r) => items::lower_record_expr(cx, r, out),
        ast::Expr::RangeExpr(r) => iter::lower_range_expr(cx, r, out),
        ast::Expr::TryExpr(t) => pat::lower_try(cx, t, out),
        ast::Expr::IfExpr(i) if is_simple_if(i) => lower_simple_if(cx, i, out),
        ast::Expr::LetExpr(_) => cx.unsupported(e.syntax(), "`let` expression outside if/while"),
        ast::Expr::ReturnExpr(_) | ast::Expr::BreakExpr(_) | ast::Expr::ContinueExpr(_) => {
            // Diverging in value position: emit as a statement; the value is never used.
            stmt::lower_to_dest(cx, e, &Dest::Discard, out);
            Expr::raw("null")
        }
        ast::Expr::IfExpr(_)
        | ast::Expr::MatchExpr(_)
        | ast::Expr::BlockExpr(_)
        | ast::Expr::LoopExpr(_)
        | ast::Expr::WhileExpr(_)
        | ast::Expr::ForExpr(_) => {
            if let ast::Expr::BlockExpr(b) = e
                && let Some(tail) = simple_block_tail(b)
            {
                return lower_expr(cx, &tail, out);
            }
            // `{ let s = f(); g(s) }` (typical of macro expansions): the lets become plain
            // statements before this one, the tail is the value.
            if let ast::Expr::BlockExpr(b) = e
                && let Some((lets, tail)) = let_block(b)
            {
                cx.push_scope();
                for l in &lets {
                    stmt::lower_let(cx, l, out);
                }
                let v = lower_expr(cx, &tail, out);
                cx.pop_scope();
                return v;
            }
            if let ast::Expr::MatchExpr(m) = e
                && let Some(sw) = pat::try_match_expr(cx, m, out)
            {
                return sw;
            }
            hoist(cx, e, out)
        }
        _ => cx.unsupported(e.syntax(), "expression kind"),
    }
}

// Lowers `e` where a value of type `want` is expected (adds widening casts etc. if needed).
pub fn lower_expr_as(cx: &mut Cx, e: &ast::Expr, want: &Ty, out: &mut Vec<Stmt>) -> Expr {
    let v = lower_expr(cx, e, out);
    let have = cx.ty(e);
    coerce(v, &have, want)
}

// A declared local's type target-types its initializer, so javac infers a bare `Optional.empty()`.
pub fn unwitness_empty(v: Expr) -> Expr {
    match v {
        Expr::Ternary(c, a, b) => Expr::Ternary(
            c,
            Box::new(unwitness_empty(*a)),
            Box::new(unwitness_empty(*b)),
        ),
        Expr::Raw(s) if s.starts_with("Optional.<") && s.ends_with(">empty()") => {
            Expr::raw("Optional.empty()")
        }
        _ => v,
    }
}

pub fn coerce(v: Expr, have: &Ty, want: &Ty) -> Expr {
    match (have.strip_refs(), want.strip_refs()) {
        // Java won't implicitly narrow int-typed expressions into byte/short.
        (Ty::Int(a), Ty::Int(b))
            if a.java() != b.java() && matches!(b.java(), "byte" | "short") =>
        {
            if matches!(v, Expr::Raw(ref s) if s.chars().all(|c| c.is_ascii_digit())) {
                v
            } else {
                Expr::Cast(b.java().into(), Box::new(v))
            }
        }
        _ => v,
    }
}

// Declares a temporary, lowers control flow into it, and yields its name.
pub fn hoist(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(e.syntax());
    let ty = cx.ty(e);
    if ty.is_unit() {
        stmt::lower_to_dest(cx, e, &Dest::Discard, out);
        return Expr::raw("null");
    }
    let hint = super::name_hint(e)
        .or_else(|| super::context_hint(e))
        .unwrap_or_else(|| "tmp".into());
    let tmp = cx.fresh(&hint);
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: stmt::decl_type_explicit(&ty),
            name: tmp.clone(),
            init: None,
        },
    ));
    stmt::lower_to_dest(cx, e, &Dest::Assign(tmp.clone()), out);
    Expr::name(tmp)
}

// A plain block made only of `let name = value;` statements and a tail.
fn let_block(b: &ast::BlockExpr) -> Option<(Vec<ast::LetStmt>, ast::Expr)> {
    if b.label().is_some()
        || b.unsafe_token().is_some()
        || b.async_token().is_some()
        || b.const_token().is_some()
    {
        return None;
    }
    let list = b.stmt_list()?;
    let lets = list
        .statements()
        .map(|s| match s {
            ast::Stmt::LetStmt(l)
                if l.let_else().is_none()
                    && l.initializer().is_some()
                    && matches!(l.pat(), Some(ast::Pat::IdentPat(_))) =>
            {
                Some(l)
            }
            _ => None,
        })
        .collect::<Option<Vec<_>>>()?;
    Some((lets, list.tail_expr()?))
}

fn simple_block_tail(b: &ast::BlockExpr) -> Option<ast::Expr> {
    if b.label().is_some()
        || b.unsafe_token().is_some()
        || b.async_token().is_some()
        || b.const_token().is_some()
    {
        return None;
    }
    let list = b.stmt_list()?;
    if list.statements().next().is_some() {
        return None;
    }
    list.tail_expr()
}

// `if c { a } else { b }` whose branches are single expressions becomes a ternary.
pub fn is_simple_if(i: &ast::IfExpr) -> bool {
    let cond_ok = i.condition().is_some_and(|c| !contains_let(&c));
    let then_ok = i
        .then_branch()
        .is_some_and(|b| simple_block_tail(&b).is_some_and(|t| is_plain(&t)));
    let else_ok = match i.else_branch() {
        Some(ast::ElseBranch::Block(b)) => simple_block_tail(&b).is_some_and(|t| is_plain(&t)),
        Some(ast::ElseBranch::IfExpr(ei)) => is_simple_if(&ei),
        None => false,
    };
    cond_ok && then_ok && else_ok
}

fn is_plain(e: &ast::Expr) -> bool {
    !matches!(
        e,
        ast::Expr::IfExpr(_)
            | ast::Expr::MatchExpr(_)
            | ast::Expr::BlockExpr(_)
            | ast::Expr::LoopExpr(_)
            | ast::Expr::WhileExpr(_)
            | ast::Expr::ForExpr(_)
            | ast::Expr::ReturnExpr(_)
            | ast::Expr::BreakExpr(_)
            | ast::Expr::ContinueExpr(_)
            | ast::Expr::TryExpr(_)
    ) && !matches!(e, ast::Expr::MacroExpr(m) if macros::is_diverging(m))
}

pub fn contains_let(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::LetExpr(_) => true,
        ast::Expr::BinExpr(b) => {
            b.lhs().is_some_and(|l| contains_let(&l)) || b.rhs().is_some_and(|r| contains_let(&r))
        }
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|x| contains_let(&x)),
        _ => false,
    }
}

fn lower_simple_if(cx: &mut Cx, i: &ast::IfExpr, out: &mut Vec<Stmt>) -> Expr {
    let c = lower_expr(cx, &i.condition().unwrap(), out);
    let a = lower_expr(
        cx,
        &simple_block_tail(&i.then_branch().unwrap()).unwrap(),
        out,
    );
    let b = match i.else_branch().unwrap() {
        ast::ElseBranch::Block(b) => lower_expr(cx, &simple_block_tail(&b).unwrap(), out),
        ast::ElseBranch::IfExpr(ei) => lower_simple_if(cx, &ei, out),
    };
    // javac types a bare `Optional.empty()` branch as Optional<Object>; give it a witness.
    let ty = cx.ty(&ast::Expr::IfExpr(i.clone()));
    let witness = |e: Expr| match (&e, ty.strip_refs()) {
        (Expr::Raw(s), Ty::Option(t)) if s == "Optional.empty()" => {
            Expr::raw(format!("Optional.<{}>empty()", t.boxed()))
        }
        _ => e,
    };
    Expr::Ternary(Box::new(c), Box::new(witness(a)), Box::new(witness(b)))
}

fn lower_literal(cx: &mut Cx, l: &ast::Literal, e: &ast::Expr) -> Expr {
    let ty = cx.ty(e);
    literal_expr(cx, l, &ty)
}

// Java literal for a Rust literal of type `ty`; pattern lowering uses this for case labels too.
pub fn literal_expr(cx: &mut Cx, l: &ast::Literal, ty: &Ty) -> Expr {
    let node = l.syntax();
    match l.kind() {
        ast::LiteralKind::IntNumber(n) => {
            let digits = n.text().to_string();
            let (body, _) = split_suffix(&digits);
            let body = body.trim_end_matches('_').to_string();
            match *ty {
                Ty::F32 => Expr::raw(format!("{body}.0f")),
                Ty::F64 => Expr::raw(format!("{body}.0")),
                Ty::Int(i) if i.is_long() => Expr::raw(
                    std_map::unsigned_literal(i, &body).unwrap_or_else(|| format!("{body}L")),
                ),
                Ty::Int(i) if i.is_unsigned() => {
                    Expr::raw(std_map::unsigned_literal(i, &body).unwrap_or(body))
                }
                _ => Expr::raw(body),
            }
        }
        ast::LiteralKind::FloatNumber(n) => {
            let text = n.text().to_string();
            let body = text
                .trim_end_matches("f64")
                .trim_end_matches("f32")
                .trim_end_matches('_');
            let mut body = body.to_string();
            if body.ends_with('.') {
                body.push('0');
            }
            if *ty == Ty::F32 {
                body.push('f');
            }
            Expr::raw(body)
        }
        ast::LiteralKind::Bool(b) => Expr::raw(if b { "true" } else { "false" }),
        ast::LiteralKind::Char(c) => match c.value() {
            Ok(ch) => Expr::raw(java_char_literal(ch)),
            Err(_) => cx.unsupported(node, "char literal"),
        },
        ast::LiteralKind::Byte(b) => match b.value() {
            Ok(v) if v.is_ascii_graphic() || v == b' ' => {
                Expr::raw(format!("(int) {}", java_char_literal(v as char)))
            }
            Ok(v) => Expr::raw(v.to_string()),
            Err(_) => cx.unsupported(node, "byte literal"),
        },
        ast::LiteralKind::String(s) => match s.value() {
            Ok(v) => Expr::raw(java_string_literal(&v)),
            Err(_) => cx.unsupported(node, "string literal"),
        },
        // `b"ab"` is a byte slice: the bytes are already `u8` (Java `int`) values.
        ast::LiteralKind::ByteString(b) => match b.value() {
            Ok(bytes) => {
                let items: Vec<Expr> = bytes.iter().map(|v| Expr::raw(v.to_string())).collect();
                if items.is_empty() {
                    // No elements to infer from: name the type so `b""` isn't a raw list.
                    Expr::New {
                        ty: "ArrayList<Integer>".into(),
                        args: vec![],
                    }
                } else {
                    Expr::New {
                        ty: "ArrayList<>".into(),
                        args: vec![Expr::call(Expr::raw("List"), "of", items)],
                    }
                }
            }
            Err(_) => cx.unsupported(node, "byte string literal"),
        },
        ast::LiteralKind::CString(_) => cx.unsupported(node, "C string literal"),
    }
}

fn split_suffix(s: &str) -> (&str, &str) {
    for suf in [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
        "f32", "f64",
    ] {
        if let Some(b) = s.strip_suffix(suf)
            && !s.starts_with("0x")
        {
            return (b, suf);
        }
        if s.starts_with("0x")
            && let Some(b) = s.strip_suffix(suf)
            && suf.starts_with(['i', 'u'])
        {
            return (b, suf);
        }
    }
    (s, "")
}

pub fn lower_path_expr(cx: &mut Cx, p: &ast::PathExpr) -> Expr {
    let Some(path) = p.path() else {
        return Expr::raw("null");
    };
    let text = path.syntax().text().to_string();
    if text == "self" {
        return Expr::raw(cx.self_name.clone());
    }
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Local(local)) => {
            if let Some((list, idx, kind)) = cx.elem_aliases.get(&local) {
                return match kind {
                    super::ElemAliasKind::Entry => {
                        Expr::call(Expr::name(list.clone()), "getValue", vec![])
                    }
                    super::ElemAliasKind::Char => Expr::call(
                        Expr::name(list.clone()),
                        "charAt",
                        vec![Expr::name(idx.clone())],
                    ),
                    _ => Expr::call(
                        Expr::name(list.clone()),
                        "get",
                        vec![Expr::name(idx.clone())],
                    ),
                };
            }
            let name = cx.lookup(local.name(cx.db).as_str());
            if let Some(view) = cx.views.get(&local) {
                return super::views::view_read(view);
            }
            if cx.builders.contains(&local) {
                return Expr::call(Expr::name(name), "toString", vec![]);
            }
            if cx.cell_handles.contains(&local) {
                Expr::name(name)
            } else if cx.cells.contains(&local) {
                Expr::raw(format!("{name}[0]"))
            } else {
                Expr::name(name)
            }
        }
        Some(PathResolution::Def(def)) => items::lower_def_path(cx, &path, def),
        Some(PathResolution::SelfType(_)) => Expr::raw(cx.self_ty_java.clone()),
        _ => cx.unsupported(p.syntax(), &format!("path `{text}`")),
    }
}

// `a + b` on a generic `T` holding a boxed number.
const GENERIC_ARITH: &str = "@SuppressWarnings(\"unchecked\") static <T> T arith(T a, char op, T b) { \
Object r = switch (a) { \
case Integer x -> { int y = (Integer) b; yield switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
case Long x -> { long y = (Long) b; yield switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
case Double x -> { double y = (Double) b; yield switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
case Float x -> { float y = (Float) b; yield switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
case Short x -> { short y = (Short) b; yield (short) switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
case Byte x -> { byte y = (Byte) b; yield (byte) switch (op) { case '+' -> x + y; case '-' -> x - y; case '*' -> x * y; case '/' -> x / y; default -> x % y; }; } \
default -> throw new UnsupportedOperationException(op + \" on \" + a.getClass().getSimpleName()); }; \
return (T) r; }";

fn lower_bin(cx: &mut Cx, b: &ast::BinExpr, out: &mut Vec<Stmt>) -> Expr {
    use ast::{BinaryOp, CmpOp, LogicOp};
    let (Some(lhs), Some(rhs), Some(op)) = (b.lhs(), b.rhs(), b.op_kind()) else {
        return cx.unsupported(b.syntax(), "binary expression");
    };
    if let BinaryOp::Assignment { op: aop } = op {
        return lower_assign(cx, &lhs, &rhs, aop, out);
    }
    let lt = cx.ty(&lhs);
    let rt = cx.ty(&rhs);
    // `T: Add<Output = T>`: Java has no operators on a type variable, so a helper switches on the
    // boxed number it holds.
    if let BinaryOp::ArithOp(a) = op
        && matches!(lt.strip_refs(), Ty::Param(_))
    {
        let sym = match a {
            ast::ArithOp::Add => '+',
            ast::ArithOp::Sub => '-',
            ast::ArithOp::Mul => '*',
            ast::ArithOp::Div => '/',
            ast::ArithOp::Rem => '%',
            _ => return cx.unsupported(b.syntax(), "bit operation on a generic type parameter"),
        };
        let l = lower_expr(cx, &lhs, out);
        let r = lower_expr(cx, &rhs, out);
        super::fmt::file_helper(cx, GENERIC_ARITH.into());
        return Expr::static_call("arith", vec![l, Expr::raw(format!("'{sym}'")), r]);
    }
    let l = lower_expr(cx, &lhs, out);
    // `a && <rhs needing statements>`: the statements may only run when `a` allows it
    // (`i < v.len() && matches!(v[i], ..)` must not index first).
    if let BinaryOp::LogicOp(lop) = op {
        let mut rhs_out = Vec::new();
        let r = lower_expr(cx, &rhs, &mut rhs_out);
        if rhs_out.is_empty() {
            out.extend(rhs_out);
            let sym = if lop == LogicOp::And { "&&" } else { "||" };
            return Expr::bin(l, sym, r);
        }
        let line = cx.line(b.syntax());
        let t = cx.fresh("cond");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "boolean".into(),
                name: t.clone(),
                init: Some(l),
            },
        ));
        rhs_out.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::Assign(Box::new(Expr::name(&t)), "=", Box::new(r))),
        ));
        let cond = if lop == LogicOp::And {
            Expr::name(&t)
        } else {
            Expr::Unary("!", Box::new(Expr::name(&t)))
        };
        out.push(Stmt::new(
            line,
            StmtKind::If {
                cond,
                then: crate::jir::Block {
                    stmts: rhs_out,
                    close_line: line,
                },
                els: None,
            },
        ));
        return Expr::name(t);
    }
    let r = lower_expr(cx, &rhs, out);
    // `Cell`/`RefCell` handles compare their contents, not the backing arrays.
    let l = std_map::ptr::content_value(cx, &lhs, l);
    let r = std_map::ptr::content_value(cx, &rhs, r);
    match op {
        BinaryOp::LogicOp(LogicOp::And) => Expr::bin(l, "&&", r),
        BinaryOp::LogicOp(LogicOp::Or) => Expr::bin(l, "||", r),
        BinaryOp::CmpOp(CmpOp::Eq { negated }) => {
            if is_ordering(&lt) || is_ordering(&rt) {
                // `compareTo` may return any difference; only its sign is the Ordering.
                let (l, r) = ordering_operands(l, &lt, r, &rt);
                Expr::bin(l, if negated { "!=" } else { "==" }, r)
            } else if lt.is_primitive() && rt.is_primitive() {
                // Two boxed operands (`list.get(i) == list.get(j)`) would compare references.
                let l = if maybe_boxed(&l) && maybe_boxed(&r) {
                    Expr::Cast(lt.strip_refs().java(), Box::new(l))
                } else {
                    l
                };
                Expr::bin(l, if negated { "!=" } else { "==" }, r)
            } else {
                // Simple mode: `v.get(i) == Some(c)` is `i < v.size() && v.get(i) == c`.
                let of = |e: &Expr| match e {
                    Expr::Call { recv: Some(o), name, args }
                        if matches!(&**o, Expr::Raw(s) if s == "Optional") && name.ends_with("of") && args.len() == 1 =>
                    {
                        Some(args[0].clone())
                    }
                    _ => None,
                };
                let eq = match (crate::mode::simple(), std_map::guarded_some(&l), of(&r), lt.strip_refs()) {
                    (true, Some((cond, v)), Some(c), Ty::Option(inner)) => {
                        let same = if inner.is_primitive() {
                            let v = if maybe_boxed(&v) && maybe_boxed(&c) {
                                Expr::Cast(inner.strip_refs().java(), Box::new(v))
                            } else {
                                v
                            };
                            Expr::bin(v, "==", c)
                        } else {
                            Expr::call(v, "equals", vec![c])
                        };
                        Expr::bin(cond, "&&", same)
                    }
                    _ => Expr::call(l, "equals", vec![r]),
                };
                if negated {
                    Expr::Unary("!", Box::new(eq))
                } else {
                    eq
                }
            }
        }
        BinaryOp::CmpOp(CmpOp::Ord { ordering, strict }) => {
            let sym = match (ordering, strict) {
                (ast::Ordering::Less, true) => "<",
                (ast::Ordering::Less, false) => "<=",
                (ast::Ordering::Greater, true) => ">",
                (ast::Ordering::Greater, false) => ">=",
            };
            if is_ordering(&lt) || is_ordering(&rt) {
                let (l, r) = ordering_operands(l, &lt, r, &rt);
                Expr::bin(l, sym, r)
            } else if let Some(cmp) = std_map::compare_expr(&lt, l.clone(), r.clone()) {
                Expr::bin(cmp, sym, Expr::raw("0"))
            } else if lt.is_primitive() {
                Expr::bin(l, sym, r)
            } else {
                // `impl<T: Ord> Bag<T>`: the bound isn't on the Java class's `T`, so cast for compareTo.
                let l = match lt.strip_refs() {
                    Ty::Param(t) if impl_declared_param(cx, &lhs) => {
                        Expr::Cast(format!("Comparable<{t}>"), Box::new(l))
                    }
                    _ => l,
                };
                let cmp = std_map::coll::ord_compare(cx, lt.strip_refs(), l, r);
                Expr::bin(cmp, sym, Expr::raw("0"))
            }
        }
        BinaryOp::ArithOp(a) => {
            if let Ty::Adt { .. } = lt.strip_refs() {
                return Expr::call(l, op_trait_method(a), vec![r]);
            }
            if matches!(rt.strip_refs(), Ty::Adt { .. }) {
                // `impl Mul<V2> for f64` would need a free function; keep it explicit.
                return cx.unsupported(b.syntax(), "a user type on the right of an operator");
            }
            if let Some(e) = std_map::unsigned_arith(&lt, a, l.clone(), r.clone()) {
                return e;
            }
            let sym = arith_symbol(a, &lt);
            // A product landing in a 64-bit `usize` total must not overflow as int first.
            let l = if cx.wide.ops.contains(b.syntax()) {
                Expr::Cast("long".into(), Box::new(l))
            } else {
                l
            };
            let v = Expr::bin(l, sym, r);
            narrow_if_needed(v, &cx.ty(&ast::Expr::BinExpr(b.clone())))
        }
        BinaryOp::Assignment { .. } => unreachable!(),
    }
}

// Calls that hand back a collection element or other boxed value.
fn maybe_boxed(e: &Expr) -> bool {
    match e {
        Expr::Call { name, .. } => {
            matches!(
                name.as_str(),
                "get"
                    | "getKey"
                    | "getValue"
                    | "getFirst"
                    | "getLast"
                    | "peek"
                    | "peekFirst"
                    | "peekLast"
                    | "next"
                    | "orElseThrow"
                    | "unwrap"
                    | "removeLast"
                    | "removeFirst"
            ) || name.starts_with('_')
        }
        _ => false,
    }
}

// `std::cmp::Ordering` is an int whose sign is what matters.
fn is_ordering(t: &Ty) -> bool {
    matches!(t.strip_refs(), Ty::Ordering)
}

fn ordering_sign(e: Expr, t: &Ty) -> Expr {
    // An `Ordering` constant is already -1/0/1.
    let constant = matches!(&e, Expr::Raw(s) if s == "-1" || s == "0" || s == "1");
    if is_ordering(t) && !constant {
        std_map::signum(e)
    } else {
        e
    }
}

// Against 0 only the sign matters, so the raw int and the signum compare the same.
fn ordering_operands(l: Expr, lt: &Ty, r: Expr, rt: &Ty) -> (Expr, Expr) {
    let zero = |e: &Expr| matches!(e, Expr::Raw(s) if s == "0");
    if zero(&l) || zero(&r) {
        return (l, r);
    }
    (ordering_sign(l, lt), ordering_sign(r, rt))
}

pub fn arith_symbol(a: ast::ArithOp, lt: &Ty) -> &'static str {
    use ast::ArithOp::*;
    match a {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        Shl => "<<",
        Shr => {
            if matches!(lt.strip_refs(), Ty::Int(i) if i.is_unsigned()) {
                ">>>"
            } else {
                ">>"
            }
        }
        BitXor => "^",
        BitOr => "|",
        BitAnd => "&",
    }
}

fn op_trait_method(a: ast::ArithOp) -> &'static str {
    use ast::ArithOp::*;
    match a {
        Add => "add",
        Sub => "sub",
        Mul => "mul",
        Div => "div",
        Rem => "rem",
        Shl => "shl",
        Shr => "shr",
        BitXor => "bitxor",
        BitOr => "bitor",
        BitAnd => "bitand",
    }
}

// A `&mut` binding aliasing an `Option` place: `if let Some(ref mut x) = o`,
// `if let Some(x) = o.as_mut()`, or `let x = o.get_or_insert(v)`.
fn option_ref_place(cx: &mut Cx, inner: &ast::Expr, out: &mut Vec<Stmt>) -> Option<(Expr, Ty)> {
    let local = cx.local_of_path(inner)?;
    let scope = cx.enclosing_fn(inner.syntax())?;
    let ip = cx
        .expanded_descendants(&scope)
        .into_iter()
        .filter_map(ast::IdentPat::cast)
        .find(|ip| cx.sema.to_def(ip) == Some(local))?;
    let ty = cx.pat_ty(&ast::Pat::IdentPat(ip.clone()));
    if !matches!(ty, Ty::Ref { mutable: true, .. }) {
        return None;
    }
    let init = ip.syntax().ancestors().find_map(|n| {
        if let Some(l) = ast::LetStmt::cast(n.clone()) {
            return l.initializer();
        }
        if let Some(le) = ast::LetExpr::cast(n) {
            return le.expr();
        }
        None
    })?;
    let target_ast = match &init {
        ast::Expr::MethodCallExpr(mc) => match mc.name_ref()?.text().to_string().as_str() {
            "as_mut" | "get_or_insert" | "get_or_insert_with" | "insert" => mc.receiver()?,
            _ => return None,
        },
        e if ip.ref_token().is_some() && ip.mut_token().is_some() => e.clone(),
        _ => return None,
    };
    if !plain_place(&target_ast) {
        return None;
    }
    let inner_ty = match cx.ty(&target_ast).strip_refs() {
        Ty::Option(t) => (**t).clone(),
        _ => return None,
    };
    let target = lower_expr(cx, &target_ast, out);
    Some((target, inner_ty))
}

// The Option place of `o.get_or_insert(..)` / `get_or_insert_with` / `insert` when its value is
// immutable in Java (writes must replace the Optional's content).
fn option_fill_target(cx: &Cx, call: &ast::MethodCallExpr) -> Option<ast::Expr> {
    let name = call.name_ref()?.text().to_string();
    if !matches!(
        name.as_str(),
        "get_or_insert" | "get_or_insert_with" | "insert"
    ) {
        return None;
    }
    let o = call.receiver()?;
    match cx.ty(&o).strip_refs() {
        Ty::Option(t) if super::is_cell_value_ty(t) && plain_place(&o) => Some(o),
        _ => None,
    }
}

fn option_write(
    cx: &mut Cx,
    o: ast::Expr,
    lhs: &ast::Expr,
    sym: &'static str,
    r: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let elem_ty = match cx.ty(&o).strip_refs() {
        Ty::Option(t) => (**t).clone(),
        _ => Ty::Other("?".into()),
    };
    let target = lower_expr(cx, &o, out);
    let value = if sym == "=" {
        r
    } else {
        let cur = Expr::call(target.clone(), "get", vec![]);
        narrow_if_needed(
            Expr::bin(cur, static_op(sym.trim_end_matches('=')), r),
            &elem_ty,
        )
    };
    let _ = lhs;
    place_assign(
        cx,
        &o,
        "=",
        Expr::call(Expr::raw("Optional"), "of", vec![value]),
        out,
    )
}

fn plain_place(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::PathExpr(_) => true,
        ast::Expr::FieldExpr(f) => f.expr().is_some_and(|b| plain_place(&b)),
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|i| plain_place(&i)),
        _ => false,
    }
}

// Java promotes byte/short arithmetic to int; cast back so the value fits its Rust type.
pub fn narrow_if_needed(v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Int(i @ (Int::I8 | Int::I16 | Int::U16)) => Expr::Cast(i.java().into(), Box::new(v)),
        _ => v,
    }
}

fn lower_assign(
    cx: &mut Cx,
    lhs: &ast::Expr,
    rhs: &ast::Expr,
    op: Option<ast::ArithOp>,
    out: &mut Vec<Stmt>,
) -> Expr {
    // `*m.entry(k).or_insert(v) += x` is a map update, not an assignment to a value.
    if let Some(e) = std_map::lower_entry_assign(cx, lhs, op, rhs, out) {
        return e;
    }
    if op.is_none()
        && let Some(view) = cx.view_of(lhs)
        && let Some(e) = super::views::view_assign(cx, &view, rhs, out)
    {
        return e;
    }
    let lt = cx.ty(lhs);
    let r = lower_expr_as(cx, rhs, &lt, out);
    let sym: &'static str = match op {
        None => "=",
        Some(a) => match arith_symbol(a, &lt) {
            "+" => "+=",
            "-" => "-=",
            "*" => "*=",
            "/" => "/=",
            "%" => "%=",
            "<<" => "<<=",
            ">>" => ">>=",
            ">>>" => ">>>=",
            "^" => "^=",
            "|" => "|=",
            _ => "&=",
        },
    };
    if let (Some(a), Ty::Adt { .. }) = (op, lt.strip_refs()) {
        let place = lower_expr(cx, lhs, out);
        let m = format!("{}_assign", op_trait_method(a));
        return Expr::call(place, m, vec![r]);
    }
    place_assign(cx, lhs, sym, r, out)
}

// Assignment to a Rust place expression. Index places use `set`/`put`.
pub fn place_assign(
    cx: &mut Cx,
    lhs: &ast::Expr,
    sym: &'static str,
    r: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    // `t.0 += 1`: Java tuples (`Map.Entry`, records) are immutable, so the place gets a new tuple.
    if let ast::Expr::FieldExpr(f) = super::strip_parens(lhs)
        && let Some(base) = f.expr()
        && let Ty::Tuple(ts) = cx.ty(&base).strip_refs().clone()
        && ts.len() >= 2
    {
        return tuple_field_assign(cx, lhs, &base, &f, &ts, sym, r, out);
    }
    // `o.get_or_insert_with(f).push(c)` on a `String`/number: the receiver was lowered (and
    // filled) by the caller; the new value goes back into the Optional.
    if let ast::Expr::MethodCallExpr(call) = super::strip_parens(lhs)
        && let Some(o) = option_fill_target(cx, &call)
    {
        return option_write(cx, o, lhs, sym, r, out);
    }
    if let Some(b) = cx.builder_path(lhs) {
        return match sym {
            "+=" => Expr::call(Expr::name(b), "append", vec![super::fmt::as_any_value(r)]),
            "=" => {
                let args = if is_empty_string_expr(&r) {
                    vec![]
                } else {
                    vec![r]
                };
                let fresh = Expr::New {
                    ty: "StringBuilder".into(),
                    args,
                };
                Expr::Assign(Box::new(Expr::name(b)), "=", Box::new(fresh))
            }
            _ => cx.unsupported(
                lhs.syntax(),
                "compound assignment to a StringBuilder string",
            ),
        };
    }
    let alias_place = match lhs {
        ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => p.expr(),
        ast::Expr::PathExpr(_) => Some(lhs.clone()),
        _ => None,
    };
    if let Some(local) = alias_place.and_then(|e| cx.local_of_path(&e))
        && let Some((list, idx, kind)) = cx.elem_aliases.get(&local).cloned()
    {
        let (list, idx) = (Expr::name(list), Expr::name(idx));
        let value = if sym == "=" {
            r
        } else {
            let elem_ty = cx.ty(lhs);
            let cur = match kind {
                super::ElemAliasKind::Entry => Expr::call(list.clone(), "getValue", vec![]),
                _ => Expr::call(list.clone(), "get", vec![idx.clone()]),
            };
            narrow_if_needed(
                Expr::bin(cur, static_op(sym.trim_end_matches('=')), r),
                &elem_ty,
            )
        };
        return match kind {
            super::ElemAliasKind::Entry => Expr::call(list, "setValue", vec![value]),
            super::ElemAliasKind::Map => Expr::call(list, "put", vec![idx, value]),
            super::ElemAliasKind::List => Expr::call(list, "set", vec![idx, value]),
            super::ElemAliasKind::Char => cx.unsupported(lhs.syntax(), "assignment to a string's char"),
        };
    }
    match lhs {
        ast::Expr::IndexExpr(ix) => {
            let (Some(base), Some(index)) = (ix.base(), ix.index()) else {
                return cx.unsupported(lhs.syntax(), "index assignment");
            };
            let bt = cx.ty(&base);
            let mut b = lower_expr(cx, &base, out);
            let i = lower_expr(cx, &index, out);
            // `impl Deref` newtypes assign through their target.
            let mut bt = bt.clone();
            if let Some(target) = super::traits::deref_target(cx, &bt) {
                b = Expr::call(b, "deref", vec![]);
                bt = target;
            }
            if matches!(bt.strip_refs(), Ty::Adt { .. }) {
                let value = if sym == "=" {
                    r
                } else {
                    let cur = Expr::call(b.clone(), "index", vec![i.clone()]);
                    let elem_ty = cx.ty(lhs);
                    narrow_if_needed(
                        Expr::bin(cur, static_op(sym.trim_end_matches('=')), r),
                        &elem_ty,
                    )
                };
                return Expr::call(b, "index_mut_set", vec![i, value]);
            }
            if matches!(bt.strip_refs(), Ty::Deque(_)) {
                return cx.unsupported(lhs.syntax(), "assigning through `VecDeque` index");
            }
            let setter = if matches!(bt.strip_refs(), Ty::Map { .. }) {
                "put"
            } else {
                "set"
            };
            let value = if sym == "=" {
                r
            } else {
                let op = sym.trim_end_matches('=');
                let getter = Expr::call(b.clone(), "get", vec![i.clone()]);
                let elem_ty = cx.ty(lhs);
                narrow_if_needed(Expr::bin(getter, static_op(op), r), &elem_ty)
            };
            Expr::call(b, setter, vec![i, value])
        }
        ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => {
            let inner = p.expr().unwrap();
            // `*n += 1` on a parameter the function returns (`mutret`): a plain local write.
            if cx
                .local_of_path(&inner)
                .is_some_and(|l| cx.returned_params.contains(&l))
            {
                let target = lower_expr(cx, &inner, out);
                return Expr::Assign(Box::new(target), sym, Box::new(r));
            }
            // `*o.get_or_insert(v) += 1`: fill `o`, then write the new value into it.
            if let ast::Expr::MethodCallExpr(call) = super::strip_parens(&inner)
                && let Some(o) = option_fill_target(cx, &call)
            {
                let _ = lower_expr(cx, &inner, out);
                return option_write(cx, o, lhs, sym, r, out);
            }
            if let Some((target, elem_ty)) = option_ref_place(cx, &inner, out) {
                let value = if sym == "=" {
                    r
                } else {
                    let cur = Expr::call(target.clone(), "get", vec![]);
                    narrow_if_needed(
                        Expr::bin(cur, static_op(sym.trim_end_matches('=')), r),
                        &elem_ty,
                    )
                };
                let value = narrow_if_needed(value, &elem_ty);
                return Expr::Assign(
                    Box::new(target),
                    "=",
                    Box::new(Expr::call(Expr::raw("Optional"), "of", vec![value])),
                );
            }
            if let ast::Expr::MethodCallExpr(m) = super::strip_parens(&inner)
                && m.name_ref().is_some_and(|n| n.text() == "borrow_mut")
                && m.receiver()
                    .is_some_and(|r| std_map::ptr::shared_object(cx, &r))
            {
                return cx.unsupported(
                    lhs.syntax(),
                    "replacing a shared object through `borrow_mut`",
                );
            }
            // A struct behind `&mut`: Java can't swap the object, so its fields are overwritten.
            let line = cx.line(lhs.syntax());
            if sym == "="
                && let Some(e) = super::values::assign_through(cx, &inner, r.clone(), line, out)
            {
                return e;
            }
            // A Java local holding a copy of an immutable value: the write would be lost.
            if cx.local_of_path(&inner).is_some()
                && cx.cell_path(&inner).is_none()
                && matches!(cx.ty(&inner), Ty::Ref { mutable: true, ref inner } if super::is_mut_ref_cell_ty(inner))
            {
                return cx.unsupported(
                    lhs.syntax(),
                    "writing through a `&mut` binding to a value Java can't share",
                );
            }
            if let Some(e) = std_map::lower_slot_assign(cx, &inner, sym, r.clone(), out) {
                return e;
            }
            if is_call_result(&inner)
                && matches!(cx.ty(&inner), Ty::Ref { mutable: true, ref inner } if super::is_mut_ref_cell_ty(inner))
            {
                return cx.unsupported(
                    lhs.syntax(),
                    "writing through a `&mut` returned by a call to a value Java can't share",
                );
            }
            let place = lower_expr(cx, &inner, out);
            Expr::Assign(
                Box::new(super::calls::deref_place(cx, &inner, place)),
                sym,
                Box::new(r),
            )
        }
        _ => {
            let place = lower_expr(cx, lhs, out);
            Expr::Assign(Box::new(place), sym, Box::new(r))
        }
    }
}

// A value produced by a call (`f()?`, `m.get_mut(k).unwrap()`), not a place Java could share.
pub fn is_call_result(e: &ast::Expr) -> bool {
    match super::strip_parens(e) {
        ast::Expr::CallExpr(_) | ast::Expr::MethodCallExpr(_) => true,
        ast::Expr::TryExpr(t) => t.expr().is_some_and(|i| is_call_result(&i)),
        _ => false,
    }
}

fn static_op(op: &str) -> &'static str {
    match op {
        "+" => "+",
        "-" => "-",
        "*" => "*",
        "/" => "/",
        "%" => "%",
        "<<" => "<<",
        ">>" => ">>",
        ">>>" => ">>>",
        "^" => "^",
        "|" => "|",
        _ => "&",
    }
}

fn lower_prefix(cx: &mut Cx, p: &ast::PrefixExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(inner) = p.expr() else {
        return cx.unsupported(p.syntax(), "prefix expression");
    };
    let ty = cx.ty(&inner);
    let v = lower_expr(cx, &inner, out);
    match p.op_kind() {
        Some(ast::UnaryOp::Neg) => {
            if let Ty::Adt { .. } = ty.strip_refs() {
                Expr::call(v, "neg", vec![])
            } else {
                Expr::Unary("-", Box::new(v))
            }
        }
        Some(ast::UnaryOp::Not) => {
            // `!view.is_empty()` on int offsets: `start < length` reads better than `!(start >= length)`.
            if let ast::Expr::MethodCallExpr(m) = super::strip_parens(&inner)
                && m.name_ref().is_some_and(|n| n.text() == "is_empty")
                && m.receiver().is_some_and(|r| cx.view_of(&r).is_some())
                && let Expr::Binary(a, ">=", b) = v
            {
                return Expr::Binary(a, "<", b);
            }
            if ty.strip_refs() == &Ty::Bool {
                Expr::Unary("!", Box::new(v))
            } else if matches!(ty.strip_refs(), Ty::Adt { .. }) {
                Expr::call(v, "not", vec![])
            } else {
                Expr::Unary("~", Box::new(v))
            }
        }
        Some(ast::UnaryOp::Deref) => {
            if let Some((target, _)) = option_ref_place(cx, &inner, out) {
                return Expr::call(target, "get", vec![]);
            }
            // `*c` on a `Cell`/`RefCell` handle: the array's only element.
            if std_map::ptr::is_array(cx, &inner).is_some() && std_map::ptr::is_handle(cx, &inner) {
                return Expr::raw(format!("{}[0]", crate::emit::render_expr(&v)));
            }
            // Cells (`n[0]`) and iter_mut aliases (`v.get(i)`) were resolved when lowering `inner`.
            let resolved = cx.cell_path(&inner).is_some()
                || cx.handle_path(&inner).is_some()
                || cx
                    .local_of_path(&inner)
                    .is_some_and(|l| cx.returned_params.contains(&l))
                || cx
                    .local_of_path(&inner)
                    .is_some_and(|l| cx.elem_aliases.contains_key(&l));
            if let Ty::Ref {
                mutable: true,
                inner: t,
            } = &ty
                && (t.is_primitive() || t.is_str())
                && !resolved
                && !is_call_result(&inner)
            {
                return cx.unsupported(p.syntax(), "deref of a `&mut` primitive/String (cell)");
            }
            super::calls::deref_place(cx, &inner, v)
        }
        None => cx.unsupported(p.syntax(), "prefix operator"),
    }
}

fn lower_field(cx: &mut Cx, f: &ast::FieldExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(base) = f.expr() else {
        return cx.unsupported(f.syntax(), "field access");
    };
    let bt = cx.ty(&base);
    let b = lower_expr(cx, &base, out);
    if let Some(idx) = f.index_token() {
        let i: usize = idx.text().parse().unwrap_or(0);
        if let Ty::Tuple(ts) = bt.strip_refs() {
            return tuple_field(b, i, ts);
        }
        return Expr::Field(Box::new(b), format!("_{i}"));
    }
    let name = f
        .name_ref()
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let name = if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) {
        // rust-analyzer reports `t.1` as a name_ref; tuples are Map.Entry/TupleN, not records.
        if let Ty::Tuple(ts) = bt.strip_refs() {
            return tuple_field(b, name.parse().unwrap_or(0), ts);
        }
        format!("_{name}")
    } else {
        super::escape_keyword(&name)
    };
    let field = Expr::Field(Box::new(b), name);
    // In `impl Wrap<i32>` the Java field is still `T value`: cast it to the concrete type.
    let generic_field = matches!(&base, ast::Expr::PathExpr(p) if p.syntax().text() == "self")
        && cx
            .sema
            .resolve_field(f)
            .and_then(|r| r.left())
            .is_some_and(|fd| fd.ty(cx.db).as_type_param(cx.db).is_some());
    let actual = cx.ty(&ast::Expr::FieldExpr(f.clone()));
    if generic_field && actual.is_nameable() && !matches!(actual.strip_refs(), Ty::Param(_)) {
        return Expr::Cast(actual.strip_refs().boxed(), Box::new(field));
    }
    field
}

pub fn tuple_field(b: Expr, i: usize, ts: &[Ty]) -> Expr {
    Expr::call(b, crate::tuples::accessor(ts, i), vec![])
}

pub fn tuple_expr(cx: &mut Cx, ty: &Ty, args: Vec<Expr>) -> Expr {
    if let Ty::Tuple(ts) = ty.strip_refs()
        && let Some(n) = crate::tuples::named(ts)
    {
        return Expr::New { ty: n.name, args };
    }
    match args.len() {
        0 => Expr::raw("null"),
        2 => Expr::Call {
            recv: Some(Box::new(Expr::raw("Map"))),
            name: "entry".into(),
            args,
        },
        n => {
            items::need_tuple(cx, n);
            let _ = ty;
            Expr::New {
                ty: format!("Tuple{n}<>"),
                args,
            }
        }
    }
}

// True for an index expression of the shape `v[v.len() - 1]`.
fn last_index(base: &ast::Expr, index: &ast::Expr) -> bool {
    let ast::Expr::BinExpr(b) = index else {
        return false;
    };
    if !matches!(b.op_kind(), Some(ast::BinaryOp::ArithOp(ast::ArithOp::Sub))) {
        return false;
    }
    let (Some(lhs), Some(rhs)) = (b.lhs(), b.rhs()) else {
        return false;
    };
    if rhs.syntax().text().to_string() != "1" {
        return false;
    }
    let ast::Expr::MethodCallExpr(m) = lhs else {
        return false;
    };
    m.name_ref().is_some_and(|n| n.text() == "len")
        && m.receiver()
            .is_some_and(|r| r.syntax().text().to_string() == base.syntax().text().to_string())
}

pub fn is_empty_string_expr(e: &Expr) -> bool {
    matches!(e, Expr::Raw(s) if s == "\"\"")
}

// `f(&mut node.left)` as a whole statement: the field goes into a one-element array for the call
// and is written back right after the statement (Rust's borrow keeps everything else off it).
fn write_back_cell(
    cx: &mut Cx,
    r: &ast::RefExpr,
    inner: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let call = cx.write_back_call.clone()?;
    let args = r
        .syntax()
        .parent()
        .filter(|p| ast::ArgList::can_cast(p.kind()))?;
    if cx.lambda_depth > 0 || args.parent()? != call || !stable_place(inner) {
        return None;
    }
    let line = cx.line(r.syntax());
    let elem = cx.ty(inner).strip_refs().java();
    let current = lower_expr(cx, inner, out);
    let base = match inner {
        ast::Expr::FieldExpr(f) => f
            .name_ref()
            .map(|n| n.text().to_string())
            .unwrap_or_else(|| "cell".into()),
        _ => "cell".into(),
    };
    let name = cx.fresh(&base);
    // Java can't create `Optional<T>[]`; the raw array is the usual workaround.
    let raw = elem.split('<').next().unwrap_or(&elem).to_string();
    let init = if raw == elem {
        format!("{{{}}}", crate::emit::render_expr(&current))
    } else {
        format!("new {raw}[]{{{}}}", crate::emit::render_expr(&current))
    };
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("{elem}[]"),
            name: name.clone(),
            init: Some(Expr::raw(init)),
        },
    ));
    let mut back = Vec::new();
    let assign = place_assign(cx, inner, "=", Expr::raw(format!("{name}[0]")), &mut back);
    back.push(Stmt::new(line, StmtKind::Expr(assign)));
    cx.write_backs.extend(back);
    Some(Expr::name(name))
}

#[allow(clippy::too_many_arguments)]
fn tuple_field_assign(
    cx: &mut Cx,
    lhs: &ast::Expr,
    base: &ast::Expr,
    f: &ast::FieldExpr,
    ts: &[Ty],
    sym: &'static str,
    r: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let index = f.name_ref().and_then(|n| n.text().parse::<usize>().ok());
    let through_ref = matches!(cx.ty(base), Ty::Ref { .. })
        && cx.cell_path(base).is_none()
        && cx
            .local_of_path(base)
            .is_some_and(|l| !cx.elem_aliases.contains_key(&l));
    let Some(index) = index.filter(|i| *i < ts.len() && stable_place(base) && !through_ref) else {
        return cx.unsupported(lhs.syntax(), "assigning a tuple field here");
    };
    let b = crate::emit::render_expr(&lower_expr(cx, base, out));
    let getter = |j: usize| Expr::raw(format!("{b}.{}()", crate::tuples::accessor(ts, j)));
    let fields = (0..ts.len())
        .map(|j| match j {
            _ if j != index => getter(j),
            _ if sym == "=" => r.clone(),
            _ => narrow_if_needed(
                Expr::bin(getter(j), static_op(sym.trim_end_matches('=')), r.clone()),
                &ts[j],
            ),
        })
        .collect();
    let rebuilt = tuple_expr(cx, &Ty::Tuple(ts.to_vec()), fields);
    place_assign(cx, base, "=", rebuilt, out)
}

// Whether `e`'s type is an impl's type parameter that is `Ord` only by the impl's bound: the
// Java class's own `T` (from the struct) then has no `Comparable` bound.
fn impl_declared_param(cx: &Cx, e: &ast::Expr) -> bool {
    let db = cx.db;
    let Some(p) = cx
        .sema
        .type_of_expr(e)
        .and_then(|t| t.original.strip_references().as_type_param(db))
    else {
        return false;
    };
    let ra_ap_hir::GenericDef::Impl(imp) = p.parent(db) else {
        return false;
    };
    let self_ty = imp.self_ty(db);
    let Some(adt) = self_ty.as_adt() else {
        return false;
    };
    let Some(pos) = self_ty
        .type_arguments()
        .position(|a| a.as_type_param(db) == Some(p))
    else {
        return false;
    };
    let ordered = |t: &ra_ap_hir::TypeParam| {
        t.trait_bounds(db)
            .iter()
            .any(|tr| matches!(tr.name(db).as_str(), "Ord" | "PartialOrd"))
    };
    let struct_param = ra_ap_hir::GenericDef::from(adt)
        .type_or_const_params(db)
        .into_iter()
        .filter_map(|tp| tp.as_type_param(db))
        .nth(pos);
    !struct_param.is_some_and(|sp| ordered(&sp))
}

// A place that can be read now and assigned after the call: fields/indexes over a local or `self`.
fn stable_place(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::PathExpr(_) => true,
        ast::Expr::FieldExpr(f) => f.expr().is_some_and(|b| stable_place(&b)),
        ast::Expr::IndexExpr(i) => {
            i.base().is_some_and(|b| stable_place(&b))
                && i.index()
                    .is_some_and(|ix| matches!(ix, ast::Expr::PathExpr(_) | ast::Expr::Literal(_)))
        }
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|x| stable_place(&x)),
        _ => false,
    }
}

// The string `s` of an `s.as_bytes()` expression (bytes and Java char codes agree for ASCII).
pub fn as_bytes_receiver(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let ast::Expr::MethodCallExpr(m) = super::strip_parens(e) else {
        return None;
    };
    let recv = m.receiver()?;
    (m.name_ref()?.text() == "as_bytes" && cx.ty(&recv).is_str()).then_some(recv)
}

fn lower_index(cx: &mut Cx, i: &ast::IndexExpr, out: &mut Vec<Stmt>) -> Expr {
    let (Some(base), Some(index)) = (i.base(), i.index()) else {
        return cx.unsupported(i.syntax(), "index expression");
    };
    let bt = cx.ty(&base);
    if let (ast::Expr::RangeExpr(r), Some(view)) = (&index, cx.view_of(&base)) {
        return super::views::view_slice(cx, &view, r, out);
    }
    // `s.as_bytes()[i]` reads one char code instead of copying the string into a list.
    if !matches!(index, ast::Expr::RangeExpr(_))
        && let Some(s) = as_bytes_receiver(cx, &base)
    {
        let s = lower_expr(cx, &s, out);
        let ix = lower_expr(cx, &index, out);
        return Expr::Cast("int".into(), Box::new(Expr::call(s, "charAt", vec![ix])));
    }
    // `v[v.len() - 1]` is the last element, which Java 21 spells `getLast()`.
    if last_index(&base, &index) && matches!(bt.strip_refs(), Ty::Vec(_) | Ty::Deque(_)) {
        let b = lower_expr(cx, &base, out);
        return Expr::call(b, "getLast", vec![]);
    }
    // `dq[0]`: ArrayDeque has no random access, but its ends are O(1).
    if matches!(bt.strip_refs(), Ty::Deque(_)) && index.syntax().text() == "0" {
        let b = lower_expr(cx, &base, out);
        return Expr::call(b, "getFirst", vec![]);
    }
    if let ast::Expr::RangeExpr(r) = &index {
        let b = lower_expr(cx, &base, out);
        return std_map::lower_slice_range(cx, b, &bt, r, out);
    }
    // `impl Deref` newtypes index through their target.
    if let Some(target) = super::traits::deref_target(cx, &bt) {
        let b = Expr::call(lower_expr(cx, &base, out), "deref", vec![]);
        let ix = lower_expr(cx, &index, out);
        return match target.strip_refs() {
            Ty::Str => Expr::call(b, "charAt", vec![ix]),
            _ => Expr::call(b, "get", vec![ix]),
        };
    }
    if matches!(bt.strip_refs(), Ty::Adt { .. }) {
        let b = lower_expr(cx, &base, out);
        let ix = lower_expr(cx, &index, out);
        return Expr::call(b, "index", vec![ix]);
    }
    if !matches!(
        bt.strip_refs(),
        Ty::Vec(_) | Ty::Deque(_) | Ty::Map { .. } | Ty::Str
    ) {
        return cx.unsupported(i.syntax(), "indexing this type");
    }
    let b = lower_expr(cx, &base, out);
    let ix = lower_expr(cx, &index, out);
    match bt.strip_refs() {
        Ty::Str => Expr::call(b, "charAt", vec![ix]),
        Ty::Deque(_) => {
            cx.warn(
                i.syntax(),
                "VecDeque index is O(i) in Java (ArrayDeque has no random access); use front/back or a Vec",
            );
            Expr::call(
                Expr::call(
                    Expr::call(Expr::call(b, "stream", vec![]), "skip", vec![ix]),
                    "findFirst",
                    vec![],
                ),
                "orElseThrow",
                vec![],
            )
        }
        _ => Expr::call(b, "get", vec![ix]),
    }
}

fn lower_cast(cx: &mut Cx, c: &ast::CastExpr, out: &mut Vec<Stmt>) -> Expr {
    let (Some(inner), Some(t)) = (c.expr(), c.ty()) else {
        return cx.unsupported(c.syntax(), "cast");
    };
    let from = if super::wide::is_long(cx, &cx.wide.locals, &cx.wide.ops, &inner) {
        Ty::Int(crate::ty::Int::I64)
    } else {
        cx.ty(&inner)
    };
    let to = cx.type_ref(&t);
    let v = lower_expr(cx, &inner, out);
    let (from, to) = (from.strip_refs().clone(), to.strip_refs().clone());
    // `ord as i32` is -1/0/1; a Java comparison result may be any difference.
    if from == Ty::Ordering && matches!(to, Ty::Int(_)) {
        return ordering_sign(v, &from);
    }
    // A boxed `Long` (`it.next()`) can't be narrowed to `int` in one cast (unbox, then widen is
    // fine): unbox it first.
    let rank = |t: &str| match t {
        "byte" => 1,
        "short" | "char" => 2,
        "int" => 3,
        "long" => 4,
        "float" => 5,
        "double" => 6,
        _ => 0,
    };
    let (fj, tj) = (from.java(), to.java());
    let narrowing = rank(&tj) > 0 && rank(&fj) > rank(&tj) || (tj == "char" && fj != "char");
    let unbox = maybe_boxed(&v) && from.is_primitive() && to.is_primitive() && narrowing;
    // Only a cast applied to the boxed value itself needs it (`(char) (x & 0xFF)` already unboxes).
    let fix = |e: Expr| match e {
        Expr::Cast(t, inner) if unbox && crate::emit::render_expr(&inner) == crate::emit::render_expr(&v) => {
            Expr::Cast(t, Box::new(Expr::Cast(fj.clone(), inner)))
        }
        e => e,
    };
    // `as` casts need the source type: `300i32 as u8` truncates even though both are Java `int`.
    if let Some(e) = std_map::num_cast(&from, &to, v.clone()) {
        return fix(e);
    }
    // `Box::new(|x| ..) as Box<dyn Fn(i32)>`: the cast gives javac the lambda's target type
    // (a closure returning this one can't be inferred otherwise).
    if matches!(to, Ty::Fn { .. }) && matches!(v, Expr::Lambda { .. }) {
        return Expr::Cast(to.java(), Box::new(v));
    }
    if from.java() == to.java() {
        return v;
    }
    match (&from, &to) {
        (Ty::Bool, Ty::Int(_)) => Expr::Ternary(
            Box::new(v),
            Box::new(Expr::raw("1")),
            Box::new(Expr::raw("0")),
        ),
        // Plain enums carry their Rust discriminant in a `value` field.
        (Ty::Adt { .. }, Ty::Int(_)) => Expr::Field(Box::new(v), "value".into()),
        _ => fix(Expr::Cast(to.java(), Box::new(v.clone()))),
    }
}

pub fn lower_closure(cx: &mut Cx, c: &ast::ClosureExpr, out: &mut Vec<Stmt>) -> Expr {
    std_map::ptr::note_fn(cx, c.syntax(), &cx.ty(&ast::Expr::ClosureExpr(c.clone())));
    // `move` takes immutable captures by value; Java lambdas would share the cell array.
    let mut copies = moved_cell_copies(cx, c, out);
    copies.extend(reassigned_snapshots(cx, c, out));
    cx.push_scope();
    for (rust, java) in copies {
        cx.bind_name(&rust, &java);
    }
    let before = cx.taken_names();
    cx.lambda_depth += 1;
    let e = lower_closure_inner(cx, c);
    cx.lambda_depth -= 1;
    cx.pop_scope();
    // Sibling lambdas may reuse `value`, `item`, ...: Java scopes each lambda's names to itself.
    cx.release_names_since(&before);
    e
}

fn moved_cell_copies(
    cx: &mut Cx,
    c: &ast::ClosureExpr,
    out: &mut Vec<Stmt>,
) -> Vec<(String, String)> {
    if c.move_token().is_none() {
        return Vec::new();
    }
    let line = cx.line(c.syntax());
    let mut seen = std::collections::HashSet::new();
    let mut copies = Vec::new();
    let nodes = cx.expanded_descendants(c.syntax());
    // A closure's own parameters and local bindings are not free captures, even if
    // they become cells when a nested closure captures them.
    let bound: std::collections::HashSet<_> = nodes
        .iter()
        .filter_map(|n| ast::IdentPat::cast(n.clone()))
        .filter_map(|p| cx.sema.to_def(&p))
        .collect();
    for pe in nodes.into_iter().filter_map(ast::PathExpr::cast) {
        let Some(local) = cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) else {
            continue;
        };
        if bound.contains(&local) || !cx.cells.contains(&local) || !seen.insert(local) {
            continue;
        }
        let ty = cx.convert(&local.ty(cx.db));
        // A `&mut` reference is copied as the same array; only plain values are copied.
        if matches!(ty, Ty::Ref { .. }) {
            continue;
        }
        let name = cx.lookup(local.name(cx.db).as_str());
        let copy = cx.fresh(&format!("{name}_moved"));
        // Java can't create a generic array (`Map.Entry<A, B>[]`); the raw one is the workaround.
        let elem = ty.java();
        let raw_elem = elem.split('<').next().unwrap_or(&elem);
        let init = if raw_elem == elem {
            format!("{{{name}[0]}}")
        } else {
            format!("new {raw_elem}[]{{{name}[0]}}")
        };
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: format!("{elem}[]"),
                name: copy.clone(),
                init: Some(Expr::raw(init)),
            },
        ));
        copies.push((local.name(cx.db).as_str().to_string(), copy));
    }
    copies
}

// A captured local that is reassigned elsewhere isn't effectively final, which Java lambdas need.
// Rust's borrow keeps it unchanged while the closure lives, so the lambda can read a snapshot.
fn reassigned_snapshots(
    cx: &mut Cx,
    c: &ast::ClosureExpr,
    out: &mut Vec<Stmt>,
) -> Vec<(String, String)> {
    let Some(scope) = cx.enclosing_fn(c.syntax()) else {
        return Vec::new();
    };
    let line = cx.line(c.syntax());
    let nodes = cx.expanded_descendants(c.syntax());
    let bound: std::collections::HashSet<_> = nodes
        .iter()
        .filter_map(|n| ast::IdentPat::cast(n.clone()))
        .filter_map(|p| cx.sema.to_def(&p))
        .collect();
    // `s.push(c)` on a String (or any `&mut self` call on a Java-immutable value) reassigns it.
    let assigned = |cx: &Cx, root: &ra_ap_syntax::SyntaxNode, local: ra_ap_hir::Local| {
        cx.expanded_descendants(root).into_iter().any(|n| {
            if let Some(b) = ast::BinExpr::cast(n.clone()) {
                return matches!(b.op_kind(), Some(ast::BinaryOp::Assignment { .. }))
                    && b.lhs().is_some_and(|l| cx.local_of_path(&l) == Some(local));
            }
            ast::MethodCallExpr::cast(n)
                .and_then(|m| m.receiver())
                .is_some_and(|r| {
                    cx.local_of_path(&r) == Some(local)
                        && !cx.builders.contains(&local)
                        && super::is_mut_ref_cell_ty(&cx.ty(&r))
                        && cx
                            .sema
                            .type_of_expr(&r)
                            .is_some_and(|t| t.adjusted().is_mutable_reference())
                })
        })
    };
    let mut seen = std::collections::HashSet::new();
    let mut copies = Vec::new();
    for pe in nodes.into_iter().filter_map(ast::PathExpr::cast) {
        let Some(local) = cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) else {
            continue;
        };
        if bound.contains(&local)
            || cx.cells.contains(&local)
            || cx.cell_handles.contains(&local)
            || !seen.insert(local)
            || assigned(cx, c.syntax(), local)
            || !assigned(cx, &scope, local)
        {
            continue;
        }
        let name = cx.lookup(local.name(cx.db).as_str());
        let copy = cx.fresh(&name);
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "var".into(),
                name: copy.clone(),
                init: Some(Expr::name(&name)),
            },
        ));
        copies.push((local.name(cx.db).as_str().to_string(), copy));
    }
    copies
}

fn strip_ref_pats(p: ast::Pat) -> ast::Pat {
    match &p {
        ast::Pat::RefPat(r) => r.pat().map(strip_ref_pats).unwrap_or(p),
        _ => p,
    }
}

fn lower_closure_inner(cx: &mut Cx, c: &ast::ClosureExpr) -> Expr {
    cx.push_scope();
    let mut params = Vec::new();
    let mut prelude = Vec::new();
    let indexed = cx
        .indexed_params
        .take()
        .and_then(|items| super::indexed::closure_params(cx, c, &items));
    if let Some(idx) = indexed {
        params.push(idx);
    } else if let Some(pl) = c.param_list() {
        for p in pl.params() {
            match p.pat().map(strip_ref_pats) {
                Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => {
                    let n = ip
                        .name()
                        .map(|n| n.text().to_string())
                        .unwrap_or_else(|| "_".into());
                    params.push(cx.declare_param(&n));
                }
                Some(ast::Pat::WildcardPat(_)) | None => params.push(cx.fresh("unused")),
                Some(other) => {
                    // Destructuring parameter: bind a fresh name, unpack inside the body.
                    let tmp = cx.fresh(super::pat_hint(&other));
                    params.push(tmp.clone());
                    pat::bind_pattern(cx, &other, Expr::name(tmp), &mut prelude);
                }
            }
        }
    }
    let body = match c.body() {
        Some(b) => b,
        None => {
            cx.pop_scope();
            return cx.unsupported(c.syntax(), "closure without body");
        }
    };
    // A diverging body (`{ ..; process::exit(2); }`) still returns the closure's declared type.
    let body_ty = match (cx.ty(&body), cx.ty(&ast::Expr::ClosureExpr(c.clone()))) {
        (Ty::Never, Ty::Fn { ret, .. }) if !ret.is_unit() => *ret,
        (t, _) => t,
    };
    let ret_unit = body_ty.is_unit();
    let saved_ret = cx.ret_ty.replace(body_ty);
    let saved_loops = std::mem::take(&mut cx.loops);
    let mut hoisted = Vec::new();
    let result = if prelude.is_empty()
        && !stmt::is_control_flow(&body)
        && !matches!(body, ast::Expr::BlockExpr(_))
    {
        let v = lower_expr(cx, &body, &mut hoisted);
        if hoisted.is_empty() { Some(v) } else { None }
    } else {
        None
    };
    let lambda_body = match result {
        Some(v) if !(ret_unit && !v.is_statement_expr()) => LambdaBody::Expr(Box::new(v)),
        _ => {
            let mut stmts = prelude;
            let dest = if ret_unit {
                Dest::Discard
            } else {
                Dest::Return
            };
            stmt::lower_to_dest(cx, &body, &dest, &mut stmts);
            let close_line = cx.end_line(c.syntax());
            let stmts = match (stmts.len(), stmts.first().map(|s| &s.kind)) {
                (1, Some(StmtKind::Block(b))) => b.stmts.clone(),
                _ => stmts,
            };
            LambdaBody::Block(crate::jir::Block { stmts, close_line })
        }
    };
    cx.loops = saved_loops;
    cx.ret_ty = saved_ret;
    cx.pop_scope();
    Expr::Lambda {
        params,
        body: lambda_body,
    }
}

// Argument expressions of a call, coerced to the callee's parameter types when known.
pub fn lower_args(
    cx: &mut Cx,
    args: impl Iterator<Item = ast::Expr>,
    out: &mut Vec<Stmt>,
) -> Vec<Expr> {
    args.map(|a| {
        // `&mut s` / `out` of a `StringBuilder` hands over the builder itself.
        if matches!(cx.ty(&a), Ty::Ref { mutable: true, .. }) {
            let inner = match super::strip_parens(&a) {
                ast::Expr::RefExpr(r) => r.expr().unwrap_or_else(|| a.clone()),
                other => other,
            };
            if let Some(b) = cx.builder_path(&inner) {
                return Expr::name(b);
            }
        }
        // Re-passing a `&mut` cell parameter (`bump(n)` inside `fn f(n: &mut i32)`) passes the array.
        if matches!(cx.ty(&a), Ty::Ref { mutable: true, .. })
            && let Some(cell) = cx.cell_path(&a)
        {
            return Expr::name(cell);
        }
        let want = cx.adjusted_ty(&a);
        // A collection passed as `I: IntoIterator` (or a crate iterator struct passed as
        // `impl Iterator`) goes in as its stream.
        if !matches!(want.strip_refs(), Ty::Iter(_))
            && calls_local_fn(cx, &a)
            && stream_param(cx, &a)
        {
            // Streaming a `v.clone()` reads `v` itself.
            let src = match super::strip_parens(&a) {
                ast::Expr::MethodCallExpr(m) if super::iter::method_name(&m) == "clone" => {
                    m.receiver().unwrap_or_else(|| a.clone())
                }
                other => other,
            };
            let v = lower_expr(cx, &src, out);
            return match want.strip_refs() {
                Ty::Map { .. } => Expr::call(Expr::call(v, "entrySet", vec![]), "stream", vec![]),
                _ => Expr::call(v, "stream", vec![]),
            };
        }
        let v = lower_expr_as(cx, &a, &want, out);
        // Java only narrows int constants on assignment, not when passing an argument to a
        // method of the program (std calls are lowered to expressions, not Java calls).
        match (&v, want.strip_refs()) {
            (Expr::Raw(s), Ty::Int(i))
                if matches!(i.java(), "byte" | "short")
                    && calls_local_fn(cx, &a)
                    && s.trim_start_matches('-')
                        .chars()
                        .all(|c| c.is_ascii_digit()) =>
            {
                Expr::Cast(i.java().into(), Box::new(v))
            }
            _ => v,
        }
    })
    .collect()
}

// Whether argument `a` fills a parameter that is a Java `Stream`: `I: IntoIterator<..>`,
// `impl Iterator<..>` and the like.
fn stream_param(cx: &Cx, a: &ast::Expr) -> bool {
    let Some(list) = a.syntax().parent().and_then(ast::ArgList::cast) else {
        return false;
    };
    let Some(pos) = list.args().position(|x| x.syntax() == a.syntax()) else {
        return false;
    };
    let Some(call) = list.syntax().parent() else {
        return false;
    };
    let params = if let Some(m) = ast::MethodCallExpr::cast(call.clone()) {
        cx.sema
            .resolve_method_call(&m)
            .map(|f| f.params_without_self(cx.db))
    } else if let Some(ast::Expr::PathExpr(p)) = ast::CallExpr::cast(call).and_then(|c| c.expr()) {
        match p.path().and_then(|p| cx.sema.resolve_path(&p)) {
            Some(ra_ap_hir::PathResolution::Def(ra_ap_hir::ModuleDef::Function(f))) => {
                Some(f.assoc_fn_params(cx.db))
            }
            _ => None,
        }
    } else {
        None
    };
    let Some(param) = params.and_then(|ps| ps.into_iter().nth(pos)) else {
        return false;
    };
    matches!(cx.convert(param.ty()), Ty::Iter(_))
}

// Whether argument `a` is passed to a function defined in the transpiled crate.
fn calls_local_fn(cx: &Cx, a: &ast::Expr) -> bool {
    let Some(call) = a.syntax().parent().and_then(|p| p.parent()) else {
        return false;
    };
    let f = if let Some(m) = ast::MethodCallExpr::cast(call.clone()) {
        cx.sema.resolve_method_call(&m)
    } else if let Some(ast::Expr::PathExpr(p)) = ast::CallExpr::cast(call).and_then(|c| c.expr()) {
        match p.path().and_then(|p| cx.sema.resolve_path(&p)) {
            Some(ra_ap_hir::PathResolution::Def(ra_ap_hir::ModuleDef::Function(f))) => Some(f),
            _ => None,
        }
    } else {
        None
    };
    f.is_some_and(|f| f.module(cx.db).krate(cx.db).origin(cx.db).is_local())
}

pub fn call_args(cx: &mut Cx, c: &impl HasArgList, out: &mut Vec<Stmt>) -> Vec<Expr> {
    match c.arg_list() {
        Some(al) => lower_args(cx, al.args(), out),
        None => vec![],
    }
}
