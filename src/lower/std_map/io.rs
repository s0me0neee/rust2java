// Std stdin/stdout/stderr, BufRead/Write, fs, env, process, time.

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

// ---- stdin / stdout -------------------------------------------------------------------------

pub(super) fn is_stdin(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = cx;
    match e {
        ast::Expr::CallExpr(c) => {
            let text = c
                .expr()
                .map(|c| c.syntax().text().to_string())
                .unwrap_or_default();
            text.ends_with("stdin")
        }
        ast::Expr::MethodCallExpr(mc) => match method_name(mc).as_str() {
            "lock" | "by_ref" => mc.receiver().is_some_and(|r| is_stdin(cx, &r)),
            _ => false,
        },
        _ => false,
    }
}

pub(super) fn is_stdout(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = cx;
    match e {
        ast::Expr::CallExpr(c) => c
            .expr()
            .map(|c| c.syntax().text().to_string().ends_with("stdout"))
            .unwrap_or(false),
        _ => false,
    }
}

pub(super) fn stdio_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    recv_ast: &ast::Expr,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (recv_ast, ret);
    match name {
        "read_line" | "read_to_string" => {
            let arg = arg_asts(m).into_iter().next()?;
            let place_ast = ref_inner(&arg);
            need_stdin(cx);
            let place = expr::lower_expr(cx, &place_ast, out);
            let ps = render(&place);
            let l = cx.fresh("line");
            let used = matches!(
                consumed_by(cx, &ast::Expr::MethodCallExpr(m.clone())).as_deref(),
                Some("unwrap" | "expect")
            );
            if used {
                if name == "read_line" {
                    let tmp = push_local(
                        cx,
                        line,
                        "String".into(),
                        Some(Expr::raw("STDIN.lines().findFirst().orElse(null)")),
                        out,
                        "line",
                    );
                    let n = push_local(cx, line, "int".into(), Some(Expr::raw("0")), out, "n");
                    push_raw(
                        out,
                        line,
                        format!(
                            "if ({tmp} != null) {{ {ps} += {tmp} + \"\\n\"; {n} = {tmp}.length() + 1; }}"
                        ),
                    );
                    Some(Expr::name(n))
                } else {
                    let tmp = push_local(
                        cx,
                        line,
                        "String".into(),
                        Some(Expr::raw(
                            "STDIN.lines().map(l -> l + \"\\n\").collect(Collectors.joining())",
                        )),
                        out,
                        "rest",
                    );
                    push_raw(out, line, format!("{ps} += {tmp};"));
                    let n = push_local(
                        cx,
                        line,
                        "int".into(),
                        Some(Expr::call(Expr::name(tmp), "length", vec![])),
                        out,
                        "n",
                    );
                    Some(Expr::name(n))
                }
            } else {
                let app = if name == "read_line" {
                    Expr::raw(format!(
                        "STDIN.lines().findFirst().map({l} -> {l} + \"\\n\").orElse(\"\")"
                    ))
                } else {
                    Expr::raw(format!(
                        "STDIN.lines().map({l} -> {l} + \"\\n\").collect(Collectors.joining())"
                    ))
                };
                let assign = expr::place_assign(cx, &place_ast, "+=", app, out);
                out.push(Stmt::new(line, StmtKind::Expr(assign)));
                Some(Expr::raw("null"))
            }
        }
        "lock" | "by_ref" => Some(Expr::raw("STDIN")),
        _ => None,
    }
}

pub(super) fn duration_method(cx: &mut Cx, name: &str, recv: Expr, args: &[Expr]) -> Option<Expr> {
    let _ = (cx, args);
    Some(match name {
        "as_secs" => Expr::call(recv, "toSeconds", vec![]),
        "as_millis" => Expr::call(recv, "toMillis", vec![]),
        "as_nanos" => Expr::call(recv, "toNanos", vec![]),
        "as_secs_f64" => Expr::bin(Expr::call(recv, "toNanos", vec![]), "/", Expr::raw("1e9")),
        "as_millis_f64" => Expr::bin(Expr::call(recv, "toNanos", vec![]), "/", Expr::raw("1e6")),
        _ => return None,
    })
}

// ---- I/O -------------------------------------------------------------------------------------

pub(super) fn fs_read(
    cx: &mut Cx,
    c: &ast::CallExpr,
    ret: &Ty,
    path: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let usage = consumed_by(cx, &ast::Expr::CallExpr(c.clone()));
    let keep = !matches!(usage.as_deref(), Some("unwrap" | "expect"))
        && !parent_is_stmt(&ast::Expr::CallExpr(c.clone()));
    let read = Expr::raw(format!("Files.readString(Path.of({}))", render(&path)));
    if keep {
        return wrap_try_ok(cx, line, ret, read, "IOException", out);
    }
    let text = cx.fresh("text");
    let ex = cx.fresh("ex");
    push_raw(
        out,
        line,
        format!(
            "String {text}; try {{ {text} = {}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}",
            render(&read)
        ),
    );
    Expr::name(text)
}

pub(super) fn parent_is_stmt(e: &ast::Expr) -> bool {
    e.syntax()
        .parent()
        .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
}

pub(super) fn fs_write(
    cx: &mut Cx,
    c: &ast::CallExpr,
    ret: &Ty,
    path: Expr,
    contents: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let write = format!(
        "Files.writeString(Path.of({}), {})",
        render(&path),
        render(&contents)
    );
    let usage = consumed_by(cx, &ast::Expr::CallExpr(c.clone()));
    if matches!(usage.as_deref(), Some("unwrap" | "expect"))
        || parent_is_stmt(&ast::Expr::CallExpr(c.clone()))
    {
        let ex = cx.fresh("ex");
        push_raw(
            out,
            line,
            format!(
                "try {{ {write}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return Expr::raw("null");
    }
    cx.needs_result = true;
    let ty = result_java(ret);
    let n = cx.fresh("res");
    let ex = cx.fresh("ex");
    push_raw(
        out,
        line,
        format!(
            "{ty} {n}; try {{ {write}; {n} = new Ok<>(null); }} catch (IOException {ex}) {{ {n} = new Err<>({ex}); }}"
        ),
    );
    Expr::name(n)
}
