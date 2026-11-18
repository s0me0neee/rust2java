// Std library calls, methods, paths and constructors -> Java standard library.

use ra_ap_hir::ModuleDef;
use ra_ap_syntax::{AstNode, ast, ast::HasArgList};

use super::{Cx, expr, traits};
use crate::emit::render_expr as render;
use crate::jir::{Expr, LambdaBody, Stmt, StmtKind};
use crate::ty::{Int, Ty};

pub(crate) mod coll;
pub mod io;
mod mem;
mod num;
mod option;
pub(crate) mod ptr;
mod text;

use coll::{
    entry_method, is_entry_recv, lower_deque_method, lower_heap_method, lower_map_method,
    lower_set_method, lower_vec_method,
};
use io::{duration_method, fs_read, fs_write, is_stdin, is_stdout, stdio_method};
use mem::{mem_replace, mem_swap, ref_inner};
use num::{conditional, lower_float_method, lower_int_method, lower_ordering_method};
use option::{lower_option_method, lower_result_method};
use text::{lower_small_method, lower_str_method};

pub use coll::{lower_array_expr, lower_entry_assign, lower_repeat, lower_slice_range};
pub(crate) use io::{
    bytes_stream, is_io_target, is_reader_source, lines_stream, output_handle, write_status,
};
pub(crate) use num::{
    compare_expr, num_cast, signum, unsigned_arith, unsigned_display, unsigned_literal,
    unsigned_string,
};

type StdFnHook = fn(&mut Cx, &ast::CallExpr, &str, &Ty, &mut Vec<Stmt>) -> Option<Expr>;
type StdMethodHook = fn(&mut Cx, &ast::MethodCallExpr, &Ty, &str, &mut Vec<Stmt>) -> Option<Expr>;

// ---- small helpers ---------------------------------------------------------------------------

fn arg_asts(c: &impl HasArgList) -> Vec<ast::Expr> {
    c.arg_list()
        .map(|al| al.args().collect())
        .unwrap_or_default()
}

fn method_name(m: &ast::MethodCallExpr) -> String {
    m.name_ref()
        .map(|n| n.text().to_string())
        .unwrap_or_default()
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
    Expr::Ternary(
        Box::new(cond),
        Box::new(opt_of(value)),
        Box::new(opt_empty(inner.as_ref())),
    )
}

// Java promotes byte/short arithmetic to int; cast back to the Rust type.
fn narrow(v: Expr, i: Int) -> Expr {
    match i {
        Int::I8 | Int::I16 | Int::U16 => Expr::Cast(i.java().into(), Box::new(v)),
        _ => v,
    }
}

fn push_local(
    cx: &mut Cx,
    line: u32,
    ty: String,
    init: Option<Expr>,
    out: &mut Vec<Stmt>,
    base: &str,
) -> String {
    let name = cx.fresh(base);
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty,
            name: name.clone(),
            init,
        },
    ));
    name
}

fn push_raw(out: &mut Vec<Stmt>, line: u32, text: String) {
    out.push(Stmt::new(line, StmtKind::Raw(text)));
}

// Re-evaluating `e` must be safe and cheap; otherwise bind it to a temporary on this line.
fn simple(cx: &mut Cx, e: Expr, ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    if is_simple(&e) || cx.lambda_depth > 0 {
        return e;
    }
    let name = push_local(cx, line, super::stmt::decl_type(ty), Some(e), out, "tmp");
    Expr::name(name)
}

fn is_simple(e: &Expr) -> bool {
    match e {
        Expr::Name(_) => true,
        Expr::Raw(s) => {
            s.starts_with('"')
                || s.starts_with('\'')
                || s.chars()
                    .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']'))
        }
        Expr::Call {
            recv: Some(r),
            name,
            args,
        } => {
            matches!(&**r, Expr::Raw(t) if t == "Optional")
                && name == "of"
                && args.len() == 1
                && is_simple(&args[0])
        }
        _ => false,
    }
}

fn consumed_by(cx: &Cx, e: &ast::Expr) -> Option<String> {
    let parent = e.syntax().parent()?;
    let mc = ast::MethodCallExpr::cast(parent)?;
    let is_recv = mc
        .receiver()
        .is_some_and(|r| r.syntax().text_range() == e.syntax().text_range());
    if is_recv {
        mc.name_ref().map(|n| n.text().to_string())
    } else {
        let _ = cx;
        None
    }
}

fn discarded(m: &ast::MethodCallExpr) -> bool {
    m.syntax()
        .parent()
        .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
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
    let name = push_local(
        cx,
        line,
        ty,
        Some(Expr::New {
            ty: "Ok<>".into(),
            args: vec![value],
        }),
        out,
        "res",
    );
    Expr::name(name)
}

fn wrap_try_ok(
    cx: &mut Cx,
    line: u32,
    ret: &Ty,
    value: Expr,
    catch: &str,
    out: &mut Vec<Stmt>,
) -> Expr {
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

// `new TreeSet<>(cmp)` / `new TreeMap<>(cmp)` when the key needs a comparator for Rust's `Ord`.
fn ordered_new(cx: &mut Cx, ty: &Ty) -> Option<Expr> {
    let (class, key) = match ty.strip_refs() {
        Ty::Set { ordered: true, t } => ("TreeSet<>", t),
        Ty::Map {
            ordered: true, k, ..
        } => ("TreeMap<>", k),
        _ => return None,
    };
    let cmp = coll::ord_comparator(cx, key)?;
    Some(Expr::New {
        ty: class.into(),
        args: vec![Expr::raw(cmp)],
    })
}

// A set/map filled from a list (of pairs, for a map); later duplicate keys win, as in Rust.
fn ordered_from(cx: &mut Cx, src: Expr, ty: &Ty) -> Expr {
    let new = match ordered_new(cx, ty) {
        Some(e) => format!("() -> {}", render(&e)),
        None => match ty.strip_refs() {
            Ty::Map { ordered, .. } | Ty::Set { ordered, .. } => {
                let kind = if matches!(ty.strip_refs(), Ty::Map { .. }) {
                    "Map"
                } else {
                    "Set"
                };
                format!("{}{kind}::new", if *ordered { "Tree" } else { "Hash" })
            }
            _ => "ArrayList::new".into(),
        },
    };
    let collector = match ty.strip_refs() {
        Ty::Map { .. } => {
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            format!(
                "Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue, ({a}, {b}) -> {b}, {new})"
            )
        }
        _ => format!("Collectors.toCollection({new})"),
    };
    Expr::call(
        Expr::call(src, "stream", vec![]),
        "collect",
        vec![Expr::raw(collector)],
    )
}

fn clone_expr(e: Expr, ty: &Ty) -> Expr {
    let new = |t: &str, e: Expr| Expr::New {
        ty: t.into(),
        args: vec![e],
    };
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
        Ty::Vec(_) => Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![],
        },
        Ty::Map { ordered, .. } => Expr::New {
            ty: if *ordered { "TreeMap<>" } else { "HashMap<>" }.into(),
            args: vec![],
        },
        Ty::Set { ordered, .. } => Expr::New {
            ty: if *ordered { "TreeSet<>" } else { "HashSet<>" }.into(),
            args: vec![],
        },
        Ty::Deque(_) => Expr::New {
            ty: "ArrayDeque<>".into(),
            args: vec![],
        },
        Ty::Heap(_) => Expr::New {
            ty: "PriorityQueue<>".into(),
            args: vec![],
        },
        Ty::Option(_) => Expr::raw("Optional.empty()"),
        Ty::Result(..) => Expr::raw("null"),
        _ => Expr::raw("null"),
    }
}

// Declares (once) the shared stdin reader on the file's last line.
pub fn need_stdin(cx: &mut Cx) {
    let decl = if cx.stdin_lines {
        "static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));"
    } else {
        "static final InputStream STDIN = new BufferedInputStream(System.in);"
    };
    if !cx.stdin_hoisted && !cx.file_trailer.iter().any(|t| t.contains("STDIN")) {
        cx.import("java.io.*");
        cx.file_trailer.push(decl.into());
    }
}

// ---- std free functions ----------------------------------------------------------------------

pub fn lower_std_fn_call(cx: &mut Cx, c: &ast::CallExpr, key: &str, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(c.syntax());
    let ret = cx.ty(&ast::Expr::CallExpr(c.clone()));
    // Per-area hooks first; each returns None without lowering anything when the key isn't its own.
    let hooks: [StdFnHook; 7] = [
        text::fn_call,
        num::fn_call,
        option::fn_call,
        coll::fn_call,
        io::fn_call,
        mem::fn_call,
        ptr::fn_call,
    ];
    for hook in hooks {
        if let Some(e) = hook(cx, c, key, &ret, out) {
            return e;
        }
    }
    match key {
        "mem::swap" => return mem_swap(cx, c, out),
        "mem::size_of" => return mem::mem_size_of(cx, c),
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
        "io::stderr" | "stdio::stderr" => return Expr::raw("System.err"),
        "env::args" | "env::args_os" => {
            cx.uses_args = true;
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
        "String::from" | "ToString::to_string" | "ToOwned::to_owned" => {
            args.into_iter().next().unwrap_or_else(|| Expr::str_lit(""))
        }
        "String::from_utf8"
        | "String::from_utf8_lossy"
        | "String::from_utf8_unchecked"
        | "str::from_utf8"
        | "str::from_utf8_unchecked" => {
            let src = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
            let b = cx.fresh("b");
            let joined = Expr::raw(format!(
                "{}.stream().map({b} -> String.valueOf((char) (int) {b})).collect(Collectors.joining())",
                render(&src)
            ));
            // Only the checked forms have a `Result` to keep.
            let fallible = matches!(key, "String::from_utf8" | "str::from_utf8");
            if fallible
                && consumed_by(cx, &ast::Expr::CallExpr(c.clone()))
                    .as_deref()
                    .is_none_or(|u| !matches!(u, "unwrap" | "expect"))
            {
                return wrap_ok(cx, line, &ret, joined, out);
            }
            joined
        }
        "Vec::new" | "Vec::with_capacity" => Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![],
        },
        "Vec::from" => Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![args[0].clone()],
        },
        "Vec::from_iter" | "Vec::collect" => Expr::call(
            args[0].clone(),
            "collect",
            vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
        ),
        "HashMap::new" | "HashMap::with_capacity" => Expr::New {
            ty: "HashMap<>".into(),
            args: vec![],
        },
        "BTreeMap::new" | "BTreeSet::new" if ordered_new(cx, &ret).is_some() => {
            ordered_new(cx, &ret).unwrap()
        }
        "BTreeMap::new" => Expr::New {
            ty: "TreeMap<>".into(),
            args: vec![],
        },
        "HashSet::new" | "HashSet::with_capacity" => Expr::New {
            ty: "HashSet<>".into(),
            args: vec![],
        },
        "BTreeSet::new" => Expr::New {
            ty: "TreeSet<>".into(),
            args: vec![],
        },
        "VecDeque::new" | "VecDeque::with_capacity" => Expr::New {
            ty: "ArrayDeque<>".into(),
            args: vec![],
        },
        "BinaryHeap::new" => {
            if heap_is_reverse(c) {
                Expr::New {
                    ty: "PriorityQueue<>".into(),
                    args: vec![],
                }
            } else {
                Expr::New {
                    ty: "PriorityQueue<>".into(),
                    args: vec![Expr::raw("Comparator.reverseOrder()")],
                }
            }
        }
        "BinaryHeap::from" => {
            let src = args[0].clone();
            if heap_is_reverse(c) {
                Expr::New {
                    ty: "PriorityQueue<>".into(),
                    args: vec![src],
                }
            } else {
                let name = push_local(
                    cx,
                    line,
                    "PriorityQueue<>".into(),
                    Some(Expr::New {
                        ty: "PriorityQueue<>".into(),
                        args: vec![Expr::raw("Comparator.reverseOrder()")],
                    }),
                    out,
                    "heap",
                );
                out.push(Stmt::new(
                    line,
                    StmtKind::Expr(Expr::call(Expr::name(name.clone()), "addAll", vec![src])),
                ));
                Expr::name(name)
            }
        }
        "Default::default" => default_value(&ret),
        "From::from" | "Into::into" => match ret.strip_refs() {
            Ty::Set { ordered: true, .. } | Ty::Map { ordered: true, .. }
                if ordered_new(cx, &ret).is_some() =>
            {
                ordered_from(cx, args[0].clone(), &ret)
            }
            Ty::Vec(_) | Ty::Set { .. } | Ty::Map { .. } | Ty::Deque(_) | Ty::Heap(_) => {
                clone_expr(args[0].clone(), &ret)
            }
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
        "char::from_u32_unchecked" => Expr::Cast(
            "char".into(),
            Box::new(args.first().cloned().unwrap_or_else(|| Expr::raw("0"))),
        ),
        "char::from_digit" => {
            let d = simple(
                cx,
                args.first().cloned().unwrap_or_else(|| Expr::raw("0")),
                &Ty::Int(Int::U32),
                line,
                out,
            );
            let r = simple(
                cx,
                args.get(1).cloned().unwrap_or_else(|| Expr::raw("10")),
                &Ty::Int(Int::U32),
                line,
                out,
            );
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
                format!(
                    "try {{ Thread.sleep({}); }} catch (InterruptedException {ex}) {{ throw new RuntimeException({ex}); }}",
                    render(&ms)
                ),
            );
            Expr::raw("null")
        }
        k if k.contains("::from_str_radix") => from_str_radix(cx, c, k, &ret, args, out),
        _ => match primitive_method_call(cx, c, key, &ret, line, out) {
            Some(e) => e,
            None => cx.unsupported(c.syntax(), &format!("std function `{key}`")),
        },
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

fn cmp_min_max(
    cx: &mut Cx,
    max: bool,
    arg_asts: &[ast::Expr],
    mut args: Vec<Expr>,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let (a, b) = (args.remove(0), args.remove(0));
    let ty = arg_asts
        .first()
        .map(|e| cx.ty(e))
        .unwrap_or_else(|| Ty::Int(Int::I32));
    let method = if max { "max" } else { "min" };
    match ty.strip_refs() {
        Ty::Int(i) if i.is_unsigned() && !matches!(i, Int::U8) => {
            num::min_max(cx, a, b, *i, max, line, out)
        }
        Ty::Int(_) | Ty::F32 | Ty::F64 => Expr::call(Expr::raw("Math"), method, vec![a, b]),
        Ty::Bool => {
            let op = if max { "||" } else { "&&" };
            Expr::bin(a, op, b)
        }
        Ty::Char => {
            let (a, b) = (simple(cx, a, &ty, line, out), simple(cx, b, &ty, line, out));
            Expr::Ternary(
                Box::new(Expr::bin(
                    a.clone(),
                    if max { ">=" } else { "<=" },
                    b.clone(),
                )),
                Box::new(a),
                Box::new(b),
            )
        }
        _ => {
            let (a, b) = (simple(cx, a, &ty, line, out), simple(cx, b, &ty, line, out));
            // `Ordering` is an int, so it compares by sign rather than `compareTo`.
            let cmp = num::compare_expr(&ty, a.clone(), b.clone())
                .unwrap_or_else(|| Expr::call(a.clone(), "compareTo", vec![b.clone()]));
            let cond = Expr::bin(cmp, if max { ">=" } else { "<=" }, Expr::raw("0"));
            Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
        }
    }
}

fn from_str_radix(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    args: Vec<Expr>,
    out: &mut Vec<Stmt>,
) -> Expr {
    let line = cx.line(c.syntax());
    let owner = key.split("::").next().unwrap_or("i32");
    let s = args.first().cloned().unwrap_or_else(|| Expr::str_lit(""));
    let radix = args.get(1).cloned().unwrap_or_else(|| Expr::raw("10"));
    if let Some(d) = text::direct_parse(cx, &ast::Expr::CallExpr(c.clone())) {
        return text::lower_direct_parse(cx, d, vec![s, radix], line, out);
    }
    let i = Int::from_name(owner).unwrap_or(Int::I32);
    option::parse_int(cx, s, ret, i, Some(radix), true, line, out)
}

// `f64::max(a, b)` / `char::to_digit(c, 10)`: a primitive method called through its type.
fn primitive_method_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let (owner, name) = key.split_once("::")?;
    let t = match owner {
        "f32" => Ty::F32,
        "f64" => Ty::F64,
        "char" => Ty::Char,
        "bool" => Ty::Bool,
        _ => Ty::Int(Int::from_name(owner)?),
    };
    let ast::Expr::PathExpr(p) = c.expr()? else {
        return None;
    };
    let Some(ra_ap_hir::PathResolution::Def(ModuleDef::Function(f))) =
        cx.sema.resolve_path(&p.path()?)
    else {
        return None;
    };
    f.self_param(cx.db)?;
    let mut args = expr::call_args(cx, c, out);
    if args.is_empty() {
        return None;
    }
    let recv = args.remove(0);
    match &t {
        Ty::F32 | Ty::F64 => lower_float_method(cx, recv, args, t == Ty::F32, name, ret, line, out),
        Ty::Int(i) => lower_int_method(cx, recv, args, *i, name, ret, line, out),
        _ => lower_small_method(cx, recv, &args, &t, name, ret, line, out),
    }
}

// ---- std paths (consts and unit variants) ----------------------------------------------------

pub fn lower_std_path(cx: &mut Cx, path: &ast::Path, def: ModuleDef) -> Expr {
    let text = path.syntax().text().to_string();
    let parts: Vec<&str> = text.split("::").collect();
    // `None::<T>` and `<i32>::MAX`: the last segment's name skips generic arguments.
    let last = path
        .segment()
        .and_then(|s| s.name_ref())
        .map(|n| n.text().to_string())
        .unwrap_or_else(|| parts.last().copied().unwrap_or("").to_string());
    let last = last.as_str();
    // `Ordering::Less` can be imported unqualified, so resolve it instead of trusting the text.
    if let ModuleDef::EnumVariant(v) = def
        && v.parent_enum(cx.db).name(cx.db).as_str() == "Ordering"
    {
        match last {
            "Less" => return Expr::raw("-1"),
            "Equal" => return Expr::raw("0"),
            "Greater" => return Expr::raw("1"),
            _ => {}
        }
    }
    let ty_tok = parts
        .iter()
        .rev()
        .find(|p| {
            matches!(
                **p,
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "char"
            )
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
        "PI" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.PI"
            } else {
                "Math.PI"
            });
        }
        "E" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.E"
            } else {
                "Math.E"
            });
        }
        "TAU" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.TAU"
            } else {
                "Math.TAU"
            });
        }
        "SQRT_2" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.sqrt(2)"
            } else {
                "Math.sqrt(2)"
            });
        }
        "LN_2" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.log(2)"
            } else {
                "Math.log(2)"
            });
        }
        "LN_10" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "(float) Math.log(10)"
            } else {
                "Math.log(10)"
            });
        }
        "EPSILON" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "Math.ulp(1.0f)"
            } else {
                "Math.ulp(1.0)"
            });
        }
        "INFINITY" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "Float.POSITIVE_INFINITY"
            } else {
                "Double.POSITIVE_INFINITY"
            });
        }
        "NEG_INFINITY" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "Float.NEGATIVE_INFINITY"
            } else {
                "Double.NEGATIVE_INFINITY"
            });
        }
        "NAN" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "Float.NaN"
            } else {
                "Double.NaN"
            });
        }
        "MIN_POSITIVE" => {
            return Expr::raw(if ty_tok == Some("f32") {
                "Float.MIN_NORMAL"
            } else {
                "Double.MIN_NORMAL"
            });
        }
        _ => {}
    }
    // Function values (`map(String::from)`), before the const/unit-variant paths above.
    if let Some(e) = text::path(cx, path, def) {
        return e;
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

pub fn lower_std_ctor(
    cx: &mut Cx,
    c: &ast::CallExpr,
    path: &ast::Path,
    args: &[Expr],
) -> Option<Expr> {
    let last = path.segment()?.name_ref()?.text().to_string();
    let _ = c;
    match last.as_str() {
        "Some" => Some(opt_of(
            args.first().cloned().unwrap_or_else(|| Expr::raw("null")),
        )),
        "Ok" => {
            cx.needs_result = true;
            Some(Expr::New {
                ty: "Ok<>".into(),
                args: args.to_vec(),
            })
        }
        "Err" => {
            cx.needs_result = true;
            Some(Expr::New {
                ty: "Err<>".into(),
                args: args.to_vec(),
            })
        }
        // Newtypes are erased.
        "Reverse" | "Wrapping" | "Saturating" => args.first().cloned(),
        _ => None,
    }
}

// ---- std methods ----------------------------------------------------------------------------

pub fn lower_std_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Expr {
    let recv_ast = m.receiver().unwrap();
    let name = key.rsplit("::").next().unwrap_or(key).to_string();
    let line = cx.line(m.syntax());
    // Per-area hooks run before the receiver is lowered (e.g. `rc.borrow_mut().push(x)`).
    let hooks: [StdMethodHook; 7] = [
        text::method,
        num::method,
        option::method,
        coll::method,
        io::method,
        mem::method,
        ptr::method,
    ];
    for hook in hooks {
        if let Some(e) = hook(cx, m, recv_ty, key, out) {
            return e;
        }
    }
    // `m.entry(k).or_insert*` must be handled before the `entry()` receiver is lowered.
    if matches!(name.as_str(), "or_insert" | "or_insert_with" | "or_default")
        && is_entry_recv(&recv_ast)
    {
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
        return Expr::call(
            Expr::raw("Duration"),
            "ofNanos",
            vec![Expr::bin(Expr::raw("System.nanoTime()"), "-", recv)],
        );
    }
    if let Some(e) = duration_method(cx, &name, recv.clone(), &args) {
        return e;
    }

    // `impl Deref for W { type Target = T; }`: method calls forward to `w.deref()`.
    let t = recv_ty.strip_refs().clone();
    let deref_forward = !matches!(
        name.as_str(),
        "to_string"
            | "clone"
            | "into"
            | "try_into"
            | "from"
            | "eq"
            | "ne"
            | "cmp"
            | "partial_cmp"
            | "hash"
            | "default"
            | "next"
            | "deref"
            | "borrow"
            | "borrow_mut"
            | "as_ref"
            | "as_mut"
    );
    let (recv, t) = match traits::deref_target(cx, recv_ty) {
        Some(target) if deref_forward => (Expr::call(recv.clone(), "deref", vec![]), target),
        _ => (recv.clone(), t),
    };
    let found = match &t {
        // `u8` ASCII tests and case maps share the char forms (a Java char is an int).
        Ty::Int(Int::U8)
            if name.starts_with("is_ascii")
                || name.starts_with("to_ascii_")
                || name == "eq_ignore_ascii_case" =>
        {
            lower_small_method(cx, recv.clone(), &args, &t, &name, &ret, line, out)
        }
        Ty::Int(i) => lower_int_method(cx, recv.clone(), args.clone(), *i, &name, &ret, line, out),
        Ty::F32 | Ty::F64 => lower_float_method(
            cx,
            recv.clone(),
            args.clone(),
            t == Ty::F32,
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Bool | Ty::Char => {
            lower_small_method(cx, recv.clone(), &args, &t, &name, &ret, line, out)
        }
        Ty::Str => lower_str_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            args.clone(),
            &arg_asts,
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Option(_) => lower_option_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            args.clone(),
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Result(..) => lower_result_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            args.clone(),
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Vec(elem) => lower_vec_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            elem,
            args.clone(),
            &arg_asts,
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Map { ordered, k, v } => lower_map_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            *ordered,
            k,
            v,
            args.clone(),
            &arg_asts,
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Set { ordered, t: elem } => lower_set_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            *ordered,
            elem,
            args.clone(),
            &arg_asts,
            &name,
            line,
            out,
        ),
        Ty::Deque(elem) => lower_deque_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            elem,
            args.clone(),
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Heap(_) => lower_heap_method(
            cx,
            m,
            &recv_ast,
            recv.clone(),
            args.clone(),
            &name,
            &ret,
            line,
            out,
        ),
        Ty::Ordering => lower_ordering_method(cx, recv.clone(), args.clone(), &name, line, out),
        _ => None,
    };
    if let Some(e) = found {
        return e;
    }

    // Generic fallbacks.
    if name == "clone" {
        return clone_expr(recv, &t);
    }
    if name == "to_string"
        && let Some(e) = super::fmt::std_error_display(cx, recv.clone(), recv_ty)
    {
        return e;
    }
    if name == "into" || name == "try_into" || name == "from" {
        let target = match ret.strip_refs() {
            Ty::Result(ok, _) if name == "try_into" => (**ok).clone(),
            other => other.clone(),
        };
        if let Ty::Adt { java, .. } = target.strip_refs() {
            // The user's own `From`/`TryFrom` impl shows up as a static method on the target.
            let method = if name == "try_into" {
                "try_from"
            } else {
                "from"
            };
            return Expr::static_call(format!("{java}.{method}"), vec![recv]);
        }
        if matches!(t, Ty::Adt { .. }) {
            return cx.unsupported(m.syntax(), "`into()` from a user type");
        }
        return clone_expr(recv, &ret);
    }
    // `Ord` provided methods on user types: Rust's max keeps `other` on ties, min keeps `self`.
    if matches!(recv_ty.strip_refs(), Ty::Adt { .. })
        && matches!(name.as_str(), "max" | "min" | "clamp")
    {
        let a = simple(cx, recv, recv_ty, line, out);
        let mut args = args.into_iter().map(|b| simple(cx, b, recv_ty, line, out));
        let (Some(b), c) = (args.next(), args.next()) else {
            return cx.unsupported(m.syntax(), &format!("std method `{key}`"));
        };
        let cmp = |x: &Expr, y: &Expr| Expr::call(x.clone(), "compareTo", vec![y.clone()]);
        return match (name.as_str(), c) {
            ("max", _) => conditional(Expr::bin(cmp(&a, &b), ">", Expr::raw("0")), a, b),
            ("min", _) => conditional(Expr::bin(cmp(&a, &b), "<=", Expr::raw("0")), a, b),
            (_, Some(hi)) => conditional(
                Expr::bin(cmp(&a, &b), "<", Expr::raw("0")),
                b,
                conditional(Expr::bin(cmp(&a, &hi), ">", Expr::raw("0")), hi, a),
            ),
            _ => cx.unsupported(m.syntax(), "clamp"),
        };
    }
    match (recv_ty.strip_refs(), name.as_str()) {
        (t, "to_string") if t.is_str() => recv,
        (t, "to_string") if t.is_primitive() => {
            Expr::call(Expr::raw("String"), "valueOf", vec![recv])
        }
        (Ty::Adt { .. } | Ty::Param(_), "to_string") => Expr::call(recv, "toString", vec![]),
        (Ty::Adt { .. }, "eq") => Expr::call(recv, "equals", args),
        (Ty::Adt { .. }, "ne") => Expr::Unary("!", Box::new(Expr::call(recv, "equals", args))),
        (Ty::Option(_) | Ty::Result(..), "to_string") => Expr::call(recv, "toString", vec![]),
        _ => cx.unsupported(m.syntax(), &format!("std method `{key}` on {recv_ty:?}")),
    }
}
