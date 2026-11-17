// std macros: println!/print!/eprintln!/format!/panic!/assert!/vec!/... (translated from arguments).

use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasName};

use super::{Cx, expr, fmt};
use crate::jir::{Block, Expr, LambdaBody, Stmt, StmtKind};
use crate::ty::Ty;

pub fn macro_name(mc: &ast::MacroCall) -> String {
    mc.path()
        .and_then(|p| p.segment())
        .and_then(|s| s.name_ref())
        .map(|n| n.text().to_string())
        .unwrap_or_default()
}

pub fn is_diverging(m: &ast::MacroExpr) -> bool {
    m.macro_call()
        .is_some_and(|mc| matches!(macro_name(&mc).as_str(), "panic" | "unreachable" | "todo" | "unimplemented"))
}

// Text of the first comma-separated macro argument (the write target).
pub fn lower_macro_expr(cx: &mut Cx, m: &ast::MacroExpr, out: &mut Vec<Stmt>) -> Expr {
    let Some(mc) = m.macro_call() else { return cx.unsupported(m.syntax(), "macro") };
    let name = macro_name(&mc);
    match name.as_str() {
        "println" | "print" | "eprintln" | "eprint" => {
            let stream = if name.starts_with('e') { "System.err" } else { "System.out" };
            let newline = name.ends_with("ln");
            match fmt::format_args_of(cx, &mc) {
                Some(fa) => {
                    let s = if newline { fmt::lower_format_ln(cx, &fa, out) } else { fmt::lower_format(cx, &fa, out) };
                    let method = if newline { "println" } else { "print" };
                    if newline && fmt::is_empty_string(&s) {
                        return Expr::call(Expr::raw(stream), method, vec![]);
                    }
                    Expr::call(Expr::raw(stream), method, vec![s])
                }
                None if newline => Expr::call(Expr::raw(stream), "println", vec![]),
                None => cx.unsupported(mc.syntax(), "print! without arguments"),
            }
        }
        "format" | "format_args" | "format_args_nl" => match fmt::format_args_of(cx, &mc) {
            Some(fa) => fmt::lower_format(cx, &fa, out),
            None => cx.unsupported(mc.syntax(), "format! arguments"),
        },
        "panic" | "unreachable" | "todo" | "unimplemented" => {
            let default = match name.as_str() {
                "unreachable" => "internal error: entered unreachable code",
                "todo" => "not yet implemented",
                "unimplemented" => "not implemented",
                _ => "explicit panic",
            };
            let msg = match fmt::format_args_of(cx, &mc) {
                Some(fa) => fmt::lower_format(cx, &fa, out),
                None => Expr::str_lit(default),
            };
            Expr::New { ty: "RuntimeException".into(), args: vec![msg] }
        }
        "vec" => {
            // The expansion holds typed element expressions: `[a, b]` or `from_elem(x, n)`.
            let Some(exp) = cx.sema.expand_macro_call(&mc) else {
                return cx.unsupported(mc.syntax(), "vec! expansion");
            };
            let from_elem = exp.value.descendants().filter_map(ast::CallExpr::cast).find(|c| {
                c.expr().is_some_and(|f| f.syntax().text().to_string().contains("from_elem"))
            });
            if let Some(c) = from_elem {
                use ra_ap_syntax::ast::HasArgList;
                let args: Vec<ast::Expr> = c.arg_list().map(|a| a.args().collect()).unwrap_or_default();
                if args.len() == 2 {
                    return super::std_map::lower_repeat(cx, mc.syntax(), &args[0], &args[1], out);
                }
            }
            match exp.value.descendants().find_map(ast::ArrayExpr::cast) {
                Some(a) => super::std_map::lower_array_expr(cx, &a, out),
                None => lower_vec(cx, m, &mc, out),
            }
        }
        "assert" | "assert_eq" | "assert_ne" => lower_assert(cx, &mc, &name, out),
        "debug_assert" | "debug_assert_eq" | "debug_assert_ne" => Expr::raw("null"),
        "dbg" => lower_dbg(cx, &mc, out),
        "matches" => lower_matches(cx, &mc, out),
        "write" | "writeln" => lower_write(cx, &mc, out),
        _ => cx.unsupported(mc.syntax(), &format!("macro `{name}!`")),
    }
}

fn lower_write(cx: &mut Cx, mc: &ast::MacroCall, out: &mut Vec<Stmt>) -> Expr {
    let Some(call) = expanded_node::<ast::MethodCallExpr>(cx, mc) else {
        return cx.unsupported(mc.syntax(), "write! expansion");
    };
    let Some(mut target) = call.receiver() else { return cx.unsupported(mc.syntax(), "write! target") };
    loop {
        let inner = match &target {
            ast::Expr::ParenExpr(p) => p.expr(),
            ast::Expr::RefExpr(r) => r.expr(),
            _ => None,
        };
        match inner { Some(e) => target = e, None => break }
    }
    let target_ty = cx.ty(&target);
    let target_value = expr::lower_expr(cx, &target, out);
    let Some(fa) = fmt::format_args_of(cx, mc) else {
        return cx.unsupported(mc.syntax(), "write! format arguments");
    };
    let text = fmt::lower_format(cx, &fa, out);
    let write = if target_ty.is_str() {
        if !matches!(target, ast::Expr::PathExpr(_)) {
            return cx.unsupported(mc.syntax(), "write! non-local String target");
        }
        Expr::Assign(Box::new(target_value), "+=", Box::new(text))
    } else if matches!(target_ty.strip_refs(), Ty::Other(s) if s.contains("Formatter"))
        || matches!(target_ty.strip_refs(), Ty::Adt { name, .. } if name == "Formatter")
    {
        Expr::call(target_value, "append", vec![text])
    } else {
        return cx.unsupported(mc.syntax(), "write! target type");
    };
    out.push(Stmt::new(cx.line(mc.syntax()), StmtKind::Expr(write)));
    cx.needs_result = true;
    Expr::New { ty: "Ok<>".into(), args: vec![Expr::Cast("Void".into(), Box::new(Expr::raw("null")))] }
}

fn expanded_node<T: AstNode>(cx: &Cx, mc: &ast::MacroCall) -> Option<T> {
    fn search<T: AstNode>(cx: &Cx, node: &ra_ap_syntax::SyntaxNode, depth: u32) -> Option<T> {
        if depth > 4 { return None; }
        for n in node.descendants() {
            if let Some(found) = T::cast(n.clone()) { return Some(found); }
            if let Some(call) = ast::MacroCall::cast(n)
                && let Some(expansion) = cx.sema.expand_macro_call(&call)
                && let Some(found) = search(cx, &expansion.value, depth + 1)
            {
                return Some(found);
            }
        }
        None
    }
    search(cx, &cx.sema.expand_macro_call(mc)?.value, 0)
}

fn lower_vec(cx: &mut Cx, m: &ast::MacroExpr, mc: &ast::MacroCall, out: &mut Vec<Stmt>) -> Expr {
    let Some(expansion) = cx.sema.expand_macro_call(mc) else {
        return cx.unsupported(mc.syntax(), "vec! expansion");
    };
    let Some(call) = expansion.value.descendants().find_map(ast::CallExpr::cast) else {
        return cx.unsupported(mc.syntax(), "vec! expansion");
    };
    let is_repeat = call.expr().is_some_and(|e| e.syntax().text().to_string().ends_with("from_elem_global"));
    if !is_repeat
        && let Some(array) = expansion.value.descendants().find_map(ast::ArrayExpr::cast)
    {
        let mut elements = Vec::new();
        for element in array.exprs() {
            let ty = cx.ty(&element);
            let value = expr::lower_expr(cx, &element, out);
            let temp = cx.fresh("vec_item");
            out.push(Stmt::new(cx.line(mc.syntax()), StmtKind::Local {
                ty: if matches!(ty.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(&ty) },
                name: temp.clone(), init: Some(value),
            }));
            elements.push(Expr::name(temp));
        }
        return Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![Expr::call(Expr::raw("Arrays"), "asList", elements)],
        };
    }
    let args: Vec<_> = call.arg_list().into_iter().flat_map(|a| a.args()).collect();
    if args.is_empty() {
        return Expr::New { ty: "ArrayList<>".into(), args: vec![] };
    }
    if args.len() != 2 {
        return cx.unsupported(mc.syntax(), "vec! repetition");
    }
    let ty = cx.ty(&ast::Expr::MacroExpr(m.clone()));
    let Ty::Vec(inner) = ty.strip_refs() else {
        return cx.unsupported(mc.syntax(), "vec! element type");
    };
    let value = expr::lower_expr(cx, &args[0], out);
    let name = cx.fresh("vec_value");
    let line = cx.line(mc.syntax());
    out.push(Stmt::new(line, StmtKind::Local {
        ty: if matches!(inner.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(inner) },
        name: name.clone(), init: Some(value),
    }));
    let count = expr::lower_expr(cx, &args[1], out);
    if inner.is_primitive() || inner.is_str() || matches!(inner.strip_refs(), Ty::Unit) {
        let copies = Expr::call(Expr::raw("Collections"), "nCopies", vec![count, Expr::name(name)]);
        return Expr::New { ty: "ArrayList<>".into(), args: vec![copies] };
    }
    let Some(copy) = clone_value(cx, Expr::name(name), inner) else {
        return cx.unsupported(mc.syntax(), "vec! repetition of this mutable type");
    };
    let index = cx.fresh("vec_index");
    let range = Expr::call(Expr::raw("IntStream"), "range", vec![Expr::raw("0"), count]);
    let mapped = Expr::call(range, "mapToObj", vec![Expr::Lambda { params: vec![index], body: LambdaBody::Expr(Box::new(copy)) }]);
    let collector = Expr::call(Expr::raw("Collectors"), "toCollection", vec![Expr::raw("ArrayList::new")]);
    Expr::call(mapped, "collect", vec![collector])
}

fn clone_value(cx: &mut Cx, value: Expr, ty: &Ty) -> Option<Expr> {
    match ty.strip_refs() {
        t if t.is_primitive() || t.is_str() => Some(value),
        Ty::Vec(inner) => {
            let name = cx.fresh("vec_item");
            let copy = clone_value(cx, Expr::name(&name), inner)?;
            let stream = Expr::call(value, "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![Expr::Lambda { params: vec![name], body: LambdaBody::Expr(Box::new(copy)) }]);
            let collector = Expr::call(Expr::raw("Collectors"), "toCollection", vec![Expr::raw("ArrayList::new")]);
            Some(Expr::call(mapped, "collect", vec![collector]))
        }
        Ty::Adt { .. } => Some(Expr::call(value, "clone", vec![])),
        _ => None,
    }
}

fn lower_assert(cx: &mut Cx, mc: &ast::MacroCall, name: &str, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(mc.syntax());
    let mut failure = Vec::new();
    let (cond, default) = if name == "assert" {
        let Some(if_expr) = expanded_node::<ast::IfExpr>(cx, mc) else {
            return cx.unsupported(mc.syntax(), "assert! expansion");
        };
        let Some(ast::Expr::PrefixExpr(not)) = if_expr.condition() else {
            return cx.unsupported(mc.syntax(), "assert! condition");
        };
        let Some(condition) = not.expr() else { return cx.unsupported(mc.syntax(), "assert! condition") };
        let value = expr::lower_expr(cx, &condition, out);
        (Expr::Unary("!", Box::new(value)), Expr::str_lit("assertion failed"))
    } else {
        let Some(matched) = expanded_node::<ast::MatchExpr>(cx, mc) else {
            return cx.unsupported(mc.syntax(), "assert comparison expansion");
        };
        let Some(ast::Expr::TupleExpr(tuple)) = matched.expr() else {
            return cx.unsupported(mc.syntax(), "assert comparison arguments");
        };
        let args: Vec<_> = tuple.fields().filter_map(|e| match e {
            ast::Expr::RefExpr(r) => r.expr(),
            _ => None,
        }).collect();
        if args.len() != 2 { return cx.unsupported(mc.syntax(), "assert comparison arguments"); }
        let mut values = Vec::new();
        for arg in &args {
            let value = expr::lower_expr(cx, arg, out);
            let temp = cx.fresh("assert_value");
            let ty = cx.ty(arg);
            out.push(Stmt::new(line, StmtKind::Local {
                ty: if matches!(ty.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(&ty) },
                name: temp.clone(), init: Some(value),
            }));
            values.push(Expr::name(temp));
        }
        let left_ty = cx.ty(&args[0]);
        let right_ty = cx.ty(&args[1]);
        if !fmt::debug_supported(&left_ty) || !fmt::debug_supported(&right_ty) {
            return cx.unsupported(mc.syntax(), "assert comparison Debug type");
        }
        let eq = if left_ty.is_primitive() && right_ty.is_primitive() {
            Expr::bin(values[0].clone(), "==", values[1].clone())
        } else {
            Expr::call(Expr::raw("Objects"), "equals", values.clone())
        };
        let cond = if name == "assert_eq" { Expr::Unary("!", Box::new(eq)) } else { eq };
        let left = fmt::debug(cx, values[0].clone(), &left_ty);
        let right = fmt::debug(cx, values[1].clone(), &right_ty);
        let op = if name == "assert_eq" { "==" } else { "!=" };
        let message = Expr::bin(
            Expr::bin(Expr::bin(Expr::str_lit(&format!("assertion `left {op} right` failed\n  left: ")), "+", left),
                "+", Expr::str_lit("\n right: ")), "+", right,
        );
        (cond, message)
    };
    let message = if let Some(fa) = fmt::format_args_of(cx, mc) {
        let custom = fmt::lower_format(cx, &fa, &mut failure);
        Expr::bin(Expr::bin(default, "+", Expr::str_lit(": ")), "+", custom)
    } else {
        default
    };
    failure.push(Stmt::new(line, StmtKind::Throw(Expr::New { ty: "RuntimeException".into(), args: vec![message] })));
    out.push(Stmt::new(line, StmtKind::If { cond, then: Block { stmts: failure, close_line: line }, els: None }));
    Expr::raw("null")
}

fn lower_dbg(cx: &mut Cx, mc: &ast::MacroCall, out: &mut Vec<Stmt>) -> Expr {
    if let Some(expansion) = cx.sema.expand_macro_call(mc)
        && let Some(tuple) = ast::TupleExpr::cast(expansion.value)
    {
        let values: Vec<_> = tuple.fields().map(|e| expr::lower_expr(cx, &e, out)).collect();
        return expr::tuple_expr(cx, &Ty::Other("?".into()), values);
    }
    let Some(matched) = expanded_node::<ast::MatchExpr>(cx, mc) else {
        let line = cx.line(mc.syntax());
        out.push(Stmt::new(line, StmtKind::Expr(Expr::call(
            Expr::raw("System.err"), "println", vec![Expr::str_lit(&format!("[line {line}]"))],
        ))));
        return Expr::raw("null");
    };
    let Some(value) = matched.expr() else { return cx.unsupported(mc.syntax(), "dbg! value") };
    let ty = cx.ty(&value);
    if !fmt::debug_supported(&ty) {
        return cx.unsupported(mc.syntax(), "dbg! Debug type");
    }
    let lowered = expr::lower_expr(cx, &value, out);
    let name = cx.fresh("dbg_value");
    let line = cx.line(mc.syntax());
    out.push(Stmt::new(line, StmtKind::Local {
        ty: if matches!(ty.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(&ty) },
        name: name.clone(), init: Some(lowered),
    }));
    let text = Expr::bin(
        Expr::str_lit(&format!("[line {line}] {} = ", value.syntax().text())), "+",
        fmt::debug(cx, Expr::name(&name), &ty),
    );
    out.push(Stmt::new(line, StmtKind::Expr(Expr::call(Expr::raw("System.err"), "println", vec![text]))));
    Expr::name(name)
}

fn lower_matches(cx: &mut Cx, mc: &ast::MacroCall, out: &mut Vec<Stmt>) -> Expr {
    let Some(matched) = expanded_node::<ast::MatchExpr>(cx, mc) else {
        return cx.unsupported(mc.syntax(), "matches! expansion");
    };
    let Some(value) = matched.expr() else { return cx.unsupported(mc.syntax(), "matches! value") };
    let Some(arm) = matched.match_arm_list().and_then(|l| l.arms().next()) else {
        return cx.unsupported(mc.syntax(), "matches! pattern");
    };
    if arm.guard().is_some() { return cx.unsupported(mc.syntax(), "matches! guard"); }
    let Some(pattern) = arm.pat() else { return cx.unsupported(mc.syntax(), "matches! pattern") };
    let ty = cx.ty(&value);
    let lowered = expr::lower_expr(cx, &value, out);
    let name = cx.fresh("match_value");
    out.push(Stmt::new(cx.line(mc.syntax()), StmtKind::Local {
        ty: if matches!(ty.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(&ty) },
        name: name.clone(), init: Some(lowered),
    }));
    match_pattern(cx, &pattern, Expr::name(name), &ty, out)
}

fn match_pattern(cx: &mut Cx, pat: &ast::Pat, value: Expr, ty: &Ty, out: &mut Vec<Stmt>) -> Expr {
    match pat {
        ast::Pat::WildcardPat(_) => Expr::raw("true"),
        ast::Pat::LiteralPat(p) => {
            let Some(lit) = p.literal() else { return cx.unsupported(pat.syntax(), "matches! literal") };
            let literal = expr::lower_expr(cx, &ast::Expr::Literal(lit), out);
            let literal = if p.minus_token().is_some() { Expr::Unary("-", Box::new(literal)) } else { literal };
            if ty.is_primitive() { Expr::bin(value, "==", literal) } else { Expr::call(Expr::raw("Objects"), "equals", vec![value, literal]) }
        }
        ast::Pat::OrPat(p) => {
            let mut parts = p.pats().map(|p| match_pattern(cx, &p, value.clone(), ty, out));
            let first = parts.next().unwrap_or_else(|| Expr::raw("false"));
            parts.fold(first, |a, b| Expr::bin(a, "||", b))
        }
        ast::Pat::PathPat(p) if p.path().is_some_and(|p| p.syntax().text() == "None") && matches!(ty.strip_refs(), Ty::Option(_)) => {
            Expr::call(value, "isEmpty", vec![])
        }
        ast::Pat::IdentPat(p) if p.name().is_some_and(|n| n.text() == "None") && matches!(ty.strip_refs(), Ty::Option(_)) => {
            Expr::call(value, "isEmpty", vec![])
        }
        ast::Pat::TupleStructPat(p) => {
            let variant = p.path().map(|path| path.syntax().text().to_string()).unwrap_or_default();
            let fields: Vec<_> = p.fields().collect();
            if fields.len() != 1 {
                return cx.unsupported(pat.syntax(), "matches! variant pattern");
            }
            match (variant.rsplit("::").next().unwrap_or(""), ty.strip_refs()) {
                ("Some", Ty::Option(inner)) => {
                    let present = Expr::call(value.clone(), "isPresent", vec![]);
                    let item = Expr::call(value, "get", vec![]);
                    let test = match_pattern(cx, &fields[0], item, inner, out);
                    Expr::bin(present, "&&", test)
                }
                (variant @ ("Ok" | "Err"), Ty::Result(ok, err)) => {
                    let test = Expr::InstanceOf(Box::new(value.clone()), format!("{variant}<?, ?>"));
                    let cast = Expr::Cast(format!("{variant}<{}, {}>", ok.boxed(), err.boxed()), Box::new(value));
                    let item = Expr::call(cast, if variant == "Ok" { "value" } else { "error" }, vec![]);
                    let field_ty = if variant == "Ok" { ok } else { err };
                    Expr::bin(test, "&&", match_pattern(cx, &fields[0], item, field_ty, out))
                }
                _ => cx.unsupported(pat.syntax(), "matches! variant (pattern lowering not available)"),
            }
        }
        _ => cx.unsupported(pat.syntax(), "matches! pattern (pattern lowering not available)"),
    }
}
