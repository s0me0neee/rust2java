// Std Option and Result methods.

use super::*;

// Hook for std free functions (`key` as built by `calls::std_fn_key`, e.g. `Rc::new`).
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, c, key, ret, out);
    None
}

// Hook for std methods, before the receiver is lowered (`key` like `RefCell::borrow`).
pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, m, recv_ty, key, out);
    None
}

// ---- Option ----------------------------------------------------------------------------------

pub(super) fn lower_option_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("null"));
    let inner = opt_inner_from_recv(m, cx);
    match name {
        "is_some" => Some(Expr::call(recv, "isPresent", vec![])),
        "is_none" => Some(Expr::call(recv, "isEmpty", vec![])),
        "unwrap" => Some(Expr::call(recv, "orElseThrow", vec![])),
        "expect" => {
            let msg = Expr::New {
                ty: "RuntimeException".into(),
                args: vec![b0],
            };
            Some(Expr::call(
                recv,
                "orElseThrow",
                vec![Expr::Lambda {
                    params: vec![],
                    body: LambdaBody::Expr(Box::new(msg)),
                }],
            ))
        }
        "unwrap_or" => Some(Expr::call(recv, "orElse", vec![b0])),
        "unwrap_or_else" => Some(Expr::call(recv, "orElseGet", vec![b0])),
        "unwrap_or_default" => Some(Expr::call(
            recv,
            "orElse",
            vec![default_value(&inner.unwrap_or(Ty::Other("?".into())))],
        )),
        "map" | "and_then" | "filter" => {
            let java = if name == "and_then" { "flatMap" } else { name };
            Some(Expr::call(recv, java, vec![b0]))
        }
        "map_or" => {
            // Rust: map_or(default, f)
            let mapped = Expr::call(recv, "map", vec![b1]);
            Some(Expr::call(mapped, "orElse", vec![b0]))
        }
        "or" => Some(Expr::call(
            recv,
            "or",
            vec![Expr::Lambda {
                params: vec![],
                body: LambdaBody::Expr(Box::new(b0)),
            }],
        )),
        "or_else" => Some(Expr::call(recv, "or", vec![b0])),
        "ok_or" => {
            cx.needs_result = true;
            let v = cx.fresh("v");
            let ok_ty = inner
                .as_ref()
                .map(|t| t.boxed())
                .unwrap_or_else(|| "Object".into());
            let err_ty = match ret.strip_refs() {
                Ty::Result(_, e) => e.boxed(),
                _ => "Object".into(),
            };
            Some(Expr::raw(format!(
                "{}.<Result<{}, {}>>map({v} -> new Ok<>({v})).orElse(new Err<>({}))",
                render(&recv),
                ok_ty,
                err_ty,
                render(&b0)
            )))
        }
        "as_ref" | "as_mut" | "as_deref" | "cloned" | "copied" | "flatten" => Some(recv),
        "replace" => {
            let p = expr::lower_expr(cx, recv_ast, out);
            let ty = match opt_inner_of(cx, recv_ast) {
                Some(t) => format!("Optional<{}>", t.boxed()),
                None => "Optional<Object>".into(),
            };
            let n = push_local(cx, line, ty, Some(p), out, "prev");
            let assign = expr::place_assign(cx, recv_ast, "=", opt_of(b0), out);
            out.push(Stmt::new(line, StmtKind::Expr(assign)));
            Some(Expr::name(n))
        }
        "take" => {
            let p = expr::lower_expr(cx, recv_ast, out);
            let ty = match opt_inner_of(cx, recv_ast) {
                Some(t) => format!("Optional<{}>", t.boxed()),
                None => "Optional<Object>".into(),
            };
            let n = push_local(cx, line, ty, Some(p), out, "taken");
            let empty = opt_empty(opt_inner_from_recv(m, cx).as_ref());
            let assign = expr::place_assign(cx, recv_ast, "=", empty, out);
            out.push(Stmt::new(line, StmtKind::Expr(assign)));
            Some(Expr::name(n))
        }
        "is_some_and" => {
            let mapped = Expr::call(recv, "map", vec![b0]);
            Some(Expr::call(mapped, "orElse", vec![Expr::raw("false")]))
        }
        "is_none_or" => {
            let mapped = Expr::call(recv, "map", vec![b0]);
            Some(Expr::call(mapped, "orElse", vec![Expr::raw("true")]))
        }
        "map_or_else" => {
            // Rust: map_or_else(default_fn, f)
            let mapped = Expr::call(recv, "map", vec![b1]);
            Some(Expr::call(mapped, "orElseGet", vec![b0]))
        }
        _ => None,
    }
}

pub(super) fn opt_inner_of(cx: &Cx, e: &ast::Expr) -> Option<Ty> {
    opt_inner(&cx.ty(e))
}

pub(super) fn opt_inner_from_recv(m: &ast::MethodCallExpr, cx: &Cx) -> Option<Ty> {
    m.receiver().and_then(|r| opt_inner_of(cx, &r))
}

// ---- Result ----------------------------------------------------------------------------------

pub(super) fn lower_result_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (line, out, ret);
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    // A `parse()` / `fs::read_to_string()` / `from_str_radix` receiver already lowered to the
    // unwrapped value when it saw the `.unwrap()` call; pass it through.
    if matches!(name, "unwrap" | "expect") && is_unwrapped_source(cx, recv_ast) {
        return Some(recv);
    }
    match name {
        "is_ok" => Some(Expr::call(recv, "is_ok", vec![])),
        "is_err" => Some(Expr::call(recv, "is_err", vec![])),
        "ok" => Some(Expr::call(recv, "ok", vec![])),
        "err" => Some(Expr::call(recv, "err", vec![])),
        "unwrap" => Some(Expr::call(recv, "unwrap", vec![])),
        "expect" => Some(Expr::call(recv, "expect", vec![b0])),
        "map" | "map_err" | "and_then" => Some(Expr::call(recv, name, vec![b0])),
        "unwrap_or" => Some(Expr::call(recv, "unwrap_or", vec![b0])),
        "unwrap_or_else" => Some(Expr::call(recv, "unwrap_or_else", vec![b0])),
        "is_ok_and" => {
            let mapped = Expr::call(recv, "map", vec![b0]);
            Some(Expr::call(mapped, "unwrap_or", vec![Expr::raw("false")]))
        }
        "is_err_and" => {
            let err = Expr::call(recv, "err", vec![]);
            let mapped = Expr::call(err, "map", vec![b0]);
            Some(Expr::call(mapped, "orElse", vec![Expr::raw("false")]))
        }
        "clone" | "as_ref" | "as_mut" => Some(recv),
        _ => None,
    }
}

pub(super) fn is_unwrapped_source(cx: &Cx, recv_ast: &ast::Expr) -> bool {
    let _ = cx;
    match recv_ast {
        ast::Expr::MethodCallExpr(mc) => matches!(
            method_name(mc).as_str(),
            "parse"
                | "read_line"
                | "read_to_string"
                | "read_to_end"
                | "write_all"
                | "write"
                | "flush"
        ),
        // `write!`/`writeln!` perform the write and hand back a dummy `Ok`.
        ast::Expr::MacroExpr(m) => m.macro_call().is_some_and(|mc| {
            matches!(
                super::super::macros::macro_name(&mc).as_str(),
                "write" | "writeln"
            )
        }),
        ast::Expr::CallExpr(c) => {
            let text = c
                .expr()
                .map(|e| e.syntax().text().to_string())
                .unwrap_or_default();
            text.ends_with("read_to_string")
                || text.ends_with("from_str_radix")
                || text.ends_with("from_utf8")
                || text.ends_with("fs::write")
                || text.ends_with("write")
                || text.ends_with("File::open")
                || text.ends_with("File::create")
                || text.ends_with("read_dir")
        }
        _ => false,
    }
}
