// std macros: println!/print!/eprintln!/format!/panic!/assert!/vec!/... (translated from arguments).

use ra_ap_syntax::{AstNode, ast};

use super::{Cx, fmt};
use crate::jir::{Expr, Stmt};

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
fn first_macro_arg(mc: &ast::MacroCall) -> String {
    let text = mc.token_tree().map(|t| t.syntax().text().to_string()).unwrap_or_default();
    let inner = text.trim().trim_start_matches(['(', '[', '{']).to_string();
    let mut depth = 0i32;
    let mut out = String::new();
    for c in inner.chars() {
        match c {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => break,
            _ => {}
        }
        out.push(c);
    }
    out.trim().to_string()
}

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
        "format" => match fmt::format_args_of(cx, &mc) {
            Some(fa) => fmt::lower_format(cx, &fa, out),
            None => cx.unsupported(mc.syntax(), "format! arguments"),
        },
        "write" | "writeln" => {
            let Some(fa) = fmt::format_args_of(cx, &mc) else {
                return cx.unsupported(mc.syntax(), "write! arguments");
            };
            let dest = first_macro_arg(&mc);
            let text = fmt::lower_format(cx, &fa, out);
            Expr::call(Expr::raw(dest), "append", vec![text])
        }
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
                None => Expr::New { ty: "ArrayList<>".into(), args: vec![] },
            }
        }
        _ => cx.unsupported(mc.syntax(), &format!("macro `{name}!`")),
    }
}
