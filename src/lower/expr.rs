// Value-position expressions. Control flow in value position is hoisted into `out`.

use ra_ap_hir::PathResolution;
use ra_ap_syntax::{AstNode, AstToken, ast, ast::HasArgList, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, calls, items, iter, macros, pat, std_map};
use crate::jir::{Expr, LambdaBody, Stmt, StmtKind, java_char_literal, java_string_literal};
use crate::ty::{Int, Ty};

pub fn lower_expr(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    match e {
        ast::Expr::Literal(l) => lower_literal(cx, l, e),
        ast::Expr::ParenExpr(p) => match p.expr() {
            Some(inner) => lower_expr(cx, &inner, out),
            None => Expr::raw("()"),
        },
        ast::Expr::PathExpr(p) => lower_path_expr(cx, p),
        ast::Expr::BinExpr(b) => lower_bin(cx, b, out),
        ast::Expr::PrefixExpr(p) => lower_prefix(cx, p, out),
        ast::Expr::RefExpr(r) => match r.expr() {
            // `&mut cell` passes the array itself so the callee can write through it.
            Some(inner) if r.mut_token().is_some() && cx.cell_path(&inner).is_some() => {
                Expr::name(cx.cell_path(&inner).unwrap())
            }
            Some(inner) if r.mut_token().is_some() && super::is_cell_value_ty(&cx.ty(&inner)) => {
                cx.unsupported(r.syntax(), "`&mut` to a field/element of immutable Java type")
            }
            Some(inner) => lower_expr(cx, &inner, out),
            None => Expr::raw("null"),
        },
        ast::Expr::CallExpr(c) => calls::lower_call(cx, c, out),
        ast::Expr::MethodCallExpr(m) => calls::lower_method_call(cx, m, out),
        ast::Expr::MacroExpr(m) if macros::is_diverging(m) => cx.unsupported(m.syntax(), "diverging macro in expression"),
        ast::Expr::MacroExpr(m) => macros::lower_macro_expr(cx, m, out),
        ast::Expr::FieldExpr(f) => lower_field(cx, f, out),
        ast::Expr::IndexExpr(i) => lower_index(cx, i, out),
        ast::Expr::CastExpr(c) => lower_cast(cx, c, out),
        ast::Expr::ClosureExpr(c) => lower_closure(cx, c),
        ast::Expr::TupleExpr(t) => {
            let ty = cx.ty(e);
            let args: Vec<Expr> = t.fields().map(|f| lower_expr(cx, &f, out)).collect();
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

pub fn coerce(v: Expr, have: &Ty, want: &Ty) -> Expr {
    match (have.strip_refs(), want.strip_refs()) {
        // Java won't implicitly narrow int-typed expressions into byte/short.
        (Ty::Int(a), Ty::Int(b)) if a.java() != b.java() && matches!(b.java(), "byte" | "short") => {
            if matches!(v, Expr::Raw(ref s) if s.chars().all(|c| c.is_ascii_digit())) { v } else { Expr::Cast(b.java().into(), Box::new(v)) }
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
    let tmp = cx.fresh("tmp");
    out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type_explicit(&ty), name: tmp.clone(), init: None }));
    stmt::lower_to_dest(cx, e, &Dest::Assign(tmp.clone()), out);
    Expr::name(tmp)
}

fn simple_block_tail(b: &ast::BlockExpr) -> Option<ast::Expr> {
    if b.label().is_some() || b.unsafe_token().is_some() || b.async_token().is_some() || b.const_token().is_some() {
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
    let then_ok = i.then_branch().is_some_and(|b| simple_block_tail(&b).is_some_and(|t| is_plain(&t)));
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
    let a = lower_expr(cx, &simple_block_tail(&i.then_branch().unwrap()).unwrap(), out);
    let b = match i.else_branch().unwrap() {
        ast::ElseBranch::Block(b) => lower_expr(cx, &simple_block_tail(&b).unwrap(), out),
        ast::ElseBranch::IfExpr(ei) => lower_simple_if(cx, &ei, out),
    };
    // javac types a bare `Optional.empty()` branch as Optional<Object>; give it a witness.
    let ty = cx.ty(&ast::Expr::IfExpr(i.clone()));
    let witness = |e: Expr| match (&e, ty.strip_refs()) {
        (Expr::Raw(s), Ty::Option(t)) if s == "Optional.empty()" => Expr::raw(format!("Optional.<{}>empty()", t.boxed())),
        _ => e,
    };
    Expr::Ternary(Box::new(c), Box::new(witness(a)), Box::new(witness(b)))
}

fn lower_literal(cx: &mut Cx, l: &ast::Literal, e: &ast::Expr) -> Expr {
    let ty = cx.ty(e);
    match l.kind() {
        ast::LiteralKind::IntNumber(n) => {
            let digits = n.text().to_string();
            let (body, _) = split_suffix(&digits);
            let body = body.trim_end_matches('_').to_string();
            match ty {
                Ty::F32 => Expr::raw(format!("{body}.0f")),
                Ty::F64 => Expr::raw(format!("{body}.0")),
                Ty::Int(i) if i.is_long() => Expr::raw(format!("{body}L")),
                _ => Expr::raw(body),
            }
        }
        ast::LiteralKind::FloatNumber(n) => {
            let text = n.text().to_string();
            let body = text.trim_end_matches("f64").trim_end_matches("f32").trim_end_matches('_');
            let mut body = body.to_string();
            if body.ends_with('.') {
                body.push('0');
            }
            if ty == Ty::F32 {
                body.push('f');
            }
            Expr::raw(body)
        }
        ast::LiteralKind::Bool(b) => Expr::raw(if b { "true" } else { "false" }),
        ast::LiteralKind::Char(c) => match c.value() {
            Ok(ch) => Expr::raw(java_char_literal(ch)),
            Err(_) => cx.unsupported(e.syntax(), "char literal"),
        },
        ast::LiteralKind::Byte(b) => match b.value() {
            Ok(v) if v.is_ascii_graphic() || v == b' ' => Expr::raw(format!("(int) {}", java_char_literal(v as char))),
            Ok(v) => Expr::raw(v.to_string()),
            Err(_) => cx.unsupported(e.syntax(), "byte literal"),
        },
        ast::LiteralKind::String(s) => match s.value() {
            Ok(v) => Expr::raw(java_string_literal(&v)),
            Err(_) => cx.unsupported(e.syntax(), "string literal"),
        },
        ast::LiteralKind::ByteString(_) | ast::LiteralKind::CString(_) => {
            cx.unsupported(e.syntax(), "byte/C string literal")
        }
    }
}

fn split_suffix(s: &str) -> (&str, &str) {
    for suf in ["i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize", "f32", "f64"] {
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
    let Some(path) = p.path() else { return Expr::raw("null") };
    let text = path.syntax().text().to_string();
    if text == "self" {
        return Expr::raw(cx.self_name.clone());
    }
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Local(local)) => {
            if let Some((list, idx)) = cx.elem_aliases.get(&local) {
                return Expr::call(Expr::name(list.clone()), "get", vec![Expr::name(idx.clone())]);
            }
            let name = cx.lookup(local.name(cx.db).as_str());
            if cx.cells.contains(&local) { Expr::raw(format!("{name}[0]")) } else { Expr::name(name) }
        }
        Some(PathResolution::Def(def)) => items::lower_def_path(cx, &path, def),
        Some(PathResolution::SelfType(_)) => Expr::raw(cx.self_ty_java.clone()),
        _ => cx.unsupported(p.syntax(), &format!("path `{text}`")),
    }
}

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
    let l = lower_expr(cx, &lhs, out);
    let r = lower_expr(cx, &rhs, out);
    match op {
        BinaryOp::LogicOp(LogicOp::And) => Expr::bin(l, "&&", r),
        BinaryOp::LogicOp(LogicOp::Or) => Expr::bin(l, "||", r),
        BinaryOp::CmpOp(CmpOp::Eq { negated }) => {
            if lt.is_primitive() && rt.is_primitive() {
                Expr::bin(l, if negated { "!=" } else { "==" }, r)
            } else {
                let eq = Expr::call(l, "equals", vec![r]);
                if negated { Expr::Unary("!", Box::new(eq)) } else { eq }
            }
        }
        BinaryOp::CmpOp(CmpOp::Ord { ordering, strict }) => {
            let sym = match (ordering, strict) {
                (ast::Ordering::Less, true) => "<",
                (ast::Ordering::Less, false) => "<=",
                (ast::Ordering::Greater, true) => ">",
                (ast::Ordering::Greater, false) => ">=",
            };
            if lt.is_primitive() {
                Expr::bin(l, sym, r)
            } else {
                Expr::bin(Expr::call(l, "compareTo", vec![r]), sym, Expr::raw("0"))
            }
        }
        BinaryOp::ArithOp(a) => {
            if let Ty::Adt { .. } = lt.strip_refs() {
                return Expr::call(l, op_trait_method(a), vec![r]);
            }
            let sym = arith_symbol(a, &lt);
            let v = Expr::bin(l, sym, r);
            narrow_if_needed(v, &cx.ty(&ast::Expr::BinExpr(b.clone())))
        }
        BinaryOp::Assignment { .. } => unreachable!(),
    }
}

fn arith_symbol(a: ast::ArithOp, lt: &Ty) -> &'static str {
    use ast::ArithOp::*;
    match a {
        Add => "+",
        Sub => "-",
        Mul => "*",
        Div => "/",
        Rem => "%",
        Shl => "<<",
        Shr => {
            if matches!(lt.strip_refs(), Ty::Int(i) if i.is_unsigned()) { ">>>" } else { ">>" }
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

// Java promotes byte/short arithmetic to int; cast back so the value fits its Rust type.
fn narrow_if_needed(v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Int(i @ (Int::I8 | Int::I16 | Int::U16)) => Expr::Cast(i.java().into(), Box::new(v)),
        _ => v,
    }
}

fn lower_assign(cx: &mut Cx, lhs: &ast::Expr, rhs: &ast::Expr, op: Option<ast::ArithOp>, out: &mut Vec<Stmt>) -> Expr {
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
pub fn place_assign(cx: &mut Cx, lhs: &ast::Expr, sym: &'static str, r: Expr, out: &mut Vec<Stmt>) -> Expr {
    match lhs {
        ast::Expr::IndexExpr(ix) => {
            let (Some(base), Some(index)) = (ix.base(), ix.index()) else {
                return cx.unsupported(lhs.syntax(), "index assignment");
            };
            let bt = cx.ty(&base);
            let b = lower_expr(cx, &base, out);
            let i = lower_expr(cx, &index, out);
            let setter = if matches!(bt.strip_refs(), Ty::Map { .. }) { "put" } else { "set" };
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
        ast::Expr::PrefixExpr(p)
            if p.op_kind() == Some(ast::UnaryOp::Deref)
                && let Some(local) = p.expr().and_then(|i| cx.local_of_path(&i))
                && let Some((list, idx)) = cx.elem_aliases.get(&local).cloned() =>
        {
            // `*x op= r` where `x` aliases `list[idx]`.
            let (list, idx) = (Expr::name(list), Expr::name(idx));
            let value = if sym == "=" {
                r
            } else {
                let elem_ty = cx.ty(lhs);
                let cur = Expr::call(list.clone(), "get", vec![idx.clone()]);
                narrow_if_needed(Expr::bin(cur, static_op(sym.trim_end_matches('=')), r), &elem_ty)
            };
            Expr::call(list, "set", vec![idx, value])
        }
        ast::Expr::PrefixExpr(p) if p.op_kind() == Some(ast::UnaryOp::Deref) => {
            let inner = p.expr().unwrap();
            let place = lower_expr(cx, &inner, out);
            Expr::Assign(Box::new(super::calls::deref_place(cx, &inner, place)), sym, Box::new(r))
        }
        _ => {
            let place = lower_expr(cx, lhs, out);
            Expr::Assign(Box::new(place), sym, Box::new(r))
        }
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
    let Some(inner) = p.expr() else { return cx.unsupported(p.syntax(), "prefix expression") };
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
            if ty.strip_refs() == &Ty::Bool {
                Expr::Unary("!", Box::new(v))
            } else {
                Expr::Unary("~", Box::new(v))
            }
        }
        Some(ast::UnaryOp::Deref) => {
            // Cells (`n[0]`) and iter_mut aliases (`v.get(i)`) were resolved when lowering `inner`.
            let resolved = cx.cell_path(&inner).is_some()
                || cx.local_of_path(&inner).is_some_and(|l| cx.elem_aliases.contains_key(&l));
            if let Ty::Ref { mutable: true, inner: t } = &ty
                && (t.is_primitive() || t.is_str())
                && !resolved
            {
                return cx.unsupported(p.syntax(), "deref of a `&mut` primitive/String (cell)");
            }
            super::calls::deref_place(cx, &inner, v)
        }
        None => cx.unsupported(p.syntax(), "prefix operator"),
    }
}

fn lower_field(cx: &mut Cx, f: &ast::FieldExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(base) = f.expr() else { return cx.unsupported(f.syntax(), "field access") };
    let bt = cx.ty(&base);
    let b = lower_expr(cx, &base, out);
    if let Some(idx) = f.index_token() {
        let i: usize = idx.text().parse().unwrap_or(0);
        if let Ty::Tuple(ts) = bt.strip_refs() {
            return tuple_field(b, i, ts.len());
        }
        return Expr::Field(Box::new(b), format!("_{i}"));
    }
    let name = f.name_ref().map(|n| n.text().to_string()).unwrap_or_default();
    let name = if !name.is_empty() && name.chars().all(|c| c.is_ascii_digit()) { format!("_{name}") } else { super::escape_keyword(&name) };
    Expr::Field(Box::new(b), name)
}

pub fn tuple_field(b: Expr, i: usize, arity: usize) -> Expr {
    if arity == 2 {
        Expr::call(b, if i == 0 { "getKey" } else { "getValue" }, vec![])
    } else {
        Expr::call(b, format!("_{i}"), vec![])
    }
}

pub fn tuple_expr(cx: &mut Cx, ty: &Ty, args: Vec<Expr>) -> Expr {
    match args.len() {
        0 => Expr::raw("null"),
        2 => Expr::Call { recv: Some(Box::new(Expr::raw("Map"))), name: "entry".into(), args },
        n => {
            items::need_tuple(cx, n);
            let _ = ty;
            Expr::New { ty: format!("Tuple{n}<>"), args }
        }
    }
}

fn lower_index(cx: &mut Cx, i: &ast::IndexExpr, out: &mut Vec<Stmt>) -> Expr {
    let (Some(base), Some(index)) = (i.base(), i.index()) else {
        return cx.unsupported(i.syntax(), "index expression");
    };
    let bt = cx.ty(&base);
    if let ast::Expr::RangeExpr(r) = &index {
        let b = lower_expr(cx, &base, out);
        return std_map::lower_slice_range(cx, b, &bt, r, out);
    }
    let b = lower_expr(cx, &base, out);
    let ix = lower_expr(cx, &index, out);
    match bt.strip_refs() {
        Ty::Str => Expr::call(b, "charAt", vec![ix]),
        _ => Expr::call(b, "get", vec![ix]),
    }
}

fn lower_cast(cx: &mut Cx, c: &ast::CastExpr, out: &mut Vec<Stmt>) -> Expr {
    let (Some(inner), Some(t)) = (c.expr(), c.ty()) else { return cx.unsupported(c.syntax(), "cast") };
    let from = cx.ty(&inner);
    let to = cx.type_ref(&t);
    let v = lower_expr(cx, &inner, out);
    let (from, to) = (from.strip_refs().clone(), to.strip_refs().clone());
    if from.java() == to.java() {
        return v;
    }
    match (&from, &to) {
        (Ty::Bool, Ty::Int(_)) => Expr::Ternary(Box::new(v), Box::new(Expr::raw("1")), Box::new(Expr::raw("0"))),
        // Plain enums carry their Rust discriminant in a `value` field.
        (Ty::Adt { .. }, Ty::Int(_)) => Expr::Field(Box::new(v), "value".into()),
        // Widening int->long/double is implicit in Java but explicit casts read closer to Rust.
        _ => Expr::Cast(to.java(), Box::new(v)),
    }
}

pub fn lower_closure(cx: &mut Cx, c: &ast::ClosureExpr) -> Expr {
    cx.lambda_depth += 1;
    let e = lower_closure_inner(cx, c);
    cx.lambda_depth -= 1;
    e
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
    if let Some(pl) = c.param_list() {
        for p in pl.params() {
            match p.pat().map(strip_ref_pats) {
                Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() => {
                    let n = ip.name().map(|n| n.text().to_string()).unwrap_or_else(|| "_".into());
                    params.push(cx.declare(&n));
                }
                Some(ast::Pat::WildcardPat(_)) | None => params.push(cx.fresh("unused")),
                Some(other) => {
                    // Destructuring parameter: bind a fresh name, unpack inside the body.
                    let tmp = cx.fresh("arg");
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
    let ret_unit = cx.ty(&body).is_unit();
    let saved_ret = cx.ret_ty.replace(cx.ty(&body));
    let saved_loops = std::mem::take(&mut cx.loops);
    let mut hoisted = Vec::new();
    let result = if prelude.is_empty() && !stmt::is_control_flow(&body) && !matches!(body, ast::Expr::BlockExpr(_)) {
        let v = lower_expr(cx, &body, &mut hoisted);
        if hoisted.is_empty() { Some(v) } else { None }
    } else {
        None
    };
    let lambda_body = match result {
        Some(v) if !(ret_unit && !v.is_statement_expr()) => LambdaBody::Expr(Box::new(v)),
        _ => {
            let mut stmts = prelude;
            let dest = if ret_unit { Dest::Discard } else { Dest::Return };
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
    Expr::Lambda { params, body: lambda_body }
}

// Argument expressions of a call, coerced to the callee's parameter types when known.
pub fn lower_args(cx: &mut Cx, args: impl Iterator<Item = ast::Expr>, out: &mut Vec<Stmt>) -> Vec<Expr> {
    args.map(|a| {
        // Re-passing a `&mut` cell parameter (`bump(n)` inside `fn f(n: &mut i32)`) passes the array.
        if matches!(cx.ty(&a), Ty::Ref { mutable: true, .. })
            && let Some(cell) = cx.cell_path(&a)
        {
            return Expr::name(cell);
        }
        let want = cx.adjusted_ty(&a);
        lower_expr_as(cx, &a, &want, out)
    })
    .collect()
}

pub fn call_args(cx: &mut Cx, c: &impl HasArgList, out: &mut Vec<Stmt>) -> Vec<Expr> {
    match c.arg_list() {
        Some(al) => lower_args(cx, al.args(), out),
        None => vec![],
    }
}
