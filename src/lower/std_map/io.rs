// Std stdin/stdout/stderr, BufRead/Write, fs, env, process, time.

use super::*;
use crate::lower::strip_parens;
use ra_ap_syntax::ast::{HasLoopBody, HasName};

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

// Decode only at the String boundary so text reads keep Rust's byte counts (invalid UTF-8 becomes
// U+FFFD instead of an error: accepted).
fn read_into(cx: &mut Cx, m: &ast::MethodCallExpr, to_string: bool, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let rdr = lower_receiver(cx, m, out);
    let place = line_arg(cx, m, out).unwrap_or_else(|| Expr::raw("null"));
    let ps = render(&place);
    cx.import("java.io.*");
    if m.receiver().is_some_and(|r| is_line_stdin(cx, &r)) {
        let rs = render(&rdr);
        return read_line_mode(cx, m, to_string, &rs, &ps, line, out);
    }
    cx.import("java.nio.charset.StandardCharsets");
    let rs = input_stream(&rdr);
    let text = cx.fresh("text");
    let ch = cx.fresh("ch");
    let ex = cx.fresh("ex");
    let stop = if to_string {
        String::new()
    } else {
        format!(" if ({ch} == 10) break;")
    };
    let read = format!(
        "ByteArrayOutputStream {text} = new ByteArrayOutputStream(); for (int {ch}; ({ch} = {rs}.read()) != -1;) {{ {text}.write({ch});{stop} }} {ps} += {text}.toString(StandardCharsets.UTF_8);"
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

// read_line/read_to_string on a line-mode STDIN. `lines()` drops the terminator, so `\n` is put
// back (a final line without one gains it, and `\r\n` loses the `\r`: accepted).
fn read_line_mode(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    to_string: bool,
    reader: &str,
    place: &str,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let read = if to_string {
        format!("{reader}.lines().map(l -> l + \"\\n\").collect(Collectors.joining())")
    } else {
        format!("{reader}.lines().findFirst().map(l -> l + \"\\n\").orElse(\"\")")
    };
    let e = ast::Expr::MethodCallExpr(m.clone());
    let unwrapped = unwrapped(cx, &e);
    if unwrapped && unwrap_in_stmt(&e) {
        out.push(Stmt::new(
            line,
            StmtKind::Raw(format!("{place} += {read};")),
        ));
        return Expr::raw("null");
    }
    let text = cx.fresh("text");
    push_raw(
        out,
        line,
        format!("String {text} = {read}; {place} += {text};"),
    );
    let count = Expr::call(Expr::name(&text), "length", vec![]);
    if unwrapped {
        return count;
    }
    cx.needs_result = true;
    let ret = cx.ty(&e);
    let result = cx.fresh("read");
    push_raw(
        out,
        line,
        format!(
            "{} {result} = new Ok<>({});",
            result_java(&ret),
            render(&count)
        ),
    );
    Expr::name(result)
}

fn read_to_end(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(m.syntax());
    let rdr = lower_receiver(cx, m, out);
    let place = line_arg(cx, m, out).unwrap_or_else(|| Expr::raw("null"));
    cx.import("java.io.*");
    let r = input_stream(&rdr);
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
    let r = input_stream(&rdr);
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

// stdin is already an InputStream, so its byte reads need no cast.
fn input_stream(recv: &Expr) -> String {
    let text = render(recv);
    if text == "STDIN" {
        text
    } else {
        format!("((InputStream) {text})")
    }
}

// Split only on LF, stripping CR only in CRLF, just like Rust's BufRead::lines.
pub(crate) fn lines_stream(cx: &mut Cx, recv: Expr, line_mode: bool, plain: bool) -> Expr {
    cx.import("java.io.*");
    if line_mode {
        if !plain {
            cx.needs_result = true;
            return Expr::call(
                recv,
                "lines().<Result<String, Object>>map",
                vec![Expr::raw("Ok::new")],
            );
        }
        return Expr::call(recv, "lines", vec![]);
    }
    cx.needs_result = true;
    cx.import("java.nio.charset.StandardCharsets");
    let r = input_stream(&recv);
    let text = cx.fresh("text");
    let ch = cx.fresh("ch");
    let bytes = cx.fresh("bytes");
    let len = cx.fresh("len");
    let ex = cx.fresh("ex");
    Expr::raw(format!(
        "Stream.<Result<String, Object>>generate(() -> {{ try {{ ByteArrayOutputStream {text} = new ByteArrayOutputStream(); for (int {ch}; ({ch} = {r}.read()) != -1;) {{ {text}.write({ch}); if ({ch} == 10) break; }} byte[] {bytes} = {text}.toByteArray(); int {len} = {bytes}.length; if ({len} == 0) return null; if ({bytes}[{len} - 1] == 10) {{ {len}--; if ({len} > 0 && {bytes}[{len} - 1] == 13) {len}--; }} return new Ok<>(new String({bytes}, 0, {len}, StandardCharsets.UTF_8)); }} catch (IOException {ex}) {{ return new Err<>({ex}); }} }}).takeWhile(Objects::nonNull)"
    ))
}

// Raw byte reads must not pass through a character decoder, even for invalid UTF-8 input.
pub(crate) fn bytes_stream(cx: &mut Cx, recv: Expr) -> Expr {
    cx.needs_result = true;
    cx.import("java.io.*");
    let r = input_stream(&recv);
    let ex = cx.fresh("ex");
    let b = cx.fresh("b");
    Expr::raw(format!(
        "Stream.<Result<Integer, Object>>generate(() -> {{ try {{ return new Ok<>({r}.read()); }} catch (IOException {ex}) {{ return new Err<>({ex}); }} }}).takeWhile({b} -> {b}.is_err() || {b}.unwrap() != -1)"
    ))
}

// ---- line-mode `lines()` items --------------------------------------------------------------
//
// `STDIN.lines()` never yields an Err, so when a program only ever unwraps the items the Java
// stream can carry plain Strings and the entire Result wrapper disappears. An I/O failure becomes
// an UncheckedIOException instead of an Err item (accepted: both abort the program).

fn is_lines_call(cx: &Cx, m: &ast::MethodCallExpr) -> bool {
    method_name(m) == "lines" && m.receiver().is_some_and(|r| is_line_stdin(cx, &r))
}

fn lines_call(cx: &Cx, e: &ast::Expr) -> Option<ast::MethodCallExpr> {
    match strip_parens(e) {
        ast::Expr::MethodCallExpr(m) if is_lines_call(cx, &m) => Some(m),
        _ => None,
    }
}

fn first_arg(m: &ast::MethodCallExpr) -> Option<ast::Expr> {
    m.arg_list().and_then(|al| al.args().next())
}

fn is_receiver(m: &ast::MethodCallExpr, node: &ra_ap_syntax::SyntaxNode) -> bool {
    m.receiver()
        .is_some_and(|r| r.syntax().text_range() == node.text_range())
}

fn is_result_ok(a: &ast::Expr) -> bool {
    matches!(strip_parens(a), ast::Expr::PathExpr(p)
        if p.path().is_some_and(|p| p.syntax().text().to_string().ends_with("Result::ok")))
}

// `|l| l.unwrap()` / `|l| l.expect(..)`: the closure only strips the phantom Result.
fn closure_unwraps_param(cx: &Cx, a: &ast::Expr) -> bool {
    let ast::Expr::ClosureExpr(c) = strip_parens(a) else {
        return false;
    };
    let Some(ast::Pat::IdentPat(ip)) = c
        .param_list()
        .and_then(|pl| pl.params().next())
        .and_then(|p| p.pat())
    else {
        return false;
    };
    let Some(local) = cx.sema.to_def(&ip) else {
        return false;
    };
    let Some(ast::Expr::MethodCallExpr(m)) = c.body().map(|b| strip_parens(&b)) else {
        return false;
    };
    matches!(method_name(&m).as_str(), "unwrap" | "expect")
        && m.receiver()
            .is_some_and(|r| cx.local_of_path(&r) == Some(local))
}

// A use of a loop binding that only strips the phantom Result (`line.unwrap()`, `line?`).
fn unwraps_only(pe: &ast::PathExpr) -> bool {
    let parent = pe.syntax().parent();
    if parent
        .as_ref()
        .is_some_and(|p| ast::TryExpr::can_cast(p.kind()))
    {
        return true;
    }
    let Some(m) = parent.and_then(ast::MethodCallExpr::cast) else {
        return false;
    };
    matches!(method_name(&m).as_str(), "unwrap" | "expect") && is_receiver(&m, pe.syntax())
}

fn for_uses_unwrap(cx: &Cx, f: &ast::ForExpr) -> bool {
    let (Some(ast::Pat::IdentPat(ip)), Some(body), Some(iter)) =
        (f.pat(), f.loop_body(), f.iterable())
    else {
        return false;
    };
    if lines_call(cx, &iter).is_none() {
        return false;
    }
    let Some(local) = cx.sema.to_def(&ip) else {
        return false;
    };
    let mut uses = 0;
    for node in cx.expanded_descendants(body.syntax()) {
        let Some(pe) = ast::PathExpr::cast(node) else {
            continue;
        };
        if cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) != Some(local) {
            continue;
        }
        uses += 1;
        if !unwraps_only(&pe) {
            return false;
        }
    }
    uses > 0
}

// `lines()` items are plain Strings when the consumer only ever unwraps them.
pub(crate) fn plain_lines(cx: &Cx, m: &ast::MethodCallExpr) -> bool {
    if !is_lines_call(cx, m) {
        return false;
    }
    let Some(parent) = m.syntax().parent() else {
        return false;
    };
    if let Some(f) = ast::ForExpr::cast(parent.clone()) {
        return for_uses_unwrap(cx, &f);
    }
    // A pulled iterator (`let mut it = ..lines();`) whose every use is `next()` + unwraps.
    if let Some(l) = ast::LetStmt::cast(parent.clone()) {
        let Some(ast::Pat::IdentPat(ip)) = l.pat() else {
            return false;
        };
        let Some(local) = cx.sema.to_def(&ip) else {
            return false;
        };
        let Some(scope) = cx.enclosing_fn(ip.syntax()) else {
            return false;
        };
        let init = ast::Expr::MethodCallExpr(m.clone());
        return plain_iterator(cx, &init, local, &scope);
    }
    let Some(c) = ast::MethodCallExpr::cast(parent) else {
        return false;
    };
    match method_name(&c).as_str() {
        "flatten" => true,
        "map" | "inspect" => first_arg(&c).is_some_and(|a| closure_unwraps_param(cx, &a)),
        "map_while" => first_arg(&c).is_some_and(|a| is_result_ok(&a)),
        _ => false,
    }
}

// The same decision, from the `for` loop's iterable expression.
pub(crate) fn plain_lines_iterable(cx: &Cx, iterable: &ast::Expr) -> bool {
    lines_call(cx, iterable).is_some_and(|m| plain_lines(cx, &m))
}

// The adaptor that would unwrap the phantom Result is dropped entirely.
pub(crate) fn plain_adaptor(
    cx: &Cx,
    name: &str,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
) -> bool {
    let Some(lines) = lines_call(cx, recv_ast) else {
        return false;
    };
    if !plain_lines(cx, &lines) {
        return false;
    }
    match name {
        "flatten" => true,
        "map" | "inspect" => first_arg(m).is_some_and(|a| closure_unwraps_param(cx, &a)),
        "map_while" => first_arg(m).is_some_and(|a| is_result_ok(&a)),
        _ => false,
    }
}

// `let mut it = stdin.lines();` when every use pulls `it.next().unwrap().unwrap()`.
pub(crate) fn plain_iterator(
    cx: &Cx,
    init: &ast::Expr,
    local: ra_ap_hir::Local<'_>,
    scope: &ra_ap_syntax::SyntaxNode,
) -> bool {
    if lines_call(cx, init).is_none() {
        return false;
    }
    let mut uses = 0;
    for node in cx.expanded_descendants(scope) {
        let Some(pe) = ast::PathExpr::cast(node) else {
            continue;
        };
        if cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) != Some(local) {
            continue;
        }
        uses += 1;
        if !next_double_unwrap(&pe) {
            return false;
        }
    }
    uses > 0
}

fn next_double_unwrap(pe: &ast::PathExpr) -> bool {
    let Some(next) = pe.syntax().parent().and_then(ast::MethodCallExpr::cast) else {
        return false;
    };
    if method_name(&next) != "next" || !is_receiver(&next, pe.syntax()) {
        return false;
    }
    let Some(first) = next.syntax().parent().and_then(ast::MethodCallExpr::cast) else {
        return false;
    };
    if !matches!(method_name(&first).as_str(), "unwrap" | "expect") {
        return false;
    }
    first
        .syntax()
        .parent()
        .and_then(ast::MethodCallExpr::cast)
        .is_some_and(|second| matches!(method_name(&second).as_str(), "unwrap" | "expect"))
}

fn local_ident(cx: &Cx, local: ra_ap_hir::Local<'_>) -> Option<ast::IdentPat> {
    local.primary_source(cx.db).into_ident_pat()
}

fn local_is_plain_iterator(cx: &Cx, local: ra_ap_hir::Local<'_>) -> bool {
    let Some(ip) = local_ident(cx, local) else {
        return false;
    };
    let Some(l) = ip.syntax().parent().and_then(ast::LetStmt::cast) else {
        return false;
    };
    let Some(init) = l.initializer() else {
        return false;
    };
    let Some(scope) = cx.enclosing_fn(ip.syntax()) else {
        return false;
    };
    plain_iterator(cx, &init, local, &scope)
}

// Is this `Result<String, _>` expression actually a plain line of a line-mode stdin stream?
pub(crate) fn phantom_line_result(cx: &Cx, e: &ast::Expr) -> bool {
    match strip_parens(e) {
        ast::Expr::PathExpr(pe) => {
            let Some(local) = cx.local_of_path(&ast::Expr::PathExpr(pe.clone())) else {
                return false;
            };
            if !matches!(cx.ty(e).strip_refs(), Ty::Result(ok, _) if ok.is_str()) {
                return false;
            }
            let Some(ip) = local_ident(cx, local) else {
                return false;
            };
            match ip.syntax().parent() {
                Some(p) if ast::ForExpr::can_cast(p.kind()) => ast::ForExpr::cast(p)
                    .and_then(|f| f.iterable())
                    .is_some_and(|it| plain_lines_iterable(cx, &it)),
                Some(p) if ast::Param::can_cast(p.kind()) => p
                    .ancestors()
                    .find_map(ast::ClosureExpr::cast)
                    .and_then(|c| c.syntax().parent())
                    .and_then(ast::MethodCallExpr::cast)
                    .is_some_and(|call| {
                        matches!(
                            method_name(&call).as_str(),
                            "map" | "inspect" | "filter_map"
                        ) && call
                            .receiver()
                            .is_some_and(|r| plain_lines_iterable(cx, &r))
                    }),
                Some(p) if ast::LetStmt::can_cast(p.kind()) => local_is_plain_iterator(cx, local),
                _ => false,
            }
        }
        ast::Expr::MethodCallExpr(m) if matches!(method_name(&m).as_str(), "unwrap" | "expect") => {
            m.receiver().is_some_and(|r| phantom_line_result(cx, &r))
        }
        ast::Expr::MethodCallExpr(m) if method_name(&m) == "next" => m
            .receiver()
            .and_then(|r| cx.local_of_path(&r))
            .is_some_and(|local| local_is_plain_iterator(cx, local)),
        _ => false,
    }
}

// `let line = line.unwrap();` on such a loop: the binding already names the loop variable.
pub(crate) fn phantom_line_let(cx: &Cx, init: &ast::Expr) -> Option<String> {
    let recv = match strip_parens(init) {
        ast::Expr::MethodCallExpr(m) if matches!(method_name(&m).as_str(), "unwrap" | "expect") => {
            m.receiver()?
        }
        ast::Expr::TryExpr(t) => t.expr()?,
        _ => return None,
    };
    if !phantom_line_result(cx, &recv) {
        return None;
    }
    let local = cx.local_of_path(&recv)?;
    let ip = local_ident(cx, local)?;
    if !ip
        .syntax()
        .parent()
        .is_some_and(|p| ast::ForExpr::can_cast(p.kind()))
    {
        return None;
    }
    Some(cx.lookup(&ip.name()?.text().to_string()))
}

// Byte-level reads need stdin as an InputStream; otherwise it can be a line reader.
pub(crate) fn has_byte_reads(cx: &Cx, root: &ra_ap_syntax::SyntaxNode) -> bool {
    cx.expanded_descendants(root)
        .into_iter()
        .filter_map(ast::MethodCallExpr::cast)
        .any(|m| {
            matches!(
                method_name(&m).as_str(),
                "read" | "bytes" | "read_to_end" | "read_exact" | "read_until"
            ) && m.receiver().is_some_and(|r| is_reader_source(&cx.ty(&r)))
        })
}

// A reader over stdin in line mode (`stdin()`, its lock, a BufReader around either), which is the
// BufferedReader `STDIN`; files stay byte streams.
pub(crate) fn is_line_stdin(cx: &Cx, e: &ast::Expr) -> bool {
    cx.stdin_lines
        && cx.sema.type_of_expr(e).is_some_and(|t| {
            use ra_ap_hir::HirDisplay;
            let shown = t
                .original
                .display(cx.db, cx.module.krate(cx.db).to_display_target(cx.db))
                .to_string();
            shown.contains("Stdin")
        })
}

pub(crate) fn is_reader_source(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(s) if s.contains("Stdin") || s.contains("BufReader"))
}
