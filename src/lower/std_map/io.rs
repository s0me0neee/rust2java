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
    match key {
        // Keeping PrintStream handles avoids losing pending output when a BufWriter is dropped.
        "BufReader::new"
        | "BufReader::with_capacity"
        | "BufWriter::new"
        | "BufWriter::with_capacity" => {
            let args = expr::call_args(cx, c, out);
            Some(args.into_iter().last().unwrap_or_else(|| Expr::raw("null")))
        }
        "File::open" => Some(file_handle(cx, c, ret, false, out)),
        "File::create" => Some(file_handle(cx, c, ret, true, out)),
        "fs::read_dir" => Some(read_dir(cx, c, ret, out)),
        _ => {
            let _ = ret;
            None
        }
    }
}

// Hook for std methods, before the receiver is lowered (`key` like `RefCell::borrow`).
pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let name = key.rsplit("::").next().unwrap_or(key);
    // `to_string` on an `OsString`/`Path` resolves through a blanket trait impl (no owner key).
    if is_os_string(recv_ty)
        && matches!(
            name,
            "to_string" | "to_string_lossy" | "display" | "to_str" | "into_string"
        )
    {
        return Some(os_string_method(cx, m, name, out));
    }
    let (owner, name) = key.split_once("::")?;
    let line = cx.line(m.syntax());
    match name {
        "lock" if is_handle_owner(owner) => Some(reader(cx, m, out)),
        "read_line" if is_read_owner(owner) => Some(read_into(cx, m, false, out)),
        "read_to_string" if is_read_owner(owner) => Some(read_into(cx, m, true, out)),
        "read_to_end" if is_read_owner(owner) => Some(read_to_end(cx, m, out)),
        "read" if is_read_owner(owner) => Some(read_bytes(cx, m, out)),
        "flush" if is_handle_owner(owner) => Some(flush(cx, m, recv_ty, out)),
        "write_all" | "write" if is_write_owner(owner) => Some(write_all(cx, m, recv_ty, out)),
        "file_name" | "path" if owner == "DirEntry" => dir_entry_method(cx, m, name, out),
        _ => {
            let _ = (recv_ty, line);
            None
        }
    }
}

// ---- handles --------------------------------------------------------------------------------

fn is_read_owner(owner: &str) -> bool {
    matches!(
        owner,
        "Stdin" | "StdinLock" | "BufReader" | "File" | "Read" | "BufRead"
    )
}

fn is_write_owner(owner: &str) -> bool {
    matches!(
        owner,
        "Stdout" | "StdoutLock" | "Stderr" | "StderrLock" | "BufWriter" | "File" | "Write"
    )
}

fn is_handle_owner(owner: &str) -> bool {
    is_read_owner(owner) || is_write_owner(owner)
}

fn reader(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    lower_receiver(cx, m, out)
}

fn lower_receiver(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    m.receiver()
        .map(|r| expr::lower_expr(cx, &r, out))
        .unwrap_or_else(|| Expr::raw("null"))
}

fn line_arg(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let arg = arg_asts(m).into_iter().next()?;
    Some(expr::lower_expr(cx, &ref_inner(&arg), out))
}

// Decode only at the String boundary so text reads retain Rust's byte counts and UTF-8 errors.
fn read_into(cx: &mut Cx, m: &ast::MethodCallExpr, to_string: bool, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let rdr = lower_receiver(cx, m, out);
    let place = line_arg(cx, m, out).unwrap_or_else(|| Expr::raw("null"));
    let ps = render(&place);
    cx.import("java.io.*");
    cx.import("java.nio.charset.StandardCharsets");
    let rs = format!("((InputStream) {})", render(&rdr));
    let text = cx.fresh("text");
    let ch = cx.fresh("ch");
    let ex = cx.fresh("ex");
    let stop = if to_string {
        String::new()
    } else {
        format!(" if ({ch} == 10) break;")
    };
    let read = format!(
        "ByteArrayOutputStream {text} = new ByteArrayOutputStream(); for (int {ch}; ({ch} = {rs}.read()) != -1;) {{ {text}.write({ch});{stop} }} {ps} += StandardCharsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap({text}.toByteArray())).toString();"
    );
    let e = ast::Expr::MethodCallExpr(m.clone());
    if unwrapped(cx, &e) {
        let dropped = unwrap_in_stmt(&e);
        let count = cx.fresh("count");
        let decl = if dropped {
            String::new()
        } else {
            format!("int {count}; ")
        };
        let assign = if dropped {
            String::new()
        } else {
            format!(" {count} = {text}.size();")
        };
        push_raw(
            out,
            line,
            format!(
                "{decl}try {{ {read}{assign} }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return if dropped {
            Expr::raw("null")
        } else {
            Expr::name(count)
        };
    }
    cx.needs_result = true;
    let result = cx.fresh("read");
    let ret = cx.ty(&e);
    push_raw(
        out,
        line,
        format!(
            "{} {result}; try {{ {read} {result} = new Ok<>({text}.size()); }} catch (IOException {ex}) {{ {result} = new Err<>({ex}); }}",
            result_java(&ret)
        ),
    );
    Expr::name(result)
}

fn read_to_end(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let rdr = lower_receiver(cx, m, out);
    let place = line_arg(cx, m, out).unwrap_or_else(|| Expr::raw("null"));
    cx.import("java.io.*");
    let r = format!("((InputStream) {})", render(&rdr));
    let bytes = cx.fresh("bytes");
    let i = cx.fresh("i");
    let ex = cx.fresh("ex");
    let read = format!(
        "byte[] {bytes} = {r}.readAllBytes(); {}.addAll(IntStream.range(0, {bytes}.length).map({i} -> {bytes}[{i}] & 255).boxed().toList());",
        render(&place)
    );
    let e = ast::Expr::MethodCallExpr(m.clone());
    if unwrapped(cx, &e) {
        let count = cx.fresh("count");
        push_raw(
            out,
            line,
            format!(
                "int {count}; try {{ {read} {count} = {bytes}.length; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return if unwrap_in_stmt(&e) {
            Expr::raw("null")
        } else {
            Expr::name(count)
        };
    }
    cx.needs_result = true;
    let result = cx.fresh("read");
    let ret = cx.ty(&e);
    push_raw(
        out,
        line,
        format!(
            "{} {result}; try {{ {read} {result} = new Ok<>({bytes}.length); }} catch (IOException {ex}) {{ {result} = new Err<>({ex}); }}",
            result_java(&ret)
        ),
    );
    Expr::name(result)
}

// `r.read(&mut buf)`: fills a prefix of `buf` and yields the count, 0 at end of input.
fn read_bytes(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let rdr = lower_receiver(cx, m, out);
    let place = render(&line_arg(cx, m, out).unwrap_or_else(|| Expr::raw("null")));
    cx.import("java.io.*");
    let r = format!("((InputStream) {})", render(&rdr));
    let (bytes, count, i, ex) = (
        cx.fresh("bytes"),
        cx.fresh("count"),
        cx.fresh("i"),
        cx.fresh("ex"),
    );
    let read = format!(
        "byte[] {bytes} = new byte[{place}.size()]; {count} = Math.max(0, {r}.read({bytes})); \
         for (int {i} = 0; {i} < {count}; {i}++) {place}.set({i}, {bytes}[{i}] & 255);"
    );
    let e = ast::Expr::MethodCallExpr(m.clone());
    if unwrapped(cx, &e) {
        push_raw(
            out,
            line,
            format!(
                "int {count}; try {{ {read} }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return if unwrap_in_stmt(&e) {
            Expr::raw("null")
        } else {
            Expr::name(count)
        };
    }
    cx.needs_result = true;
    let result = cx.fresh("read");
    let ret = cx.ty(&e);
    push_raw(
        out,
        line,
        format!(
            "{} {result}; try {{ int {count}; {read} {result} = new Ok<>({count}); }} catch (IOException {ex}) {{ {result} = new Err<>({ex}); }}",
            result_java(&ret)
        ),
    );
    Expr::name(result)
}

fn flush(cx: &mut Cx, m: &ast::MethodCallExpr, recv_ty: &Ty, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let recv = lower_receiver(cx, m, out);
    let recv = output_handle(cx, recv_ty, recv);
    out.push(Stmt::new(
        line,
        StmtKind::Expr(Expr::call(recv.clone(), "flush", vec![])),
    ));
    finish_write(cx, m, line, recv, Expr::raw("null"), out)
}

fn write_all(cx: &mut Cx, m: &ast::MethodCallExpr, recv_ty: &Ty, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let recv = lower_receiver(cx, m, out);
    let recv = output_handle(cx, recv_ty, recv);
    let Some(arg) = arg_asts(m).into_iter().next() else {
        return cx.unsupported(m.syntax(), "write without bytes");
    };
    let value = expr::lower_expr(cx, &arg, out);
    let bytes = byte_array(cx, value);
    let bytes = push_local(cx, line, "byte[]".into(), Some(bytes), out, "bytes");
    push_raw(
        out,
        line,
        format!("({}).write({bytes}, 0, {bytes}.length);", render(&recv)),
    );
    let value = if method_name(m) == "write" {
        Expr::raw(format!("{bytes}.length"))
    } else {
        Expr::raw("null")
    };
    finish_write(cx, m, line, recv, value, out)
}

// Java byte arrays are needed only at the I/O boundary; Rust byte slices stay List<Integer>.
fn byte_array(cx: &mut Cx, value: Expr) -> Expr {
    cx.import("java.io.*");
    let b = cx.fresh("b");
    let n = cx.fresh("n");
    let other = cx.fresh("other");
    Expr::raw(format!(
        "({}).stream().collect(ByteArrayOutputStream::new, ({b}, {n}) -> {b}.write({n}), ({b}, {other}) -> {b}.writeBytes({other}.toByteArray())).toByteArray()",
        render(&value)
    ))
}

fn finish_write(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    line: u32,
    recv: Expr,
    value: Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let e = ast::Expr::MethodCallExpr(m.clone());
    if parent_is_stmt(&e) {
        return Expr::raw("null");
    }
    let ret = cx.ty(&e);
    let result = write_status(cx, line, recv, value, &ret, out);
    if unwrapped(cx, &e) {
        Expr::call(result, "unwrap", vec![])
    } else {
        result
    }
}

// PrintStream stores write failures instead of throwing; expose them as Rust Results.
pub(crate) fn write_status(
    cx: &mut Cx,
    line: u32,
    recv: Expr,
    value: Expr,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Expr {
    cx.needs_result = true;
    cx.import("java.io.IOException");
    let status = Expr::raw(format!(
        "({}).checkError() ? new Err<>(new IOException(\"write failed\")) : new Ok<>({})",
        render(&recv),
        render(&value)
    ));
    Expr::name(push_local(
        cx,
        line,
        result_java(ret),
        Some(status),
        out,
        "written",
    ))
}

// `read_line(..).unwrap();` as a statement: nobody wants the count.
fn unwrap_in_stmt(e: &ast::Expr) -> bool {
    e.syntax()
        .parent()
        .and_then(ast::MethodCallExpr::cast)
        .filter(|m| matches!(method_name(m).as_str(), "unwrap" | "expect"))
        .and_then(|m| m.syntax().parent())
        .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
}

fn unwrapped(cx: &Cx, e: &ast::Expr) -> bool {
    matches!(consumed_by(cx, e).as_deref(), Some("unwrap" | "expect"))
}

// ---- files ----------------------------------------------------------------------------------

// PrintStream supports both raw byte writes and formatting without a separate drop-time flush.
fn file_handle(
    cx: &mut Cx,
    c: &ast::CallExpr,
    ret: &Ty,
    create: bool,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let args = expr::call_args(cx, c, out);
    let path = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let call = if create {
        format!(
            "new PrintStream(Files.newOutputStream(Path.of({})), false, java.nio.charset.StandardCharsets.UTF_8)",
            render(&path)
        )
    } else {
        format!(
            "new BufferedInputStream(Files.newInputStream(Path.of({})))",
            render(&path)
        )
    };
    let e = ast::Expr::CallExpr(c.clone());
    let ty = if create { "PrintStream" } else { "InputStream" };
    if unwrapped(cx, &e) {
        let name = cx.fresh(if create { "out" } else { "rdr" });
        let ex = cx.fresh("ex");
        push_raw(
            out,
            line,
            format!(
                "{ty} {name}; try {{ {name} = {call}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return Expr::name(name);
    }
    let _ = ret;
    let name = cx.fresh("file");
    let ex = cx.fresh("ex");
    cx.needs_result = true;
    push_raw(
        out,
        line,
        format!(
            "Result<Object, Object> {name}; try {{ {name} = new Ok<>({call}); }} catch (IOException {ex}) {{ {name} = new Err<>({ex}); }}"
        ),
    );
    Expr::name(name)
}

// `fs::read_dir` -> sorted stream of `Path`, each wrapped as an `Ok` like Rust's `DirEntry`.
fn read_dir(cx: &mut Cx, c: &ast::CallExpr, ret: &Ty, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let args = expr::call_args(cx, c, out);
    let path = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let p = cx.fresh("p");
    let call = format!(
        "Files.list(Path.of({})).sorted().map({p} -> (Result<Object, Object>) new Ok<Object, Object>({p}))",
        render(&path)
    );
    let e = ast::Expr::CallExpr(c.clone());
    if unwrapped(cx, &e) {
        let name = cx.fresh("entries");
        let ex = cx.fresh("ex");
        push_raw(
            out,
            line,
            format!(
                "Stream<Result<Object, Object>> {name}; try {{ {name} = {call}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"
            ),
        );
        return Expr::name(name);
    }
    let _ = ret;
    let name = cx.fresh("entries");
    let ex = cx.fresh("ex");
    cx.needs_result = true;
    push_raw(
        out,
        line,
        format!(
            "Result<Stream<Result<Object, Object>>, Object> {name}; try {{ {name} = new Ok<>({call}); }} catch (IOException {ex}) {{ {name} = new Err<>({ex}); }}"
        ),
    );
    Expr::name(name)
}

fn dir_entry_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let recv = lower_receiver(cx, m, out);
    Some(match name {
        "file_name" => Expr::call(
            Expr::Cast("Path".into(), Box::new(recv)),
            "getFileName",
            vec![],
        ),
        _ => recv,
    })
}

// `OsString`/`Path` accessors on `fs::read_dir` entries.
fn is_os_string(ty: &Ty) -> bool {
    matches!(
        ty.strip_refs(),
        Ty::Other(s)
            if s.starts_with("OsStr")
                || s.starts_with("OsString")
                || s.starts_with("Path")
                || s.starts_with("Cow")
    )
}

fn os_string_method(cx: &mut Cx, m: &ast::MethodCallExpr, name: &str, out: &mut Vec<Stmt>) -> Expr {
    let recv = lower_receiver(cx, m, out);
    let text = if matches!(&recv, Expr::Call { name, .. } if name == "toString") {
        recv
    } else {
        Expr::call(recv, "toString", vec![])
    };
    match name {
        "to_str" => opt_of(text),
        "into_string" => {
            let ret = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
            wrap_ok(cx, cx.line(m.syntax()), &ret, text, out)
        }
        _ => text,
    }
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

// File handles extracted through Result have Object as their declared Java type.
pub(crate) fn output_handle(cx: &mut Cx, ty: &Ty, value: Expr) -> Expr {
    if matches!(ty.strip_refs(), Ty::Other(s) if s == "File" || s.starts_with("BufWriter")) {
        cx.import("java.io.PrintStream");
        Expr::Cast("PrintStream".into(), Box::new(value))
    } else {
        value
    }
}

pub(crate) fn is_io_target(ty: &Ty) -> bool {
    matches!(
        ty.strip_refs(),
        Ty::Other(s)
            if s.starts_with("Stdout")
                || s.starts_with("Stderr")
                || s.starts_with("BufWriter")
                || s == "File"
    )
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
    let _ = (recv_ast, ret, line);
    match name {
        "lock" | "by_ref" => Some(reader(cx, m, out)),
        "read_line" => Some(read_into(cx, m, false, out)),
        "read_to_string" => Some(read_into(cx, m, true, out)),
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
    let keep = !matches!(usage.as_deref(), Some("unwrap" | "expect"));
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
    let is_bytes = c
        .arg_list()
        .and_then(|a| a.args().nth(1))
        .is_some_and(|a| matches!(cx.ty(&a).strip_refs(), Ty::Vec(_)));
    let (method, contents) = if is_bytes {
        ("write", byte_array(cx, contents))
    } else {
        ("writeString", contents)
    };
    let write = format!(
        "Files.{method}(Path.of({}), {})",
        render(&path),
        render(&contents)
    );
    let usage = consumed_by(cx, &ast::Expr::CallExpr(c.clone()));
    if matches!(usage.as_deref(), Some("unwrap" | "expect")) {
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

// Split only on LF, stripping CR only in CRLF, just like Rust's BufRead::lines.
pub(crate) fn lines_stream(cx: &mut Cx, recv: Expr) -> Expr {
    cx.needs_result = true;
    cx.import("java.io.*");
    cx.import("java.nio.charset.StandardCharsets");
    let r = format!("((InputStream) {})", render(&recv));
    let text = cx.fresh("text");
    let ch = cx.fresh("ch");
    let bytes = cx.fresh("bytes");
    let len = cx.fresh("len");
    let ex = cx.fresh("ex");
    Expr::raw(format!(
        "Stream.<Result<String, Object>>generate(() -> {{ try {{ ByteArrayOutputStream {text} = new ByteArrayOutputStream(); for (int {ch}; ({ch} = {r}.read()) != -1;) {{ {text}.write({ch}); if ({ch} == 10) break; }} byte[] {bytes} = {text}.toByteArray(); int {len} = {bytes}.length; if ({len} == 0) return null; if ({bytes}[{len} - 1] == 10) {{ {len}--; if ({len} > 0 && {bytes}[{len} - 1] == 13) {len}--; }} return new Ok<>(StandardCharsets.UTF_8.newDecoder().decode(java.nio.ByteBuffer.wrap({bytes}, 0, {len})).toString()); }} catch (IOException {ex}) {{ return new Err<>({ex}); }} }}).takeWhile(Objects::nonNull)"
    ))
}

// Raw byte reads must not pass through a character decoder, even for invalid UTF-8 input.
pub(crate) fn bytes_stream(cx: &mut Cx, recv: Expr) -> Expr {
    cx.needs_result = true;
    cx.import("java.io.*");
    let r = format!("((InputStream) {})", render(&recv));
    let ex = cx.fresh("ex");
    let b = cx.fresh("b");
    Expr::raw(format!(
        "Stream.<Result<Integer, Object>>generate(() -> {{ try {{ return new Ok<>({r}.read()); }} catch (IOException {ex}) {{ return new Err<>({ex}); }} }}).takeWhile({b} -> {b}.is_err() || {b}.unwrap() != -1)"
    ))
}

pub(crate) fn is_reader_source(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(s) if s.contains("Stdin") || s.contains("BufReader"))
}
