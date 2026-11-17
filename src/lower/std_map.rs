// Std library calls, methods, paths and constructors -> Java standard library.

use ra_ap_hir::ModuleDef;
use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

use super::{Cx, expr};
use crate::emit::render_expr as render;
use crate::jir::{Expr, LambdaBody, Stmt, StmtKind};
use crate::ty::{Int, Ty};

// ---- small helpers ---------------------------------------------------------------------------

fn arg_asts(c: &impl HasArgList) -> Vec<ast::Expr> {
    c.arg_list().map(|al| al.args().collect()).unwrap_or_default()
}

fn method_name(m: &ast::MethodCallExpr) -> String {
    m.name_ref().map(|n| n.text().to_string()).unwrap_or_default()
}

fn opt_of(v: Expr) -> Expr {
    Expr::call(Expr::raw("Optional"), "of", vec![v])
}

fn opt_empty(inner: Option<&Ty>) -> Expr {
    match inner {
        Some(t) if t.is_nameable() => Expr::raw(format!("Optional.<{}>empty()", t.boxed())),
        _ => Expr::raw("Optional.empty()"),
    }
}

fn opt_inner(ty: &Ty) -> Option<Ty> {
    match ty.strip_refs() {
        Ty::Option(t) => Some((**t).clone()),
        _ => None,
    }
}

fn optional(cond: Expr, value: Expr, ret: &Ty) -> Expr {
    let inner = opt_inner(ret);
    Expr::Ternary(Box::new(cond), Box::new(opt_of(value)), Box::new(opt_empty(inner.as_ref())))
}

// Java promotes byte/short arithmetic to int; cast back to the Rust type.
fn narrow(v: Expr, i: Int) -> Expr {
    match i {
        Int::I8 | Int::I16 | Int::U16 => Expr::Cast(i.java().into(), Box::new(v)),
        _ => v,
    }
}

fn push_local(cx: &mut Cx, line: u32, ty: String, init: Option<Expr>, out: &mut Vec<Stmt>, base: &str) -> String {
    let name = cx.fresh(base);
    out.push(Stmt::new(line, StmtKind::Local { ty, name: name.clone(), init }));
    name
}

fn push_raw(out: &mut Vec<Stmt>, line: u32, text: String) {
    out.push(Stmt::new(line, StmtKind::Raw(text)));
}

// Re-evaluating `e` must be safe and cheap; otherwise bind it to a temporary on this line.
fn simple(cx: &mut Cx, e: Expr, ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let simple = match &e {
        Expr::Name(_) => true,
        Expr::Raw(s) => s.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']' | '"' | ' ')),
        _ => false,
    };
    if simple || cx.lambda_depth > 0 {
        return e;
    }
    let name = push_local(cx, line, super::stmt::decl_type(ty), Some(e), out, "tmp");
    Expr::name(name)
}

fn consumed_by(cx: &Cx, e: &ast::Expr) -> Option<String> {
    let parent = e.syntax().parent()?;
    let mc = ast::MethodCallExpr::cast(parent)?;
    let is_recv = mc.receiver().is_some_and(|r| r.syntax().text_range() == e.syntax().text_range());
    if is_recv { mc.name_ref().map(|n| n.text().to_string()) } else { let _ = cx; None }
}

fn discarded(m: &ast::MethodCallExpr) -> bool {
    m.syntax().parent().is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
}

fn result_java(ret: &Ty) -> String {
    match ret.strip_refs() {
        Ty::Result(t, e) => format!("Result<{}, {}>", t.boxed(), e.boxed()),
        _ => "Result<Object, Object>".into(),
    }
}

fn wrap_ok(cx: &mut Cx, line: u32, ret: &Ty, value: Expr, out: &mut Vec<Stmt>) -> Expr {
    cx.needs_result = true;
    let ty = result_java(ret);
    let name = push_local(cx, line, ty, Some(Expr::New { ty: "Ok<>".into(), args: vec![value] }), out, "res");
    Expr::name(name)
}

fn wrap_try_ok(cx: &mut Cx, line: u32, ret: &Ty, value: Expr, catch: &str, out: &mut Vec<Stmt>) -> Expr {
    cx.needs_result = true;
    let ty = result_java(ret);
    let name = cx.fresh("res");
    let ex = cx.fresh("ex");
    push_raw(
        out,
        line,
        format!(
            "{ty} {name}; try {{ {name} = new Ok<>({}); }} catch ({catch} {ex}) {{ {name} = new Err<>({ex}); }}",
            render(&value)
        ),
    );
    Expr::name(name)
}

fn clone_expr(e: Expr, ty: &Ty) -> Expr {
    let new = |t: &str, e: Expr| Expr::New { ty: t.into(), args: vec![e] };
    match ty.strip_refs() {
        Ty::Vec(_) => new("ArrayList<>", e),
        Ty::Set { ordered, .. } => new(if *ordered { "TreeSet<>" } else { "HashSet<>" }, e),
        Ty::Map { ordered, .. } => new(if *ordered { "TreeMap<>" } else { "HashMap<>" }, e),
        Ty::Deque(_) => new("ArrayDeque<>", e),
        Ty::Heap(_) => new("PriorityQueue<>", e),
        Ty::Adt { .. } => Expr::call(e, "clone", vec![]),
        _ => e,
    }
}

// Zero value for `Default::default` / `unwrap_or_default`.
fn default_value(ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Bool => Expr::raw("false"),
        Ty::Char => Expr::raw("'\\0'"),
        Ty::Int(i) => Expr::raw(if i.is_long() { "0L" } else { "0" }),
        Ty::F32 => Expr::raw("0.0f"),
        Ty::F64 => Expr::raw("0.0"),
        Ty::Str => Expr::str_lit(""),
        Ty::Vec(_) => Expr::New { ty: "ArrayList<>".into(), args: vec![] },
        Ty::Map { ordered, .. } => {
            Expr::New { ty: if *ordered { "TreeMap<>" } else { "HashMap<>" }.into(), args: vec![] }
        }
        Ty::Set { ordered, .. } => Expr::New { ty: if *ordered { "TreeSet<>" } else { "HashSet<>" }.into(), args: vec![] },
        Ty::Deque(_) => Expr::New { ty: "ArrayDeque<>".into(), args: vec![] },
        Ty::Heap(_) => Expr::New { ty: "PriorityQueue<>".into(), args: vec![] },
        Ty::Option(_) => Expr::raw("Optional.empty()"),
        Ty::Result(..) => Expr::raw("null"),
        _ => Expr::raw("null"),
    }
}

// Declares (once) the shared stdin reader on the file's last line.
pub fn need_stdin(cx: &mut Cx) {
    let decl = "static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));";
    if !cx.file_trailer.iter().any(|t| t.contains("STDIN")) {
        cx.file_trailer.push(decl.into());
    }
}

// ---- std free functions ----------------------------------------------------------------------

pub fn lower_std_fn_call(cx: &mut Cx, c: &ast::CallExpr, key: &str, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let ret = cx.ty(&ast::Expr::CallExpr(c.clone()));
    match key {
        "mem::swap" => return mem_swap(cx, c, out),
        "mem::replace" => return mem_replace(cx, c, &ret, false, out),
        "mem::take" => return mem_replace(cx, c, &ret, true, out),
        "mem::drop" | "mem::forget" => {
            let mut args = expr::call_args(cx, c, out);
            if let Some(v) = args.pop()
                && v.is_statement_expr()
            {
                out.push(Stmt::new(line, StmtKind::Expr(v)));
            }
            return Expr::raw("null");
        }
        "fs::read_to_string" => {
            let mut args = expr::call_args(cx, c, out);
            return fs_read(cx, c, &ret, args.remove(0), out);
        }
        "fs::write" => {
            let mut args = expr::call_args(cx, c, out);
            let contents = args.pop().unwrap_or_else(|| Expr::raw("null"));
            let path = args.pop().unwrap_or_else(|| Expr::raw("null"));
            return fs_write(cx, c, &ret, path, contents, out);
        }
        "io::stdin" | "stdio::stdin" => {
            need_stdin(cx);
            return Expr::raw("STDIN");
        }
        "io::stdout" | "stdio::stdout" => return Expr::raw("System.out"),
        "env::args" | "env::args_os" => {
            return Expr::raw("Stream.concat(Stream.of(\"main\"), Arrays.stream(ARGS))");
        }
        "env::var" => {
            let args = expr::call_args(cx, c, out);
            let key = args.first().cloned().unwrap_or_else(|| Expr::str_lit(""));
            cx.needs_result = true;
            let ty = result_java(&ret);
            let name = cx.fresh("val");
            let res = cx.fresh("env");
            push_raw(
                out,
                line,
                format!(
                    "String {name} = System.getenv({}); {ty} {res}; if ({name} != null) {res} = new Ok<>({name}); else {res} = new Err<>(\"environment variable not found\");",
                    render(&key)
                ),
            );
            return Expr::name(res);
        }
        "env::var_os" => {
            let args = expr::call_args(cx, c, out);
            let key = args.first().cloned().unwrap_or_else(|| Expr::str_lit(""));
            let call = Expr::call(Expr::raw("System"), "getenv", vec![key]);
            return Expr::call(Expr::raw("Optional"), "ofNullable", vec![call]);
        }
        "Instant::now" => return Expr::raw("System.nanoTime()"),
        "Duration::from_secs" => {
            let args = expr::call_args(cx, c, out);
            return Expr::call(Expr::raw("Duration"), "ofSeconds", args);
        }
        "Duration::from_millis" => {
            let args = expr::call_args(cx, c, out);
            return Expr::call(Expr::raw("Duration"), "ofMillis", args);
        }
        "Duration::from_nanos" => {
            let args = expr::call_args(cx, c, out);
            return Expr::call(Expr::raw("Duration"), "ofNanos", args);
        }
        "process::id" => return Expr::raw("(int) ProcessHandle.current().pid()"),
        _ => {}
    }
    let args = expr::call_args(cx, c, out);
    let arg_asts = arg_asts(c);
    match key {
        "String::new" | "String::with_capacity" => Expr::str_lit(""),
        "String::from" | "ToString::to_string" | "ToOwned::to_owned" => args.into_iter().next().unwrap_or_else(|| Expr::str_lit("")),
        "String::from_utf8" | "String::from_utf8_lossy" | "String::from_utf8_unchecked" => {
            let src = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
            let b = cx.fresh("b");
            let joined = Expr::raw(format!(
                "{}stream().map({b} -> String.valueOf((char) (int) {b})).collect(Collectors.joining())",
                render(&src)
            ));
            if key == "String::from_utf8" && consumed_by(cx, &ast::Expr::CallExpr(c.clone())).as_deref().is_none_or(|u| !matches!(u, "unwrap" | "expect")) {
                return wrap_ok(cx, line, &ret, joined, out);
            }
            joined
        }
        "Vec::new" | "Vec::with_capacity" => Expr::New { ty: "ArrayList<>".into(), args: vec![] },
        "Vec::from" => Expr::New { ty: "ArrayList<>".into(), args: vec![args[0].clone()] },
        "Vec::from_iter" | "Vec::collect" => {
            Expr::call(args[0].clone(), "collect", vec![Expr::raw("Collectors.toCollection(ArrayList::new)")])
        }
        "HashMap::new" | "HashMap::with_capacity" => Expr::New { ty: "HashMap<>".into(), args: vec![] },
        "BTreeMap::new" => Expr::New { ty: "TreeMap<>".into(), args: vec![] },
        "HashSet::new" | "HashSet::with_capacity" => Expr::New { ty: "HashSet<>".into(), args: vec![] },
        "BTreeSet::new" => Expr::New { ty: "TreeSet<>".into(), args: vec![] },
        "VecDeque::new" | "VecDeque::with_capacity" => Expr::New { ty: "ArrayDeque<>".into(), args: vec![] },
        "BinaryHeap::new" => {
            if heap_is_reverse(c) {
                Expr::New { ty: "PriorityQueue<>".into(), args: vec![] }
            } else {
                Expr::New { ty: "PriorityQueue<>".into(), args: vec![Expr::raw("Comparator.reverseOrder()")] }
            }
        }
        "BinaryHeap::from" => {
            let src = args[0].clone();
            if heap_is_reverse(c) {
                Expr::New { ty: "PriorityQueue<>".into(), args: vec![src] }
            } else {
                let name = push_local(
                    cx,
                    line,
                    "PriorityQueue<>".into(),
                    Some(Expr::New { ty: "PriorityQueue<>".into(), args: vec![Expr::raw("Comparator.reverseOrder()")] }),
                    out,
                    "heap",
                );
                out.push(Stmt::new(line, StmtKind::Expr(Expr::call(Expr::name(name.clone()), "addAll", vec![src]))));
                Expr::name(name)
            }
        }
        "Default::default" => default_value(&ret),
        "From::from" | "Into::into" => match ret.strip_refs() {
            Ty::Vec(_) | Ty::Set { .. } | Ty::Map { .. } | Ty::Deque(_) | Ty::Heap(_) => clone_expr(args[0].clone(), &ret),
            _ => args.into_iter().next().unwrap_or_else(|| Expr::raw("null")),
        },
        "Clone::clone" => {
            let a = args.into_iter().next().unwrap_or_else(|| Expr::raw("null"));
            clone_expr(a, &ret)
        }
        "Box::new" | "Rc::new" | "Arc::new" | "RefCell::new" | "Cell::new" | "Pin::new" => {
            args.into_iter().next().unwrap_or_else(|| Expr::raw("null"))
        }
        "cmp::min" | "cmp::max" => cmp_min_max(cx, key == "cmp::max", &arg_asts, args, line, out),
        "char::from_u32" => {
            let n = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
            let cond = Expr::bin(
                Expr::bin(n.clone(), ">=", Expr::raw("0")),
                "&&",
                Expr::bin(n.clone(), "<=", Expr::raw("0xFFFF")),
            );
            optional(cond, Expr::Cast("char".into(), Box::new(n)), &ret)
        }
        "char::from_u32_unchecked" => Expr::Cast("char".into(), Box::new(args.first().cloned().unwrap_or_else(|| Expr::raw("0")))),
        "char::from_digit" => {
            let d = simple(cx, args.first().cloned().unwrap_or_else(|| Expr::raw("0")), &Ty::Int(Int::U32), line, out);
            let r = simple(cx, args.get(1).cloned().unwrap_or_else(|| Expr::raw("10")), &Ty::Int(Int::U32), line, out);
            let (ds, rs) = (render(&d), render(&r));
            Expr::raw(format!(
                "(Character.forDigit({ds}, {rs}) == '\\0' ? Optional.<Character>empty() : Optional.of(Character.forDigit({ds}, {rs})))"
            ))
        }
        "process::exit" => Expr::call(Expr::raw("System"), "exit", args),
        "thread::sleep" => {
            let ms = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
            let ex = cx.fresh("ex");
            push_raw(
                out,
                line,
                format!("try {{ Thread.sleep({}); }} catch (InterruptedException {ex}) {{ throw new RuntimeException({ex}); }}", render(&ms)),
            );
            Expr::raw("null")
        }
        k if k.contains("::from_str_radix") => from_str_radix(cx, c, k, &ret, args, out),
        _ => cx.unsupported(c.syntax(), &format!("std function `{key}`")),
    }
}

fn heap_is_reverse(c: &ast::CallExpr) -> bool {
    if c.syntax().text().to_string().contains("Reverse") {
        return true;
    }
    c.syntax()
        .ancestors()
        .find_map(ast::LetStmt::cast)
        .and_then(|l| l.ty())
        .is_some_and(|t| t.syntax().text().to_string().contains("Reverse"))
}

fn cmp_min_max(cx: &mut Cx, max: bool, arg_asts: &[ast::Expr], mut args: Vec<Expr>, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let (a, b) = (args.remove(0), args.remove(0));
    let ty = arg_asts.first().map(|e| cx.ty(e)).unwrap_or_else(|| Ty::Int(Int::I32));
    let method = if max { "max" } else { "min" };
    match ty.strip_refs() {
        Ty::Int(_) | Ty::F32 | Ty::F64 => Expr::call(Expr::raw("Math"), method, vec![a, b]),
        Ty::Bool => {
            let op = if max { "||" } else { "&&" };
            Expr::bin(a, op, b)
        }
        Ty::Char => {
            let (a, b) = (simple(cx, a, &ty, line, out), simple(cx, b, &ty, line, out));
            Expr::Ternary(Box::new(Expr::bin(a.clone(), if max { ">=" } else { "<=" }, b.clone())), Box::new(a), Box::new(b))
        }
        _ => {
            let (a, b) = (simple(cx, a, &ty, line, out), simple(cx, b, &ty, line, out));
            let cond = Expr::bin(Expr::call(a.clone(), "compareTo", vec![b.clone()]), if max { ">=" } else { "<=" }, Expr::raw("0"));
            Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
        }
    }
}

fn from_str_radix(cx: &mut Cx, c: &ast::CallExpr, key: &str, ret: &Ty, args: Vec<Expr>, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let owner = key.split("::").next().unwrap_or("i32");
    let s = args.first().cloned().unwrap_or_else(|| Expr::str_lit(""));
    let radix = args.get(1).cloned().unwrap_or_else(|| Expr::raw("10"));
    let long = owner.starts_with('i') && matches!(owner, "i64" | "i128") || matches!(owner, "u64" | "u128");
    let call = if long {
        Expr::call(Expr::raw("Long"), "parseLong", vec![s, radix])
    } else {
        Expr::call(Expr::raw("Integer"), "parseInt", vec![s, radix])
    };
    if matches!(consumed_by(cx, &ast::Expr::CallExpr(c.clone())).as_deref(), Some("unwrap" | "expect")) {
        return call;
    }
    wrap_try_ok(cx, line, ret, call, "NumberFormatException", out)
}

// ---- std paths (consts and unit variants) ----------------------------------------------------

pub fn lower_std_path(cx: &mut Cx, path: &ast::Path, def: ModuleDef) -> Expr {
    let _ = def;
    let text = path.syntax().text().to_string();
    let parts: Vec<&str> = text.split("::").collect();
    let last = parts.last().copied().unwrap_or("");
    let ty_tok = parts
        .iter()
        .rev()
        .find(|p| {
            matches!(**p, "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "f32" | "f64" | "char")
        })
        .copied();
    match last {
        "None" => return Expr::raw("Optional.empty()"),
        "Less" if text.contains("Ordering") => return Expr::raw("-1"),
        "Equal" if text.contains("Ordering") => return Expr::raw("0"),
        "Greater" if text.contains("Ordering") => return Expr::raw("1"),
        "MAX" | "MIN" | "BITS" => {
            if let Some(e) = int_const(ty_tok, last) {
                return Expr::raw(e);
            }
        }
        "PI" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.PI" } else { "Math.PI" }),
        "E" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.E" } else { "Math.E" }),
        "TAU" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.TAU" } else { "Math.TAU" }),
        "SQRT_2" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.sqrt(2)" } else { "Math.sqrt(2)" }),
        "LN_2" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.log(2)" } else { "Math.log(2)" }),
        "LN_10" => return Expr::raw(if ty_tok == Some("f32") { "(float) Math.log(10)" } else { "Math.log(10)" }),
        "EPSILON" => {
            return Expr::raw(if ty_tok == Some("f32") { "Math.ulp(1.0f)" } else { "Math.ulp(1.0)" });
        }
        "INFINITY" => return Expr::raw(if ty_tok == Some("f32") { "Float.POSITIVE_INFINITY" } else { "Double.POSITIVE_INFINITY" }),
        "NEG_INFINITY" => return Expr::raw(if ty_tok == Some("f32") { "Float.NEGATIVE_INFINITY" } else { "Double.NEGATIVE_INFINITY" }),
        "NAN" => return Expr::raw(if ty_tok == Some("f32") { "Float.NaN" } else { "Double.NaN" }),
        "MIN_POSITIVE" => return Expr::raw(if ty_tok == Some("f32") { "Float.MIN_NORMAL" } else { "Double.MIN_NORMAL" }),
        _ => {}
    }
    cx.unsupported(path.syntax(), &format!("std path `{text}`"))
}

fn int_const(ty: Option<&str>, which: &str) -> Option<&'static str> {
    let max = which == "MAX";
    if which == "BITS" {
        return None;
    }
    Some(match (ty?, max) {
        ("i8", true) => "Byte.MAX_VALUE",
        ("i8", false) => "Byte.MIN_VALUE",
        ("i16", true) => "Short.MAX_VALUE",
        ("i16", false) => "Short.MIN_VALUE",
        ("i32" | "isize", true) => "Integer.MAX_VALUE",
        ("i32" | "isize", false) => "Integer.MIN_VALUE",
        ("i64" | "i128", true) => "Long.MAX_VALUE",
        ("i64" | "i128", false) => "Long.MIN_VALUE",
        ("u8", true) => "255",
        ("u8", false) => "0",
        ("u16", true) => "65535",
        ("u16", false) => "0",
        ("u32", true) => "(int) 4294967295L",
        ("u32", false) => "0",
        ("u64" | "u128", true) => "-1L",
        ("u64" | "u128", false) => "0L",
        ("usize", true) => "Integer.MAX_VALUE",
        ("usize", false) => "0",
        ("f32", true) => "Float.MAX_VALUE",
        ("f32", false) => "-Float.MAX_VALUE",
        ("f64", true) => "Double.MAX_VALUE",
        ("f64", false) => "-Double.MAX_VALUE",
        ("char", true) => "'\\uFFFF'",
        ("char", false) => "'\\0'",
        _ => return None,
    })
}

// ---- std constructors -----------------------------------------------------------------------

pub fn lower_std_ctor(cx: &mut Cx, c: &ast::CallExpr, path: &ast::Path, args: &[Expr]) -> Option<Expr> {
    let last = path.segment()?.name_ref()?.text().to_string();
    let _ = c;
    match last.as_str() {
        "Some" => Some(opt_of(args.first().cloned().unwrap_or_else(|| Expr::raw("null")))),
        "Ok" => {
            cx.needs_result = true;
            Some(Expr::New { ty: "Ok<>".into(), args: args.to_vec() })
        }
        "Err" => {
            cx.needs_result = true;
            Some(Expr::New { ty: "Err<>".into(), args: args.to_vec() })
        }
        // Newtypes are erased.
        "Reverse" | "Wrapping" | "Saturating" => args.first().cloned(),
        _ => None,
    }
}

// ---- mem::{swap, replace, take} --------------------------------------------------------------

fn ref_inner(e: &ast::Expr) -> ast::Expr {
    match e {
        ast::Expr::RefExpr(r) => r.expr().unwrap_or_else(|| e.clone()),
        _ => e.clone(),
    }
}

fn mem_swap(cx: &mut Cx, c: &ast::CallExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let asts = arg_asts(c);
    let (Some(a_ast), Some(b_ast)) = (asts.first(), asts.get(1)) else {
        return cx.unsupported(c.syntax(), "mem::swap without two arguments");
    };
    let (a_place, b_place) = (ref_inner(a_ast), ref_inner(b_ast));
    let a_val = expr::lower_expr(cx, &a_place, out);
    let b_val = expr::lower_expr(cx, &b_place, out);
    let ty = cx.ty(&a_place);
    let tmp = push_local(cx, line, super::stmt::decl_type(&ty), Some(a_val), out, "swap");
    let to_b = expr::place_assign(cx, &a_place, "=", b_val, out);
    out.push(Stmt::new(line, StmtKind::Expr(to_b)));
    let to_a = expr::place_assign(cx, &b_place, "=", Expr::name(tmp), out);
    out.push(Stmt::new(line, StmtKind::Expr(to_a)));
    Expr::raw("null")
}

fn mem_replace(cx: &mut Cx, c: &ast::CallExpr, ret: &Ty, take: bool, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let asts = arg_asts(c);
    let Some(a_ast) = asts.first() else { return cx.unsupported(c.syntax(), "mem::replace without argument") };
    let place_ast = ref_inner(a_ast);
    let ty = cx.ty(&place_ast);
    let old = expr::lower_expr(cx, &place_ast, out);
    let new = if take { default_value(&ty) } else { expr::lower_expr(cx, asts.get(1).unwrap_or(a_ast), out) };
    let tmp = push_local(cx, line, super::stmt::decl_type(&ty), Some(old), out, "old");
    let assign = expr::place_assign(cx, &place_ast, "=", new, out);
    out.push(Stmt::new(line, StmtKind::Expr(assign)));
    let _ = ret;
    Expr::name(tmp)
}

// ---- std methods ----------------------------------------------------------------------------

pub fn lower_std_method(cx: &mut Cx, m: &ast::MethodCallExpr, recv_ty: &Ty, key: &str, out: &mut Vec<Stmt>) -> Expr {
    let recv_ast = m.receiver().unwrap();
    let name = key.rsplit("::").next().unwrap_or(key).to_string();
    let line = cx.line(m.syntax());
    // `m.entry(k).or_insert*` must be handled before the `entry()` receiver is lowered.
    if matches!(name.as_str(), "or_insert" | "or_insert_with" | "or_default") && is_entry_recv(&recv_ast) {
        let args = if name == "or_insert" {
            match m.arg_list() {
                Some(al) => expr::lower_args(cx, al.args(), out),
                None => vec![],
            }
        } else {
            vec![]
        };
        if let Some(e) = entry_method(cx, m, &name, &args, out) {
            return e;
        }
    }
    let recv = expr::lower_expr(cx, &recv_ast, out);
    let arg_asts = arg_asts(m);
    let args = match m.arg_list() {
        Some(al) => expr::lower_args(cx, al.args(), out),
        None => vec![],
    };
    let ret = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));

    // Stdin / stdout chains don't look like ordinary methods' receivers.
    if is_stdin(cx, &recv_ast) {
        if let Some(e) = stdio_method(cx, m, &name, &recv_ast, &ret, line, out) {
            return e;
        }
    }
    if name == "flush" && is_stdout(cx, &recv_ast) {
        return Expr::raw("System.out.flush()");
    }
    if let Some(e) = entry_method(cx, m, &name, &args, out) {
        return e;
    }
    if name == "elapsed" && key.starts_with("Instant::") {
        return Expr::call(Expr::raw("Duration"), "ofNanos", vec![Expr::bin(Expr::raw("System.nanoTime()"), "-", recv)]);
    }
    if let Some(e) = duration_method(cx, &name, recv.clone(), &args) {
        return e;
    }

    let t = recv_ty.strip_refs().clone();
    let found = match &t {
        Ty::Int(i) => lower_int_method(cx, m, recv.clone(), args.clone(), &arg_asts, *i, &name, &ret, line, out),
        Ty::F32 | Ty::F64 => lower_float_method(cx, m, recv.clone(), args.clone(), t == Ty::F32, &name, &ret, line, out),
        Ty::Bool | Ty::Char => lower_small_method(cx, recv.clone(), &args, &t, &name, &ret, line, out),
        Ty::Str => lower_str_method(cx, m, &recv_ast, recv.clone(), args.clone(), &arg_asts, &name, &ret, line, out),
        Ty::Option(_) => lower_option_method(cx, m, &recv_ast, recv.clone(), args.clone(), &name, &ret, line, out),
        Ty::Result(..) => lower_result_method(cx, m, &recv_ast, recv.clone(), args.clone(), &name, &ret, line, out),
        Ty::Vec(elem) => lower_vec_method(cx, m, &recv_ast, recv.clone(), elem, args.clone(), &arg_asts, &name, &ret, line, out),
        Ty::Map { ordered, k, v } => {
            lower_map_method(cx, m, &recv_ast, recv.clone(), *ordered, k, v, args.clone(), &arg_asts, &name, &ret, line, out)
        }
        Ty::Set { ordered, t: elem } => {
            lower_set_method(cx, m, &recv_ast, recv.clone(), *ordered, elem, args.clone(), &arg_asts, &name, line, out)
        }
        Ty::Deque(elem) => lower_deque_method(cx, m, &recv_ast, recv.clone(), elem, args.clone(), &name, &ret, line, out),
        Ty::Heap(_) => lower_heap_method(cx, m, &recv_ast, recv.clone(), args.clone(), &name, &ret, line, out),
        Ty::Ordering => lower_ordering_method(cx, recv.clone(), args.clone(), &name),
        _ => None,
    };
    if let Some(e) = found {
        return e;
    }

    // Generic fallbacks.
    if name == "clone" {
        return clone_expr(recv, recv_ty);
    }
    if name == "into" || name == "from" {
        return clone_expr(recv, &ret);
    }
    match (recv_ty.strip_refs(), name.as_str()) {
        (t, "to_string") if t.is_str() => recv,
        (t, "to_string") if t.is_primitive() => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        (Ty::Adt { .. }, "to_string") => Expr::call(recv, "toString", vec![]),
        (Ty::Option(_) | Ty::Result(..), "to_string") => Expr::call(recv, "toString", vec![]),
        _ => cx.unsupported(m.syntax(), &format!("std method `{key}` on {recv_ty:?}")),
    }
}

// ---- stdin / stdout -------------------------------------------------------------------------

fn is_stdin(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = cx;
    match e {
        ast::Expr::CallExpr(c) => {
            let text = c.expr().map(|c| c.syntax().text().to_string()).unwrap_or_default();
            text.ends_with("stdin")
        }
        ast::Expr::MethodCallExpr(mc) => match method_name(mc).as_str() {
            "lock" | "by_ref" => mc.receiver().is_some_and(|r| is_stdin(cx, &r)),
            _ => false,
        },
        _ => false,
    }
}

fn is_stdout(cx: &Cx, e: &ast::Expr) -> bool {
    let _ = cx;
    match e {
        ast::Expr::CallExpr(c) => c.expr().map(|c| c.syntax().text().to_string().ends_with("stdout")).unwrap_or(false),
        _ => false,
    }
}

fn stdio_method(cx: &mut Cx, m: &ast::MethodCallExpr, name: &str, recv_ast: &ast::Expr, ret: &Ty, line: u32, out: &mut Vec<Stmt>) -> Option<Expr> {
    let _ = (recv_ast, ret);
    match name {
        "read_line" | "read_to_string" => {
            let arg = arg_asts(m).into_iter().next()?;
            let place_ast = ref_inner(&arg);
            need_stdin(cx);
            let place = expr::lower_expr(cx, &place_ast, out);
            let ps = render(&place);
            let l = cx.fresh("line");
            let used = matches!(consumed_by(cx, &ast::Expr::MethodCallExpr(m.clone())).as_deref(), Some("unwrap" | "expect"));
            if used {
                if name == "read_line" {
                    let tmp = push_local(cx, line, "String".into(), Some(Expr::raw("STDIN.lines().findFirst().orElse(null)")), out, "line");
                    let n = push_local(cx, line, "int".into(), Some(Expr::raw("0")), out, "n");
                    push_raw(out, line, format!("if ({tmp} != null) {{ {ps} += {tmp} + \"\\n\"; {n} = {tmp}.length() + 1; }}"));
                    Some(Expr::name(n))
                } else {
                    let tmp = push_local(
                        cx,
                        line,
                        "String".into(),
                        Some(Expr::raw("STDIN.lines().map(l -> l + \"\\n\").collect(Collectors.joining())")),
                        out,
                        "rest",
                    );
                    push_raw(out, line, format!("{ps} += {tmp};"));
                    let n = push_local(cx, line, "int".into(), Some(Expr::call(Expr::name(tmp), "length", vec![])), out, "n");
                    Some(Expr::name(n))
                }
            } else {
                let app = if name == "read_line" {
                    Expr::raw(format!("STDIN.lines().findFirst().map({l} -> {l} + \"\\n\").orElse(\"\")"))
                } else {
                    Expr::raw(format!("STDIN.lines().map({l} -> {l} + \"\\n\").collect(Collectors.joining())"))
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

fn duration_method(cx: &mut Cx, name: &str, recv: Expr, args: &[Expr]) -> Option<Expr> {
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

fn is_entry_recv(e: &ast::Expr) -> bool {
    matches!(e, ast::Expr::MethodCallExpr(mc) if method_name(mc) == "entry")
}

// `m.entry(k)` chains: or_insert / or_insert_with / or_default -> computeIfAbsent.
fn entry_method(cx: &mut Cx, m: &ast::MethodCallExpr, name: &str, args: &[Expr], out: &mut Vec<Stmt>) -> Option<Expr> {
    if !matches!(name, "or_insert" | "or_insert_with" | "or_default") {
        return None;
    }
    let recv_ast = m.receiver()?;
    let ast::Expr::MethodCallExpr(entry) = &recv_ast else { return None };
    if method_name(entry) != "entry" {
        return None;
    }
    let map_ast = entry.receiver()?;
    let key = entry.arg_list()?.args().next()?;
    let map = expr::lower_expr(cx, &map_ast, out);
    let key = expr::lower_expr(cx, &key, out);
    let map_ty = cx.ty(&map_ast);
    let v_ty = match map_ty.strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        _ => Ty::Other("?".into()),
    };
    let value = match name {
        "or_insert" => args.first().cloned().unwrap_or_else(|| Expr::raw("null")),
        "or_insert_with" => {
            let a = m.arg_list()?.args().next()?;
            ctor_value(cx, &a, &v_ty, out)?
        }
        _ => default_value(&v_ty),
    };
    let ignored = cx.fresh("ignored");
    let f = Expr::Lambda { params: vec![ignored], body: LambdaBody::Expr(Box::new(value)) };
    Some(Expr::call(map, "computeIfAbsent", vec![key, f]))
}

// Value produced by a `FnOnce() -> T` argument such as `Vec::new` or `|| expr`.
fn ctor_value(cx: &mut Cx, a: &ast::Expr, ty: &Ty, out: &mut Vec<Stmt>) -> Option<Expr> {
    if let ast::Expr::ClosureExpr(c) = a {
        if c.param_list().is_some_and(|pl| pl.params().next().is_some()) {
            return None;
        }
        let body = c.body()?;
        return Some(expr::lower_expr(cx, &body, out));
    }
    let text = a.syntax().text().to_string();
    let last = text.rsplit("::").next().unwrap_or(&text);
    Some(match last {
        "new" if text.contains("Vec") => Expr::New { ty: "ArrayList<>".into(), args: vec![] },
        "new" if text.contains("String") => Expr::str_lit(""),
        "new" if text.contains("HashMap") => Expr::New { ty: "HashMap<>".into(), args: vec![] },
        "new" if text.contains("HashSet") => Expr::New { ty: "HashSet<>".into(), args: vec![] },
        "default" => default_value(ty),
        _ => return None,
    })
}

// `*m.entry(k).or_insert(v) op= r` -> put / merge. Called from expr::lower_assign.
pub fn lower_entry_assign(cx: &mut Cx, lhs: &ast::Expr, op: Option<ast::ArithOp>, rhs: &ast::Expr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let ast::Expr::PrefixExpr(p) = lhs else { return None };
    if p.op_kind() != Some(ast::UnaryOp::Deref) {
        return None;
    }
    let inner = p.expr()?;
    let ast::Expr::MethodCallExpr(mc) = &inner else { return None };
    if !matches!(method_name(mc).as_str(), "or_insert" | "or_insert_with" | "or_default") {
        return None;
    }
    let ast::Expr::MethodCallExpr(entry) = mc.receiver()? else { return None };
    if method_name(&entry) != "entry" {
        return None;
    }
    let map_ast = entry.receiver()?;
    let key_ast = entry.arg_list()?.args().next()?;
    let map_ty = cx.ty(&map_ast);
    let v_ty = match map_ty.strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        _ => Ty::Other("?".into()),
    };
    let map = expr::lower_expr(cx, &map_ast, out);
    let key = expr::lower_expr(cx, &key_ast, out);
    let r = expr::lower_expr(cx, rhs, out);
    match op {
        None => Some(Expr::call(map, "put", vec![key, r])),
        Some(a) => {
            let (x, y) = (cx.fresh("acc"), cx.fresh("delta"));
            let body = match a {
                ast::ArithOp::Add => Expr::bin(Expr::name(x.clone()), "+", Expr::name(y.clone())),
                ast::ArithOp::Sub => Expr::bin(Expr::name(x.clone()), "-", Expr::name(y.clone())),
                ast::ArithOp::Mul => Expr::bin(Expr::name(x.clone()), "*", Expr::name(y.clone())),
                ast::ArithOp::Div => Expr::bin(Expr::name(x.clone()), "/", Expr::name(y.clone())),
                ast::ArithOp::Rem => Expr::bin(Expr::name(x.clone()), "%", Expr::name(y.clone())),
                ast::ArithOp::BitOr => Expr::bin(Expr::name(x.clone()), "|", Expr::name(y.clone())),
                ast::ArithOp::BitAnd => Expr::bin(Expr::name(x.clone()), "&", Expr::name(y.clone())),
                ast::ArithOp::BitXor => Expr::bin(Expr::name(x.clone()), "^", Expr::name(y.clone())),
                _ => return None,
            };
            let _ = v_ty;
            let f = Expr::Lambda { params: vec![x, y], body: LambdaBody::Expr(Box::new(body)) };
            Some(Expr::call(map, "merge", vec![key, r, f]))
        }
    }
}

// ---- integers --------------------------------------------------------------------------------

fn lower_int_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: Expr,
    args: Vec<Expr>,
    arg_asts: &[ast::Expr],
    i: Int,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (m, arg_asts);
    let long = i.is_long();
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0"));
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    let bits = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), f, a);
    match name {
        "abs" => Some(narrow(math("abs", vec![recv]), i)),
        "signum" => Some(narrow(Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), "signum", vec![recv]), i)),
        "pow" => Some(Expr::Cast(i.java().into(), Box::new(math("pow", vec![recv, b0])))),
        "min" => Some(narrow(math("min", vec![recv, b0]), i)),
        "max" => Some(narrow(math("max", vec![recv, b0]), i)),
        "rem_euclid" => Some(narrow(math("floorMod", vec![recv, math("abs", vec![b0])]), i)),
        "div_euclid" => Some(narrow(math("floorDiv", vec![recv, b0]), i)),
        "div_ceil" => Some(narrow(Expr::bin(Expr::bin(Expr::bin(recv, "+", b0.clone()), "-", Expr::raw("1")), "/", b0), i)),
        "count_ones" => Some(bits("bitCount", vec![recv])),
        "count_zeros" => Some(Expr::bin(Expr::raw(if long { "64" } else { "32" }), "-", bits("bitCount", vec![recv]))),
        "leading_zeros" => Some(bits("numberOfLeadingZeros", vec![recv])),
        "trailing_zeros" => Some(bits("numberOfTrailingZeros", vec![recv])),
        "swap_bytes" => Some(bits("reverseBytes", vec![recv])),
        "reverse_bits" => Some(bits("reverse", vec![recv])),
        "rotate_left" => Some(bits("rotateLeft", vec![recv, b0])),
        "rotate_right" => Some(bits("rotateRight", vec![recv, b0])),
        "is_power_of_two" => Some(Expr::bin(Expr::bin(Expr::bin(recv.clone(), "&", Expr::bin(recv.clone(), "-", Expr::raw("1"))), "==", Expr::raw("0")), "&&", Expr::bin(recv, ">", Expr::raw("0")))),
        "is_positive" => Some(Expr::bin(recv, ">", Expr::raw("0"))),
        "is_negative" => Some(Expr::bin(recv, "<", Expr::raw("0"))),
        "cmp" => Some(Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), "compare", vec![recv, b0])),
        "partial_cmp" => Some(opt_of(Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), "compare", vec![recv, b0]))),
        "to_string" => Some(Expr::call(Expr::raw("String"), "valueOf", vec![recv])),
        "clone" | "to_owned" => Some(recv),
        "abs_diff" => Some(narrow(math("abs", vec![Expr::bin(recv, "-", b0)]), i)),
        _ if name.starts_with("checked_") => Some(checked_int(cx, recv, b0, b1, i, name, ret, line, out)),
        _ if name.starts_with("saturating_") => Some(saturating_int(recv, b0, i, name)),
        _ if name.starts_with("wrapping_") => Some(wrapping_int(recv, b0, i, name)),
        _ => None,
    }
}

fn checked_int(cx: &mut Cx, recv: Expr, b0: Expr, _b1: Expr, i: Int, name: &str, ret: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let inner = opt_inner(ret);
    let empty = opt_empty(inner.as_ref());
    let yes = |v: Expr| opt_of(narrow(v, i));
    let uint = i.is_unsigned();
    let try_opt = |cx: &mut Cx, out: &mut Vec<Stmt>, v: Expr, exc: &str| {
        let ty = format!("Optional<{}>", inner.as_ref().map(|t| t.boxed()).unwrap_or_else(|| "Integer".into()));
        let n = cx.fresh("chk");
        let ex = cx.fresh("ex");
        push_raw(out, line, format!("{ty} {n}; try {{ {n} = Optional.of({}); }} catch ({exc} {ex}) {{ {n} = Optional.empty(); }}", render(&v)));
        Expr::name(n)
    };
    match name {
        "checked_add" => match i {
            Int::U8 => conditional(Expr::bin(Expr::bin(recv.clone(), "+", b0.clone()), "<=", Expr::raw("255")), yes(Expr::bin(recv, "+", b0)), empty),
            Int::U16 => conditional(Expr::bin(Expr::bin(recv.clone(), "+", b0.clone()), "<=", Expr::raw("65535")), yes(Expr::bin(recv, "+", b0)), empty),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(Expr::bin(Expr::Cast("long".into(), Box::new(recv.clone())), "+", b0.clone()), "<=", Expr::raw("4294967295L")),
                yes(Expr::bin(recv, "+", b0)),
                empty,
            ),
            Int::U64 | Int::U128 => try_opt(cx, out, Expr::call(Expr::raw("Math"), "addExact", vec![recv, b0]), "ArithmeticException"),
            Int::I8 => range_checked(recv, b0, i, "+", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "+", -32768, 32767, inner.as_ref()),
            _ => try_opt(cx, out, Expr::call(Expr::raw("Math"), "addExact", vec![recv, b0]), "ArithmeticException"),
        },
        "checked_sub" => match i {
            Int::U8 | Int::U16 | Int::U32 | Int::Usize => {
                conditional(Expr::bin(recv.clone(), ">=", b0.clone()), yes(Expr::bin(recv, "-", b0)), empty)
            }
            Int::U64 | Int::U128 => conditional(
                Expr::bin(Expr::call(Expr::raw("Long"), "compareUnsigned", vec![recv.clone(), b0.clone()]), ">=", Expr::raw("0")),
                yes(Expr::bin(recv, "-", b0)),
                empty,
            ),
            Int::I8 => range_checked(recv, b0, i, "-", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "-", -32768, 32767, inner.as_ref()),
            _ => try_opt(cx, out, Expr::call(Expr::raw("Math"), "subtractExact", vec![recv, b0]), "ArithmeticException"),
        },
        "checked_mul" => match i {
            Int::U8 => conditional(
                Expr::bin(Expr::bin(recv.clone(), "*", b0.clone()), "<=", Expr::raw("255")),
                yes(Expr::bin(recv, "*", b0)),
                empty,
            ),
            Int::U16 => conditional(
                Expr::bin(Expr::bin(Expr::Cast("long".into(), Box::new(recv.clone())), "*", b0.clone()), "<=", Expr::raw("65535")),
                yes(Expr::Cast("int".into(), Box::new(Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b0)))),
                empty,
            ),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(Expr::bin(Expr::Cast("long".into(), Box::new(recv.clone())), "*", b0.clone()), "<=", Expr::raw("4294967295L")),
                yes(Expr::Cast("int".into(), Box::new(Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b0)))),
                empty,
            ),
            Int::I8 => range_checked(recv, b0, i, "*", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "*", -32768, 32767, inner.as_ref()),
            _ => try_opt(cx, out, Expr::call(Expr::raw("Math"), "multiplyExact", vec![recv, b0]), "ArithmeticException"),
        },
        "checked_div" => {
            let minv = int_min(i);
            let cond = if uint {
                Expr::bin(b0.clone(), "!=", Expr::raw("0"))
            } else {
                Expr::bin(
                    Expr::bin(b0.clone(), "!=", Expr::raw("0")),
                    "&&",
                    Expr::Unary("!", Box::new(Expr::bin(Expr::bin(recv.clone(), "==", Expr::raw(minv)), "&&", Expr::bin(b0.clone(), "==", Expr::raw("-1"))))),
                )
            };
            conditional(cond, yes(Expr::bin(recv, "/", b0)), empty)
        }
        "checked_rem" => conditional(Expr::bin(b0.clone(), "!=", Expr::raw("0")), yes(Expr::bin(recv, "%", b0)), empty),
        "checked_neg" => {
            let minv = int_min(i);
            conditional(Expr::bin(recv.clone(), "==", Expr::raw(minv)), empty, yes(Expr::Unary("-", Box::new(recv))))
        }
        "checked_abs" => {
            let minv = int_min(i);
            conditional(
                Expr::bin(recv.clone(), "==", Expr::raw(minv)),
                empty,
                yes(Expr::call(Expr::raw("Math"), "abs", vec![recv])),
            )
        }
        _ => Expr::raw("null"),
    }
}

fn conditional(cond: Expr, a: Expr, b: Expr) -> Expr {
    Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
}

fn range_checked(recv: Expr, b: Expr, i: Int, op: &'static str, lo: i64, hi: i64, _inner: Option<&Ty>) -> Expr {
    let val = Expr::bin(recv, op, b);
    let cond = Expr::bin(
        Expr::bin(val.clone(), ">=", Expr::raw(lo.to_string())),
        "&&",
        Expr::bin(val.clone(), "<=", Expr::raw(hi.to_string())),
    );
    Expr::Ternary(Box::new(cond), Box::new(opt_of(narrow(val, i))), Box::new(Expr::raw("Optional.empty()")))
}

fn int_min(i: Int) -> &'static str {
    match i {
        Int::I8 => "-128",
        Int::I16 => "-32768",
        Int::I64 | Int::I128 => "Long.MIN_VALUE",
        _ => "Integer.MIN_VALUE",
    }
}

fn saturating_int(recv: Expr, b: Expr, i: Int, name: &str) -> Expr {
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    match name {
        "saturating_add" => match i {
            Int::U8 => narrow(math("min", vec![Expr::bin(recv, "+", b), Expr::raw("255")]), i),
            Int::U16 => narrow(math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b), Expr::raw("65535L")]), i),
            Int::U32 | Int::Usize => narrow(math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b), Expr::raw("4294967295L")]), i),
            Int::I8 | Int::I16 => narrow(math("min", vec![Expr::raw(int_max(i)), Expr::bin(recv, "+", b)]), i),
            Int::I32 => narrow(
                math("max", vec![Expr::raw("Integer.MIN_VALUE"), math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b), Expr::raw("Integer.MAX_VALUE")])]),
                i,
            ),
            _ => Expr::bin(recv, "+", b),
        },
        "saturating_sub" => match i {
            Int::U8 | Int::U16 | Int::U32 | Int::Usize => narrow(math("max", vec![Expr::bin(recv, "-", b), Expr::raw("0")]), i),
            Int::I8 | Int::I16 => narrow(math("max", vec![Expr::raw(int_min(i)), Expr::bin(recv, "-", b)]), i),
            Int::I32 => narrow(math("max", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "-", b), Expr::raw("Integer.MIN_VALUE")]), i),
            _ => Expr::bin(recv, "-", b),
        },
        "saturating_mul" => match i {
            Int::U8 => narrow(math("min", vec![Expr::bin(recv, "*", b), Expr::raw("255")]), i),
            Int::U16 => narrow(math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b), Expr::raw("65535L")]), i),
            Int::U32 | Int::Usize => narrow(
                math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b), Expr::raw("4294967295L")]),
                i,
            ),
            Int::I32 => narrow(
                math("max", vec![Expr::raw("Integer.MIN_VALUE"), math("min", vec![Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b), Expr::raw("Integer.MAX_VALUE")])]),
                i,
            ),
            _ => Expr::bin(recv, "*", b),
        },
        "saturating_abs" => {
            let m = int_min(i);
            conditional(Expr::bin(recv.clone(), "==", Expr::raw(m)), narrow(Expr::raw(int_max(i)), i), narrow(math("abs", vec![recv]), i))
        }
        "saturating_neg" => {
            let m = int_min(i);
            conditional(Expr::bin(recv.clone(), "==", Expr::raw(m)), narrow(Expr::raw(int_max(i)), i), narrow(Expr::Unary("-", Box::new(recv)), i))
        }
        _ => Expr::bin(recv, "*", b),
    }
}

fn int_max(i: Int) -> &'static str {
    match i {
        Int::I8 => "127",
        Int::I16 => "32767",
        Int::I32 | Int::Isize => "Integer.MAX_VALUE",
        Int::I64 | Int::I128 => "Long.MAX_VALUE",
        Int::U8 => "255",
        Int::U16 => "65535",
        Int::U32 | Int::Usize => "4294967295L",
        Int::U64 | Int::U128 => "-1L",
    }
}

fn wrapping_int(recv: Expr, b: Expr, i: Int, name: &str) -> Expr {
    match name {
        "wrapping_add" => narrow(Expr::bin(recv, "+", b), i),
        "wrapping_sub" => narrow(Expr::bin(recv, "-", b), i),
        "wrapping_mul" => narrow(Expr::bin(recv, "*", b), i),
        "wrapping_div" => narrow(Expr::bin(recv, "/", b), i),
        "wrapping_rem" => narrow(Expr::bin(recv, "%", b), i),
        "wrapping_neg" => narrow(Expr::Unary("-", Box::new(recv)), i),
        "wrapping_shl" => narrow(Expr::bin(recv, "<<", b), i),
        "wrapping_shr" => narrow(Expr::bin(recv, if i.is_unsigned() { ">>>" } else { ">>" }, b), i),
        "wrapping_abs" => narrow(Expr::call(Expr::raw("Math"), "abs", vec![recv]), i),
        _ => recv,
    }
}

// ---- floats ----------------------------------------------------------------------------------

fn lower_float_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: Expr,
    args: Vec<Expr>,
    f32: bool,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (m, ret);
    let cls = if f32 { "Float" } else { "Double" };
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    let cast = |v: Expr| if f32 { Expr::Cast("float".into(), Box::new(v)) } else { v };
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0.0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0.0"));
    Some(match name {
        "abs" => math("abs", vec![recv]),
        "min" => math("min", vec![recv, b0]),
        "max" => math("max", vec![recv, b0]),
        "sqrt" => cast(math("sqrt", vec![recv])),
        "floor" => cast(math("floor", vec![recv])),
        "ceil" => cast(math("ceil", vec![recv])),
        "trunc" => cast(Expr::Ternary(Box::new(Expr::bin(recv.clone(), "<", Expr::raw("0"))), Box::new(math("ceil", vec![recv.clone()])), Box::new(math("floor", vec![recv])))),
        "round" => {
            let s = simple(cx, recv, &if f32 { Ty::F32 } else { Ty::F64 }, line, out);
            let r = render(&s);
            cast(Expr::raw(format!("Math.copySign(Math.floor(Math.abs({r}) + 0.5), {r})")))
        }
        "powi" | "powf" => cast(math("pow", vec![recv, b0])),
        "mul_add" => Expr::bin(Expr::bin(recv, "*", b0), "+", b1),
        "rem_euclid" => {
            let s = simple(cx, recv, &if f32 { Ty::F32 } else { Ty::F64 }, line, out);
            let r = render(&s);
            let b = render(&b0);
            Expr::raw(format!("({r} - Math.floor({r} / Math.abs({b})) * Math.abs({b}))"))
        }
        "is_nan" => Expr::call(Expr::raw(cls), "isNaN", vec![recv]),
        "is_infinite" => Expr::call(Expr::raw(cls), "isInfinite", vec![recv]),
        "is_finite" => Expr::bin(
            Expr::Unary("!", Box::new(Expr::call(Expr::raw(cls), "isInfinite", vec![recv.clone()]))),
            "&&",
            Expr::Unary("!", Box::new(Expr::call(Expr::raw(cls), "isNaN", vec![recv]))),
        ),
        "signum" => cast(math("signum", vec![recv])),
        "copysign" => cast(math("copySign", vec![recv, b0])),
        "to_degrees" => cast(math("toDegrees", vec![recv])),
        "to_radians" => cast(math("toRadians", vec![recv])),
        "ln" => cast(math("log", vec![recv])),
        "log2" => cast(math("log", vec![recv])),
        "log10" => cast(math("log10", vec![recv])),
        "exp" => cast(math("exp", vec![recv])),
        "sin" => cast(math("sin", vec![recv])),
        "cos" => cast(math("cos", vec![recv])),
        "tan" => cast(math("tan", vec![recv])),
        "asin" => cast(math("asin", vec![recv])),
        "acos" => cast(math("acos", vec![recv])),
        "atan" => cast(math("atan", vec![recv])),
        "atan2" => cast(math("atan2", vec![recv, b0])),
        "hypot" => cast(math("hypot", vec![recv, b0])),
        "clamp" => cast(math("max", vec![b0, math("min", vec![b1, recv])])),
        "cmp" | "total_cmp" => Expr::call(Expr::raw(cls), "compare", vec![recv, b0]),
        "partial_cmp" => opt_of(Expr::call(Expr::raw(cls), "compare", vec![recv, b0])),
        "to_string" => Expr::raw(format!(
            "new BigDecimal(String.valueOf({})).stripTrailingZeros().toPlainString()",
            render(&recv)
        )),
        "clone" | "to_owned" => recv,
        _ => return None,
    })
}

// ---- bool / char -----------------------------------------------------------------------------

fn lower_small_method(
    cx: &mut Cx,
    recv: Expr,
    args: &[Expr],
    ty: &Ty,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    if ty.strip_refs() == &Ty::Bool {
        return Some(match name {
            "to_string" => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
            "cmp" => Expr::call(Expr::raw("Boolean"), "compare", vec![recv, b0]),
            "partial_cmp" => opt_of(Expr::call(Expr::raw("Boolean"), "compare", vec![recv, b0])),
            "clone" | "to_owned" => recv,
            "then_some" => opt_of(b0),
            _ => return None,
        });
    }
    let c = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Character"), f, a);
    Some(match name {
        "is_alphabetic" | "is_uppercase" | "is_lowercase" => c(
            if name == "is_alphabetic" { "isLetter" } else if name == "is_uppercase" { "isUpperCase" } else { "isLowerCase" },
            vec![recv],
        ),
        "is_numeric" => c("isDigit", vec![recv]),
        "is_alphanumeric" => c("isLetterOrDigit", vec![recv]),
        "is_whitespace" => c("isWhitespace", vec![recv]),
        "is_control" => c("isISOControl", vec![recv]),
        "is_ascii" => Expr::bin(recv, "<", Expr::raw("128")),
        "is_ascii_digit" => Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'0'")), "&&", Expr::bin(recv, "<=", Expr::raw("'9'"))),
        "is_ascii_alphabetic" => {
            let lower = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'a'")), "&&", Expr::bin(recv.clone(), "<=", Expr::raw("'z'")));
            let upper = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'A'")), "&&", Expr::bin(recv, "<=", Expr::raw("'Z'")));
            Expr::bin(lower, "||", upper)
        }
        "is_ascii_alphanumeric" => {
            let digit = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'0'")), "&&", Expr::bin(recv.clone(), "<=", Expr::raw("'9'")));
            let lower = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'a'")), "&&", Expr::bin(recv.clone(), "<=", Expr::raw("'z'")));
            let upper = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'A'")), "&&", Expr::bin(recv, "<=", Expr::raw("'Z'")));
            Expr::bin(Expr::bin(digit, "||", lower), "||", upper)
        }
        "is_ascii_whitespace" => Expr::bin(
            Expr::bin(recv.clone(), "==", Expr::raw("' '")),
            "||",
            Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'\\t'")), "&&", Expr::bin(recv, "<=", Expr::raw("'\\r'"))),
        ),
        "is_ascii_uppercase" => Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'A'")), "&&", Expr::bin(recv, "<=", Expr::raw("'Z'"))),
        "is_ascii_lowercase" => Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'a'")), "&&", Expr::bin(recv, "<=", Expr::raw("'z'"))),
        "is_ascii_hexdigit" => {
            let d = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'0'")), "&&", Expr::bin(recv.clone(), "<=", Expr::raw("'9'")));
            let l = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'a'")), "&&", Expr::bin(recv.clone(), "<=", Expr::raw("'f'")));
            let u = Expr::bin(Expr::bin(recv.clone(), ">=", Expr::raw("'A'")), "&&", Expr::bin(recv, "<=", Expr::raw("'F'")));
            Expr::bin(Expr::bin(d, "||", l), "||", u)
        }
        "is_digit" => Expr::bin(Expr::call(Expr::raw("Character"), "digit", vec![recv, b0]), ">=", Expr::raw("0")),
        "to_digit" => {
            let d = Expr::call(Expr::raw("Character"), "digit", vec![recv, b0]);
            let n = cx.fresh("digit");
            push_raw(
                out,
                line,
                format!("int {n} = {};", render(&d)),
            );
            conditional(Expr::bin(Expr::name(n.clone()), ">=", Expr::raw("0")), opt_of(Expr::name(n.clone())), opt_empty(opt_inner(ret).as_ref()))
        }
        "to_ascii_uppercase" | "to_uppercase" => c("toUpperCase", vec![recv]),
        "to_ascii_lowercase" | "to_lowercase" => c("toLowerCase", vec![recv]),
        "to_string" => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        "eq_ignore_ascii_case" => Expr::bin(c("toLowerCase", vec![recv]), "==", c("toLowerCase", vec![b0])),
        "cmp" => Expr::call(Expr::raw("Character"), "compare", vec![recv, b0]),
        "partial_cmp" => opt_of(Expr::call(Expr::raw("Character"), "compare", vec![recv, b0])),
        "clone" | "to_owned" => recv,
        _ => return None,
    })
}

// ---- strings ---------------------------------------------------------------------------------

fn lower_str_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    arg_asts: &[ast::Expr],
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::str_lit(""));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::str_lit(""));
    let arg0_ty = arg_asts.first().map(|a| cx.ty(a)).unwrap_or_else(|| Ty::Str);
    match name {
        "len" => return Some(Expr::call(recv, "length", vec![])),
        "is_empty" => return Some(Expr::call(recv, "isEmpty", vec![])),
        "push_str" => {
            let v = Expr::bin(recv, "+", b0);
            return Some(expr::place_assign(cx, recv_ast, "=", v, out));
        }
        "push" => {
            let v = Expr::bin(recv, "+", b0);
            return Some(expr::place_assign(cx, recv_ast, "=", v, out));
        }
        "clear" => return Some(expr::place_assign(cx, recv_ast, "=", Expr::str_lit(""), out)),
        "insert" => {
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let pre = Expr::call(recv.clone(), "substring", vec![Expr::raw("0"), i.clone()]);
            let post = Expr::call(recv.clone(), "substring", vec![i]);
            let v = Expr::bin(Expr::bin(pre, "+", b1), "+", post);
            return Some(expr::place_assign(cx, recv_ast, "=", v, out));
        }
        "pop" => {
            let p = expr::lower_expr(cx, recv_ast, out);
            let ps = render(&p);
            let n = push_local(
                cx,
                line,
                "Optional<Character>".into(),
                Some(Expr::raw(format!(
                    "{ps}.isEmpty() ? Optional.<Character>empty() : Optional.of({ps}.charAt({ps}.length() - 1))"
                ))),
                out,
                "popped",
            );
            push_raw(out, line, format!("if ({n}.isPresent()) {ps} = {ps}.substring(0, {ps}.length() - 1);"));
            return Some(Expr::name(n));
        }
        "truncate" => {
            let p = expr::lower_expr(cx, recv_ast, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (ps, ns) = (render(&p), render(&n));
            push_raw(out, line, format!("if ({ps}.length() > {ns}) {ps} = {ps}.substring(0, {ns});"));
            return Some(Expr::raw("null"));
        }
        "contains" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) { Expr::call(Expr::raw("String"), "valueOf", vec![b0]) } else { b0 };
            return Some(Expr::call(recv, "contains", vec![arg]));
        }
        "starts_with" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) { Expr::call(Expr::raw("String"), "valueOf", vec![b0]) } else { b0 };
            return Some(Expr::call(recv, "startsWith", vec![arg]));
        }
        "ends_with" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) { Expr::call(Expr::raw("String"), "valueOf", vec![b0]) } else { b0 };
            return Some(Expr::call(recv, "endsWith", vec![arg]));
        }
        "find" | "rfind" => {
            let getter = if name == "find" { "indexOf" } else { "lastIndexOf" };
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let av = simple(cx, b0, &arg0_ty, line, out);
            let idx = Expr::call(sv, getter, vec![av]);
            let n = push_local(cx, line, "int".into(), Some(idx), out, "idx");
            return Some(conditional(
                Expr::bin(Expr::name(n.clone()), ">=", Expr::raw("0")),
                opt_of(Expr::name(n.clone())),
                opt_empty(opt_inner(ret).as_ref()),
            ));
        }
        "replace" => return Some(Expr::call(recv, "replace", vec![b0, b1])),
        "trim" => return Some(Expr::call(recv, "strip", vec![])),
        "trim_start" => return Some(Expr::call(recv, "stripLeading", vec![])),
        "trim_end" => return Some(Expr::call(recv, "stripTrailing", vec![])),
        "to_uppercase" | "to_ascii_uppercase" => {
            return Some(Expr::call(recv, "toUpperCase", vec![Expr::raw("Locale.ROOT")]))
        }
        "to_lowercase" | "to_ascii_lowercase" => {
            return Some(Expr::call(recv, "toLowerCase", vec![Expr::raw("Locale.ROOT")]))
        }
        "repeat" => return Some(Expr::call(recv, "repeat", vec![b0])),
        "eq_ignore_ascii_case" => return Some(Expr::call(recv, "equalsIgnoreCase", vec![b0])),
        "as_str" | "as_ref" | "to_owned" | "to_string" | "clone" | "into" => return Some(recv),
        "parse" => return Some(parse_expr(cx, _m, recv, ret, line, out)),
        "get" => {
            if let Some(ast::Expr::RangeExpr(r)) = arg_asts.first() {
                let sub = lower_slice_range(cx, recv, &Ty::Str, r, out);
                return Some(opt_of(sub));
            }
            return None;
        }
        "split_at" => {
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let a = Expr::call(recv.clone(), "substring", vec![Expr::raw("0"), i.clone()]);
            let b = Expr::call(recv, "substring", vec![i]);
            return Some(Expr::call(Expr::raw("Map"), "entry", vec![a, b]));
        }
        "strip_prefix" | "strip_suffix" => {
            let test = if name == "strip_prefix" { "startsWith" } else { "endsWith" };
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let av = simple(cx, b0, &arg0_ty, line, out);
            let inner = if name == "strip_prefix" {
                Expr::call(sv.clone(), "substring", vec![Expr::call(av.clone(), "length", vec![])])
            } else {
                Expr::call(sv.clone(), "substring", vec![Expr::raw("0"), Expr::bin(Expr::call(sv.clone(), "length", vec![]), "-", Expr::call(av.clone(), "length", vec![]))])
            };
            return Some(conditional(Expr::call(sv, test, vec![av]), opt_of(inner), opt_empty(opt_inner(ret).as_ref())));
        }
        _ => {}
    }
    None
}

fn parse_expr(cx: &mut Cx, m: &ast::MethodCallExpr, recv: Expr, ret: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let target = match ret.strip_refs() {
        Ty::Result(t, _) => (**t).clone(),
        _ => Ty::Other("?".into()),
    };
    let call = match target.strip_refs() {
        Ty::Int(i) if i.is_long() => Expr::call(Expr::raw("Long"), "parseLong", vec![recv.clone()]),
        Ty::Int(_) => Expr::call(Expr::raw("Integer"), "parseInt", vec![recv.clone()]),
        Ty::F32 => Expr::call(Expr::raw("Float"), "parseFloat", vec![recv.clone()]),
        Ty::F64 => Expr::call(Expr::raw("Double"), "parseDouble", vec![recv.clone()]),
        Ty::Bool => Expr::call(Expr::raw("Boolean"), "parseBoolean", vec![recv.clone()]),
        Ty::Char => Expr::call(recv.clone(), "charAt", vec![Expr::raw("0")]),
        _ => return cx.unsupported(m.syntax(), "`parse` to this type"),
    };
    if matches!(consumed_by(cx, &ast::Expr::MethodCallExpr(m.clone())).as_deref(), Some("unwrap" | "expect")) {
        return call;
    }
    wrap_try_ok(cx, line, ret, call, "NumberFormatException", out)
}

// ---- Option ----------------------------------------------------------------------------------

fn lower_option_method(
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
            let msg = Expr::New { ty: "RuntimeException".into(), args: vec![b0] };
            Some(Expr::call(recv, "orElseThrow", vec![Expr::Lambda { params: vec![], body: LambdaBody::Expr(Box::new(msg)) }]))
        }
        "unwrap_or" => Some(Expr::call(recv, "orElse", vec![b0])),
        "unwrap_or_else" => Some(Expr::call(recv, "orElseGet", vec![b0])),
        "unwrap_or_default" => Some(Expr::call(recv, "orElse", vec![default_value(&inner.unwrap_or(Ty::Other("?".into())))])),
        "map" | "and_then" | "filter" => {
            let java = if name == "and_then" { "flatMap" } else { name };
            Some(Expr::call(recv, java, vec![b0]))
        }
        "map_or" => {
            // Rust: map_or(default, f)
            let mapped = Expr::call(recv, "map", vec![b1]);
            Some(Expr::call(mapped, "orElse", vec![b0]))
        }
        "or" => Some(Expr::call(recv, "or", vec![Expr::Lambda { params: vec![], body: LambdaBody::Expr(Box::new(b0)) }])),
        "or_else" => Some(Expr::call(recv, "or", vec![b0])),
        "ok_or" => {
            cx.needs_result = true;
            let v = cx.fresh("v");
            let ok_ty = inner.as_ref().map(|t| t.boxed()).unwrap_or_else(|| "Object".into());
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

fn opt_inner_of(cx: &Cx, e: &ast::Expr) -> Option<Ty> {
    opt_inner(&cx.ty(e))
}

fn opt_inner_from_recv(m: &ast::MethodCallExpr, cx: &Cx) -> Option<Ty> {
    m.receiver().and_then(|r| opt_inner_of(cx, &r))
}

// ---- Result ----------------------------------------------------------------------------------

fn lower_result_method(
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

fn is_unwrapped_source(cx: &Cx, recv_ast: &ast::Expr) -> bool {
    let _ = cx;
    match recv_ast {
        ast::Expr::MethodCallExpr(mc) => matches!(method_name(mc).as_str(), "parse" | "read_line" | "read_to_string"),
        ast::Expr::CallExpr(c) => {
            let text = c.expr().map(|e| e.syntax().text().to_string()).unwrap_or_default();
            text.ends_with("read_to_string")
                || text.ends_with("from_str_radix")
                || text.ends_with("from_utf8")
                || text.ends_with("fs::write")
                || text.ends_with("write")
        }
        _ => false,
    }
}

// ---- Vec / slices ----------------------------------------------------------------------------

fn lower_vec_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    elem: &Ty,
    args: Vec<Expr>,
    arg_asts: &[ast::Expr],
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0"));
    let witness = opt_inner(ret);
    match name {
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "push" => Some(Expr::call(recv, "add", vec![b0])),
        "pop" => {
            let cond = Expr::call(recv.clone(), "isEmpty", vec![]);
            Some(conditional(cond, opt_empty(witness.as_ref()), opt_of(Expr::call(recv, "removeLast", vec![]))))
        }
        "get" => {
            if let Some(ast::Expr::RangeExpr(r)) = arg_asts.first() {
                let sub = lower_slice_range(cx, recv, &Ty::Vec(Box::new(elem.clone())), r, out);
                return Some(opt_of(sub));
            }
            let cond = Expr::bin(b0.clone(), "<", Expr::call(recv.clone(), "size", vec![]));
            Some(conditional(cond, opt_of(Expr::call(recv, "get", vec![b0])), opt_empty(witness.as_ref())))
        }
        "first" => Some(Expr::call(Expr::call(recv, "stream", vec![]), "findFirst", vec![])),
        "last" => {
            let cond = Expr::call(recv.clone(), "isEmpty", vec![]);
            Some(conditional(cond, opt_empty(witness.as_ref()), opt_of(Expr::call(recv, "getLast", vec![]))))
        }
        "insert" => Some(Expr::call(recv, "add", vec![b0, b1])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "swap" => Some(Expr::call(Expr::raw("Collections"), "swap", vec![recv, b0, b1])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "truncate" => {
            let p = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (ps, ns) = (render(&p), render(&n));
            push_raw(out, line, format!("if ({ps}.size() > {ns}) {ps}.subList({ns}, {ps}.size()).clear();"));
            Some(Expr::raw("null"))
        }
        "extend" => {
            let at = arg_asts.first().map(|a| cx.ty(a)).unwrap_or_else(|| Ty::Other("?".into()));
            let arg = if matches!(at.strip_refs(), Ty::Iter(_)) {
                Expr::call(b0, "collect", vec![Expr::raw("Collectors.toList()")])
            } else {
                b0
            };
            Some(Expr::call(recv, "addAll", vec![arg]))
        }
        "retain" => {
            let f = negate_lambda(b0);
            Some(Expr::call(recv, "removeIf", vec![f]))
        }
        "dedup" => {
            let p = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let ps = render(&p);
            let i = cx.fresh("i");
            push_raw(
                out,
                line,
                format!("for (int {i} = {ps}.size() - 1; {i} > 0; {i}--) if ({ps}.get({i}).equals({ps}.get({i} - 1))) {ps}.remove({i});"),
            );
            Some(Expr::raw("null"))
        }
        "sort" | "sort_unstable" => Some(Expr::call(recv, "sort", vec![Expr::raw("null")])),
        "sort_by" => Some(Expr::call(recv, "sort", vec![b0])),
        "sort_by_key" => {
            let key_ty = match arg_asts.first() {
                Some(ast::Expr::ClosureExpr(c)) => c.body().map(|b| cx.ty(&b)),
                _ => None,
            };
            let cmp = match key_ty.as_ref().map(|t| t.strip_refs().clone()) {
                Some(Ty::Int(i)) if i.is_long() => "comparingLong",
                Some(Ty::Int(_)) => "comparingInt",
                Some(Ty::F32 | Ty::F64) => "comparingDouble",
                _ => "comparing",
            };
            let f = Expr::call(Expr::raw("Comparator"), cmp, vec![typed_lambda(b0, elem)]);
            Some(Expr::call(recv, "sort", vec![f]))
        }
        "sort_by_cached_key" => Some(Expr::call(Expr::raw("Comparator"), "comparing", vec![b0])),
        "reverse" => Some(Expr::call(Expr::raw("Collections"), "reverse", vec![recv])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "binary_search" | "binary_search_by" => {
            cx.needs_result = true;
            let n = push_local(cx, line, "int".into(), Some(Expr::call(Expr::raw("Collections"), "binarySearch", vec![recv, b0])), out, "pos");
            let idx = Expr::name(n);
            Some(conditional(
                Expr::bin(idx.clone(), ">=", Expr::raw("0")),
                Expr::New { ty: "Ok<Integer, Integer>".into(), args: vec![idx.clone()] },
                Expr::New { ty: "Err<Integer, Integer>".into(), args: vec![Expr::bin(Expr::Unary("-", Box::new(idx)), "-", Expr::raw("1"))] },
            ))
        }
        "split_at" => {
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let a = Expr::call(recv.clone(), "subList", vec![Expr::raw("0"), i.clone()]);
            let b = Expr::call(recv.clone(), "subList", vec![i, Expr::call(recv, "size", vec![])]);
            Some(Expr::call(Expr::raw("Map"), "entry", vec![a, b]))
        }
        "clone" => Some(clone_expr(recv, &Ty::Vec(Box::new(elem.clone())))),
        "as_slice" | "as_mut" | "as_mut_slice" | "into_boxed_slice" => Some(recv),
        "reserve" | "shrink_to_fit" => Some(Expr::raw("null")),
        _ => {
            let _ = m;
            None
        }
    }
}

// Give the single lambda parameter its Java type where javac cannot infer it.
fn typed_lambda(f: Expr, elem: &Ty) -> Expr {
    match f {
        Expr::Lambda { mut params, body } if params.len() == 1 && !params[0].contains(' ') => {
            params[0] = format!("{} {}", elem.boxed(), params[0]);
            Expr::Lambda { params, body }
        }
        other => other,
    }
}

fn negate_lambda(f: Expr) -> Expr {
    match f {
        Expr::Lambda { params, body: LambdaBody::Expr(e) } => {
            Expr::Lambda { params, body: LambdaBody::Expr(Box::new(Expr::Unary("!", e))) }
        }
        other => other,
    }
}

// ---- maps ------------------------------------------------------------------------------------

fn lower_map_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    ordered: bool,
    _k: &Ty,
    _v: &Ty,
    args: Vec<Expr>,
    _arg_asts: &[ast::Expr],
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("null"));
    match name {
        "insert" => {
            let put = Expr::call(recv, "put", vec![b0, b1]);
            if discarded(m) { Some(put) } else { Some(Expr::call(Expr::raw("Optional"), "ofNullable", vec![put])) }
        }
        "get" | "get_mut" => Some(Expr::call(Expr::raw("Optional"), "ofNullable", vec![Expr::call(recv, "get", vec![b0])])),
        "remove" => {
            let call = Expr::call(recv, "remove", vec![b0]);
            if discarded(m) { Some(call) } else { Some(Expr::call(Expr::raw("Optional"), "ofNullable", vec![call])) }
        }
        "contains_key" => Some(Expr::call(recv, "containsKey", vec![b0])),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "extend" => Some(Expr::call(recv, "putAll", vec![b0])),
        "clone" => Some(Expr::New {
            ty: if ordered { "TreeMap<>" } else { "HashMap<>" }.into(),
            args: vec![recv],
        }),
        "entry" => {
            let _ = m;
            Some(cx.unsupported(m.syntax(), "entry API (only or_insert* is supported)"))
        }
        "get_or_insert_with" => {
            let _ = (recv_ast, ret, line, out);
            None
        }
        _ => None,
    }
}

// ---- sets ------------------------------------------------------------------------------------

fn lower_set_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    ordered: bool,
    _elem: &Ty,
    args: Vec<Expr>,
    _arg_asts: &[ast::Expr],
    name: &str,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let _ = ordered;
    match name {
        "insert" => Some(Expr::call(recv, "add", vec![b0])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "extend" => Some(Expr::call(recv, "addAll", vec![b0])),
        "union" => {
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!("Stream.concat({s}.stream(), {o}.stream()).distinct()")))
        }
        "intersection" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!("{s}.stream().filter({x} -> {o}.contains({x}))")))
        }
        "difference" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!("{s}.stream().filter({x} -> !{o}.contains({x}))")))
        }
        "symmetric_difference" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!(
                "Stream.concat({s}.stream().filter({x} -> !{o}.contains({x})), {o}.stream().filter({x} -> !{s}.contains({x})))"
            )))
        }
        "is_subset" => {
            let x = cx.fresh("x");
            Some(Expr::raw(format!("{}.stream().allMatch({x} -> {}.contains({x}))", render(&recv), render(&b0))))
        }
        "is_superset" => {
            let x = cx.fresh("x");
            Some(Expr::raw(format!("{}.stream().allMatch({x} -> {}.contains({x}))", render(&b0), render(&recv))))
        }
        "clone" => Some(Expr::New {
            ty: if ordered { "TreeSet<>" } else { "HashSet<>" }.into(),
            args: vec![recv],
        }),
        _ => {
            let _ = (line, out);
            None
        }
    }
}

// ---- VecDeque / BinaryHeap ------------------------------------------------------------------

fn lower_deque_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    _elem: &Ty,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, ret, line, out);
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let opt_nullable = |f: &str| Expr::call(Expr::raw("Optional"), "ofNullable", vec![Expr::call(recv.clone(), f, vec![])]);
    match name {
        "push_back" | "push" => Some(Expr::call(recv, "addLast", vec![b0])),
        "push_front" => Some(Expr::call(recv, "addFirst", vec![b0])),
        "pop_front" | "pop" => Some(opt_nullable("pollFirst")),
        "pop_back" => Some(opt_nullable("pollLast")),
        "front" => Some(opt_nullable("peekFirst")),
        "back" => Some(opt_nullable("peekLast")),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "extend" => Some(Expr::call(recv, "addAll", vec![b0])),
        "get" => Some(Expr::call(Expr::call(Expr::call(recv, "stream", vec![]), "skip", vec![b0]), "findFirst", vec![])),
        "clone" => Some(Expr::New { ty: "ArrayDeque<>".into(), args: vec![recv] }),
        "iter" | "into_iter" => Some(Expr::call(recv, "stream", vec![])),
        _ => {
            let _ = m;
            None
        }
    }
}

fn lower_heap_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, ret, line, out);
    let opt_nullable = |f: &str| Expr::call(Expr::raw("Optional"), "ofNullable", vec![Expr::call(recv.clone(), f, vec![])]);
    match name {
        "push" => Some(Expr::call(recv, "add", vec![args.into_iter().next().unwrap_or_else(|| Expr::raw("null"))])),
        "pop" => Some(opt_nullable("poll")),
        "peek" => Some(opt_nullable("peek")),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "into_sorted_vec" => Some(Expr::call(Expr::call(recv, "stream", vec![]), "sorted", vec![])),
        "clone" => Some(Expr::New { ty: "PriorityQueue<>".into(), args: vec![recv] }),
        _ => None,
    }
}

fn lower_ordering_method(cx: &mut Cx, recv: Expr, args: Vec<Expr>, name: &str) -> Option<Expr> {
    let _ = (cx, args);
    Some(match name {
        "is_lt" => Expr::bin(recv, "<", Expr::raw("0")),
        "is_le" => Expr::bin(recv, "<=", Expr::raw("0")),
        "is_gt" => Expr::bin(recv, ">", Expr::raw("0")),
        "is_ge" => Expr::bin(recv, ">=", Expr::raw("0")),
        "is_eq" => Expr::bin(recv, "==", Expr::raw("0")),
        "reverse" => Expr::Unary("-", Box::new(recv)),
        "to_string" => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        "clone" => recv,
        _ => return None,
    })
}

// ---- I/O -------------------------------------------------------------------------------------

fn fs_read(cx: &mut Cx, c: &ast::CallExpr, ret: &Ty, path: Expr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let usage = consumed_by(cx, &ast::Expr::CallExpr(c.clone()));
    let keep = !matches!(usage.as_deref(), Some("unwrap" | "expect")) && !parent_is_stmt(&ast::Expr::CallExpr(c.clone()));
    let read = Expr::raw(format!("Files.readString(Path.of({}))", render(&path)));
    if keep {
        return wrap_try_ok(cx, line, ret, read, "IOException", out);
    }
    let text = cx.fresh("text");
    let ex = cx.fresh("ex");
    push_raw(
        out,
        line,
        format!("String {text}; try {{ {text} = {}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}", render(&read)),
    );
    Expr::name(text)
}

fn parent_is_stmt(e: &ast::Expr) -> bool {
    e.syntax().parent().is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
}

fn fs_write(cx: &mut Cx, c: &ast::CallExpr, ret: &Ty, path: Expr, contents: Expr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    cx.import("java.nio.file.*");
    cx.import("java.io.*");
    let write = format!("Files.writeString(Path.of({}), {})", render(&path), render(&contents));
    let usage = consumed_by(cx, &ast::Expr::CallExpr(c.clone()));
    if matches!(usage.as_deref(), Some("unwrap" | "expect")) || parent_is_stmt(&ast::Expr::CallExpr(c.clone())) {
        let ex = cx.fresh("ex");
        push_raw(out, line, format!("try {{ {write}; }} catch (IOException {ex}) {{ throw new UncheckedIOException({ex}); }}"));
        return Expr::raw("null");
    }
    cx.needs_result = true;
    let ty = result_java(ret);
    let n = cx.fresh("res");
    let ex = cx.fresh("ex");
    push_raw(
        out,
        line,
        format!("{ty} {n}; try {{ {write}; {n} = new Ok<>(null); }} catch (IOException {ex}) {{ {n} = new Err<>({ex}); }}"),
    );
    Expr::name(n)
}

// ---- arrays / slicing ------------------------------------------------------------------------

// `[a, b, c]` -> mutable list; `[x; n]` -> n independent copies when `x` is a mutable object.
pub fn lower_array_expr(cx: &mut Cx, a: &ast::ArrayExpr, out: &mut Vec<Stmt>) -> Expr {
    let exprs: Vec<ast::Expr> = a.exprs().collect();
    if a.semicolon_token().is_some() && exprs.len() == 2 {
        return lower_repeat(cx, a.syntax(), &exprs[0], &exprs[1], out);
    }
    let items: Vec<Expr> = exprs.iter().map(|e| expr::lower_expr(cx, e, out)).collect();
    if items.is_empty() {
        return Expr::New { ty: "ArrayList<>".into(), args: vec![] };
    }
    // `()` elements are `null` in Java, which `List.of` rejects.
    let factory = if exprs.iter().any(|e| cx.ty(e).is_unit()) { ("Arrays", "asList") } else { ("List", "of") };
    let list = Expr::call(Expr::raw(factory.0), factory.1, items);
    Expr::New { ty: "ArrayList<>".into(), args: vec![list] }
}

// `[x; n]` / `vec![x; n]`.
pub fn lower_repeat(cx: &mut Cx, node: &ra_ap_syntax::SyntaxNode, elem: &ast::Expr, count: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    let elem_ty = cx.ty(elem);
    let n = expr::lower_expr(cx, count, out);
    let n = simple(cx, n, &Ty::Int(Int::Usize), cx.line(node), out);
    if crate::lower::is_cell_value_ty(&elem_ty) || matches!(elem_ty.strip_refs(), Ty::Option(_)) {
        let x = expr::lower_expr(cx, elem, out);
        let copies = Expr::call(Expr::raw("Collections"), "nCopies", vec![n, x]);
        return Expr::New { ty: "ArrayList<>".into(), args: vec![copies] };
    }
    // Re-evaluate the element expression per slot: Rust clones it n times.
    let mut inner = Vec::new();
    cx.lambda_depth += 1;
    let x = expr::lower_expr(cx, elem, &mut inner);
    cx.lambda_depth -= 1;
    if !inner.is_empty() {
        return cx.unsupported(node, "repeat-array element needing statements");
    }
    let i = cx.fresh("i");
    let range = Expr::call(Expr::raw("IntStream"), "range", vec![Expr::raw("0"), n]);
    let mapped = Expr::call(range, "mapToObj", vec![Expr::Lambda { params: vec![i], body: LambdaBody::Expr(Box::new(x)) }]);
    Expr::call(mapped, "collect", vec![Expr::raw("Collectors.toCollection(ArrayList::new)")])
}

// `&v[a..b]`, `&s[a..]` -> subList / substring.
pub fn lower_slice_range(cx: &mut Cx, base: Expr, base_ty: &Ty, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    use ast::RangeItem;
    let line = cx.line(r.syntax());
    let start = r.start().map(|e| expr::lower_expr(cx, &e, out));
    let end = r.end().map(|e| expr::lower_expr(cx, &e, out));
    let inclusive = r.op_kind() == Some(ast::RangeOp::Inclusive);
    let end = end.map(|e| if inclusive { Expr::bin(e, "+", Expr::raw("1")) } else { e });
    let base = simple(cx, base, base_ty, line, out);
    let from = start.unwrap_or_else(|| Expr::raw("0"));
    match base_ty.strip_refs() {
        Ty::Str => {
            let to = match end {
                Some(e) => e,
                None => Expr::call(base.clone(), "length", vec![]),
            };
            Expr::call(base, "substring", vec![from, to])
        }
        Ty::Vec(_) => {
            let to = match end {
                Some(e) => e,
                None => Expr::call(base.clone(), "size", vec![]),
            };
            Expr::call(base, "subList", vec![from, to])
        }
        _ => cx.unsupported(r.syntax(), "slice by range on this type"),
    }
}
