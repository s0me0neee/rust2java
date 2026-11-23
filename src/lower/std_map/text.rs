// Std String/&str, char, bool.

use ra_ap_syntax::ast::RangeItem;

use super::*;
use crate::lower::strip_parens;

// Hook for std free functions (`key` as built by `calls::std_fn_key`, e.g. `Rc::new`).
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    _ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    match key {
        // `String::from(x)` / `str::to_string(x)` in call form: strings stay as they are.
        k if k == "String::from" || k.ends_with("::to_string") => {
            let first = c.arg_list()?.args().next()?;
            let ty = cx.ty(&first);
            let arg = expr::lower_expr(cx, &first, out);
            Some(if ty.is_str() {
                arg
            } else {
                Expr::call(Expr::raw("String"), "valueOf", vec![arg])
            })
        }
        // Bytes (a `List<Integer>` of char codes, see `as_bytes`) back to text; ASCII is valid
        // UTF-8, so the `Result` is always `Ok`.
        "converts::from_utf8" | "String::from_utf8" | "String::from_utf8_lossy" => {
            let bytes = expr::call_args(cx, c, out).into_iter().next()?;
            let b = cx.fresh_param("b");
            cx.release_param(&b);
            let text = Expr::raw(format!(
                "{}.stream().map({b} -> String.valueOf((char) (int) {b})).collect(Collectors.joining())",
                crate::emit::render_expr(&bytes)
            ));
            // `.unwrap()` on a Result with an opaque std error is already dropped.
            let unwrapped = c
                .syntax()
                .parent()
                .and_then(ast::MethodCallExpr::cast)
                .is_some_and(|p| {
                    matches!(
                        p.name_ref().map(|n| n.text().to_string()).as_deref(),
                        Some("unwrap" | "expect")
                    )
                });
            if key.ends_with("_lossy") || unwrapped {
                return Some(text);
            }
            cx.needs_result = true;
            Some(Expr::New {
                ty: "Ok<>".into(),
                args: vec![text],
            })
        }
        // `char::from(u8)`: the argument may be a boxed `Integer` when iterating a list.
        "char::from" => {
            let args = expr::call_args(cx, c, out);
            let b = args.into_iter().next()?;
            let code = match &b {
                // A literal is already a plain int; a bound name may be a boxed `Integer`.
                Expr::Raw(_) => b,
                _ => Expr::Cast("int".into(), Box::new(b)),
            };
            Some(Expr::Cast("char".into(), Box::new(code)))
        }
        _ => None,
    }
}

// Std functions used as values (`map(String::from)`, `split(char::is_whitespace)`).
pub(super) fn path(cx: &mut Cx, _path: &ast::Path, def: ModuleDef) -> Option<Expr> {
    let ModuleDef::Function(f) = def else {
        return None;
    };
    let key = crate::lower::calls::std_fn_key(cx, f);
    Some(match key.as_str() {
        // `String::valueOf` covers `String::from`/`to_string` (a blanket impl, hence `?::`).
        k if k == "String::from" || k.ends_with("::to_string") || k.ends_with("::to_owned") => {
            Expr::raw("String::valueOf")
        }
        "char::from" => {
            let b = cx.fresh("b");
            Expr::Lambda {
                params: vec![b.clone()],
                body: LambdaBody::Expr(Box::new(Expr::Cast(
                    "char".into(),
                    Box::new(Expr::Cast("int".into(), Box::new(Expr::name(b)))),
                ))),
            }
        }
        "str::trim" => Expr::raw("String::strip"),
        "str::trim_start" => Expr::raw("String::stripLeading"),
        "str::trim_end" => Expr::raw("String::stripTrailing"),
        "str::len" | "String::len" => Expr::raw("String::length"),
        "Vec::len" => Expr::raw("List::size"),
        "Vec::is_empty" => Expr::raw("List::isEmpty"),
        "str::is_empty" | "String::is_empty" => Expr::raw("String::isEmpty"),
        "str::to_uppercase"
        | "str::to_ascii_uppercase"
        | "str::to_lowercase"
        | "str::to_ascii_lowercase" => {
            let s = cx.fresh_param("s");
            cx.release_param(&s);
            let f = if key.contains("upper") {
                "toUpperCase"
            } else {
                "toLowerCase"
            };
            Expr::Lambda {
                params: vec![s.clone()],
                body: LambdaBody::Expr(Box::new(Expr::call(
                    Expr::name(s),
                    f,
                    vec![Expr::raw("Locale.ROOT")],
                ))),
            }
        }
        "String::as_str" | "str::as_ref" | "String::as_ref" => Expr::raw("String::valueOf"),
        k if k.starts_with("char::is_") => {
            let c = cx.fresh("c");
            let body = lower_small_method(
                cx,
                Expr::name(c.clone()),
                &[],
                &Ty::Char,
                &k["char::".len()..],
                &Ty::Bool,
                1,
                &mut Vec::new(),
            )?;
            Expr::Lambda {
                params: vec![c],
                body: LambdaBody::Expr(Box::new(body)),
            }
        }
        _ => return None,
    })
}

// Hook for std methods, before the receiver is lowered (`key` like `RefCell::borrow`).
// Methods of a suffix view (see `lower::views`), answered on the base string from the offset.
fn view_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    view: &crate::lower::views::View,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let (base, start) = (Expr::name(&view.0), Expr::name(&view.1));
    let length = || Expr::call(Expr::name(&view.0), "length", vec![]);
    let line = cx.line(m.syntax());
    let arg_ast = arg_asts(m).into_iter().next();
    let arg_ty = arg_ast.as_ref().map(|a| cx.ty(a));
    match name {
        "len" => Some(Expr::bin(length(), "-", start)),
        "is_empty" => Some(Expr::bin(start, ">=", length())),
        "starts_with" => {
            let text = match arg_ty?.strip_refs() {
                Ty::Str => expr::lower_expr(cx, &arg_ast?, out),
                Ty::Char => Expr::call(
                    Expr::raw("String"),
                    "valueOf",
                    vec![expr::lower_expr(cx, &arg_ast?, out)],
                ),
                _ => return None,
            };
            Some(Expr::call(base, "startsWith", vec![text, start]))
        }
        "find" | "rfind" => {
            let s = &view.0;
            let o = &view.1;
            let idx = match arg_ty?.strip_refs() {
                Ty::Str | Ty::Char => {
                    let p = expr::lower_expr(cx, &arg_ast?, out);
                    if name == "find" {
                        Expr::call(base, "indexOf", vec![p, start.clone()])
                    } else {
                        Expr::call(base, "lastIndexOf", vec![p])
                    }
                }
                _ => {
                    if cx.lambda_depth > 0 {
                        return None;
                    }
                    let f = expr::lower_expr(cx, &arg_ast?, out);
                    let pred = push_local(
                        cx,
                        line,
                        "Function<Character, Boolean>".into(),
                        Some(f),
                        out,
                        "pred",
                    );
                    let i = cx.fresh("i");
                    let range = if name == "find" {
                        format!("IntStream.range({o}, {s}.length())")
                    } else {
                        format!(
                            "IntStream.iterate({s}.length() - 1, {i} -> {i} >= {o}, {i} -> {i} - 1)"
                        )
                    };
                    Expr::raw(format!(
                        "{range}.filter({i} -> {pred}.apply({s}.charAt({i}))).findFirst().orElse(-1)"
                    ))
                }
            };
            let n = push_local(cx, line, "int".into(), Some(idx), out, "idx");
            let ret = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
            Some(conditional(
                Expr::bin(Expr::name(&n), ">=", Expr::name(o)),
                opt_of(Expr::bin(Expr::name(&n), "-", Expr::name(o))),
                opt_empty(opt_inner(&ret).as_ref()),
            ))
        }
        _ => None,
    }
}

pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let name = key.rsplit("::").next().unwrap_or(key);
    if key.starts_with("Debug")
        && let Some(e) = crate::lower::fmt::lower_debug_builder(cx, m, out)
    {
        return Some(e);
    }
    // `c.to_uppercase().collect::<String>()` / `.to_string()`: the case-mapped text directly.
    if matches!(name, "collect" | "to_string")
        && let Some(ast::Expr::MethodCallExpr(inner)) = m.receiver().map(|r| strip_parens(&r))
        && matches!(
            method_name(&inner).as_str(),
            "to_uppercase" | "to_lowercase"
        )
        && let Some(c) = inner.receiver()
        && cx.ty(&c).strip_refs() == &Ty::Char
        && cx.ty(&ast::Expr::MethodCallExpr(m.clone())).is_str()
    {
        let f = if method_name(&inner) == "to_uppercase" {
            "toUpperCase"
        } else {
            "toLowerCase"
        };
        let c = expr::lower_expr(cx, &c, out);
        return Some(Expr::call(
            Expr::call(Expr::raw("String"), "valueOf", vec![c]),
            f,
            vec![Expr::raw("Locale.ROOT")],
        ));
    }
    // The Formatter is the `StringBuilder` a Display impl writes into; like `write!`, the
    // `fmt::Result` is dropped.
    if matches!(key, "Formatter::write_str" | "Formatter::write_char") && cx.in_display {
        let f = expr::lower_expr(cx, &m.receiver()?, out);
        let arg = expr::lower_expr(cx, &arg_asts(m).into_iter().next()?, out);
        let line = cx.line(m.syntax());
        out.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::call(f, "append", vec![arg])),
        ));
        return Some(Expr::raw("null"));
    }
    if let Some(view) = m.receiver().and_then(|r| cx.view_of(&r))
        && let Some(e) = view_method(cx, m, name, &view, out)
    {
        return Some(e);
    }
    // `s.as_bytes().len()` without copying the bytes into a list first.
    if matches!(name, "len" | "is_empty")
        && let Some(s) = m.receiver().and_then(|r| expr::as_bytes_receiver(cx, &r))
    {
        let s = expr::lower_expr(cx, &s, out);
        return Some(Expr::call(
            s,
            if name == "len" { "length" } else { "isEmpty" },
            vec![],
        ));
    }
    if !matches!(name, "unwrap" | "expect") || !matches!(recv_ty.strip_refs(), Ty::Option(_)) {
        return None;
    }
    let ast::Expr::MethodCallExpr(inner) = m.receiver()? else {
        return None;
    };
    if method_name(&inner) != "to_digit" || !matches!(cx.ty(&inner.receiver()?), Ty::Char) {
        return None;
    }
    let line = cx.line(m.syntax());
    let c = expr::lower_expr(cx, &inner.receiver()?, out);
    let radix = match inner.arg_list().and_then(|al| al.args().next()) {
        Some(a) => expr::lower_expr(cx, &a, out),
        None => Expr::raw("10"),
    };
    if name == "expect"
        && let Some(arg) = m.arg_list().and_then(|al| al.args().next())
    {
        // The message is always evaluated in Rust, so keep it; only the panic text is Java's.
        let msg = expr::lower_expr(cx, &arg, out);
        if !matches!(&msg, Expr::Raw(s) if s.starts_with('"')) {
            push_local(cx, line, "String".into(), Some(msg), out, "msg");
        }
    }
    let d = cx.fresh("digit");
    push_raw(
        out,
        line,
        format!(
            "int {d} = Character.digit({}, {}); if ({d} < 0) throw new NumberFormatException();",
            render(&c),
            render(&radix)
        ),
    );
    Some(Expr::name(d))
}

// ---- bool / char -----------------------------------------------------------------------------

pub(super) fn lower_small_method(
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
            // `c.then_some(v)` / `c.then(|| v)`: Some only when the condition holds.
            "then_some" | "then" => {
                let value = if name == "then" {
                    supplied(b0, ret)
                } else {
                    b0
                };
                let empty = match ret.strip_refs() {
                    Ty::Option(t) => opt_empty(Some(t)),
                    _ => opt_empty(None),
                };
                Expr::Ternary(Box::new(recv), Box::new(opt_of(value)), Box::new(empty))
            }
            _ => return None,
        });
    }
    let c = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Character"), f, a);
    Some(match name {
        "is_alphabetic" | "is_uppercase" | "is_lowercase" => c(
            if name == "is_alphabetic" {
                "isLetter"
            } else if name == "is_uppercase" {
                "isUpperCase"
            } else {
                "isLowerCase"
            },
            vec![recv],
        ),
        "is_numeric" => c("isDigit", vec![recv]),
        "is_alphanumeric" => c("isLetterOrDigit", vec![recv]),
        "is_whitespace" => c("isWhitespace", vec![recv]),
        "is_control" => c("isISOControl", vec![recv]),
        "is_ascii" => Expr::bin(recv, "<", Expr::raw("128")),
        // Simple mode assumes ASCII text, where the `Character` tests agree with Rust's.
        "is_ascii_digit" | "is_ascii_alphabetic" | "is_ascii_alphanumeric" | "is_ascii_whitespace"
        | "is_ascii_uppercase" | "is_ascii_lowercase"
            if crate::mode::simple() =>
        {
            let f = match name {
                "is_ascii_digit" => "isDigit",
                "is_ascii_alphabetic" => "isLetter",
                "is_ascii_alphanumeric" => "isLetterOrDigit",
                "is_ascii_whitespace" => "isWhitespace",
                "is_ascii_uppercase" => "isUpperCase",
                _ => "isLowerCase",
            };
            c(f, vec![recv])
        }
        "is_ascii_digit" => Expr::bin(
            Expr::bin(recv.clone(), ">=", Expr::raw("'0'")),
            "&&",
            Expr::bin(recv, "<=", Expr::raw("'9'")),
        ),
        "is_ascii_alphabetic" => {
            let lower = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'a'")),
                "&&",
                Expr::bin(recv.clone(), "<=", Expr::raw("'z'")),
            );
            let upper = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'A'")),
                "&&",
                Expr::bin(recv, "<=", Expr::raw("'Z'")),
            );
            Expr::bin(lower, "||", upper)
        }
        "is_ascii_alphanumeric" => {
            let digit = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'0'")),
                "&&",
                Expr::bin(recv.clone(), "<=", Expr::raw("'9'")),
            );
            let lower = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'a'")),
                "&&",
                Expr::bin(recv.clone(), "<=", Expr::raw("'z'")),
            );
            let upper = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'A'")),
                "&&",
                Expr::bin(recv, "<=", Expr::raw("'Z'")),
            );
            Expr::bin(Expr::bin(digit, "||", lower), "||", upper)
        }
        // Rust's set is space, \t, \n, \x0C, \r (no vertical tab).
        "is_ascii_whitespace" => Expr::bin(
            Expr::call(Expr::str_lit(" \t\n\x0C\r"), "indexOf", vec![recv]),
            ">=",
            Expr::raw("0"),
        ),
        "is_ascii_uppercase" => Expr::bin(
            Expr::bin(recv.clone(), ">=", Expr::raw("'A'")),
            "&&",
            Expr::bin(recv, "<=", Expr::raw("'Z'")),
        ),
        "is_ascii_lowercase" => Expr::bin(
            Expr::bin(recv.clone(), ">=", Expr::raw("'a'")),
            "&&",
            Expr::bin(recv, "<=", Expr::raw("'z'")),
        ),
        "is_ascii_punctuation" => {
            let range = |a: &str, b: &str| {
                Expr::bin(
                    Expr::bin(recv.clone(), ">=", Expr::raw(a)),
                    "&&",
                    Expr::bin(recv.clone(), "<=", Expr::raw(b)),
                )
            };
            let mut e = range("'!'", "'/'");
            for (a, b) in [("':'", "'@'"), ("'['", "'`'"), ("'{'", "'~'")] {
                e = Expr::bin(e, "||", range(a, b));
            }
            e
        }
        "is_ascii_hexdigit" => {
            let d = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'0'")),
                "&&",
                Expr::bin(recv.clone(), "<=", Expr::raw("'9'")),
            );
            let l = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'a'")),
                "&&",
                Expr::bin(recv.clone(), "<=", Expr::raw("'f'")),
            );
            let u = Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'A'")),
                "&&",
                Expr::bin(recv, "<=", Expr::raw("'F'")),
            );
            Expr::bin(Expr::bin(d, "||", l), "||", u)
        }
        "is_digit" => Expr::bin(
            Expr::call(Expr::raw("Character"), "digit", vec![recv, b0]),
            ">=",
            Expr::raw("0"),
        ),
        "to_digit" => {
            let d = Expr::call(Expr::raw("Character"), "digit", vec![recv, b0]);
            let n = cx.fresh("digit");
            push_raw(out, line, format!("int {n} = {};", render(&d)));
            conditional(
                Expr::bin(Expr::name(n.clone()), ">=", Expr::raw("0")),
                opt_of(Expr::name(n.clone())),
                opt_empty(opt_inner(ret).as_ref()),
            )
        }
        "to_ascii_uppercase" | "to_uppercase" => c("toUpperCase", vec![recv]),
        "to_ascii_lowercase" | "to_lowercase" => c("toLowerCase", vec![recv]),
        "to_string" => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        "eq_ignore_ascii_case" => Expr::bin(
            c("toLowerCase", vec![recv]),
            "==",
            c("toLowerCase", vec![b0]),
        ),
        "cmp" => Expr::call(Expr::raw("Character"), "compare", vec![recv, b0]),
        "partial_cmp" => opt_of(Expr::call(
            Expr::raw("Character"),
            "compare",
            vec![recv, b0],
        )),
        "clone" | "to_owned" => recv,
        "into" if ret.is_str() => Expr::call(Expr::raw("String"), "valueOf", vec![recv]),
        _ => return None,
    })
}

// ---- strings ---------------------------------------------------------------------------------

pub(super) fn lower_str_method(
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
    let arg0_ty = arg_asts
        .first()
        .map(|a| cx.ty(a))
        .unwrap_or_else(|| Ty::Str);
    // A `StringBuilder` string answers these itself; anything else reads `s.toString()`.
    if let Some(b) = cx.builder_path(recv_ast) {
        let b = Expr::name(b);
        match name {
            "len" => return Some(Expr::call(b, "length", vec![])),
            "is_empty" => return Some(Expr::call(b, "isEmpty", vec![])),
            "push" | "push_str" => {
                return Some(Expr::call(
                    b,
                    "append",
                    vec![crate::lower::fmt::as_any_value(b0)],
                ));
            }
            "clear" => return Some(Expr::call(b, "setLength", vec![Expr::raw("0")])),
            // As statements only (`builders` checks): the popped char isn't needed.
            "pop" => {
                let n = render(&b);
                return Some(Expr::call(b, "setLength", vec![Expr::raw(format!("Math.max(0, {n}.length() - 1)"))]));
            }
            "truncate" => {
                let n = render(&b);
                let k = render(&b0);
                return Some(Expr::call(b, "setLength", vec![Expr::raw(format!("Math.min({k}, {n}.length())"))]));
            }
            _ => {}
        }
    }
    match name {
        "len" => return Some(Expr::call(recv, "length", vec![])),
        "is_empty" => return Some(Expr::call(recv, "isEmpty", vec![])),
        "push_str" | "push" => {
            let _ = recv;
            return Some(expr::place_assign(cx, recv_ast, "+=", b0, out));
        }
        "clear" => {
            return Some(expr::place_assign(
                cx,
                recv_ast,
                "=",
                Expr::str_lit(""),
                out,
            ));
        }
        "insert" | "insert_str" => {
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
            push_raw(
                out,
                line,
                format!("if ({n}.isPresent()) {ps} = {ps}.substring(0, {ps}.length() - 1);"),
            );
            return Some(Expr::name(n));
        }
        "extend" => {
            let stream = match arg0_ty.strip_refs() {
                Ty::Iter(_) => b0,
                Ty::Vec(_) | Ty::Deque(_) | Ty::Set { .. } => Expr::call(b0, "stream", vec![]),
                _ => return None,
            };
            let item = match arg0_ty.strip_refs() {
                Ty::Iter(t) | Ty::Vec(t) | Ty::Deque(t) | Ty::Set { t, .. } => (**t).clone(),
                _ => return None,
            };
            let stream = if matches!(item.strip_refs(), Ty::Char) {
                Expr::call(stream, "map", vec![Expr::raw("String::valueOf")])
            } else {
                stream
            };
            let text = Expr::call(stream, "collect", vec![Expr::raw("Collectors.joining()")]);
            return Some(expr::place_assign(cx, recv_ast, "+=", text, out));
        }
        "replace_range" => {
            let Some(ast::Expr::RangeExpr(range)) = arg_asts.first().map(strip_parens) else {
                return None;
            };
            let p = expr::lower_expr(cx, recv_ast, out);
            let p = simple(cx, p, &Ty::Str, line, out);
            let start = match range.start() {
                Some(s) => expr::lower_expr(cx, &s, out),
                None => Expr::raw("0"),
            };
            let end = match range.end() {
                Some(e) => {
                    let e = expr::lower_expr(cx, &e, out);
                    if range.op_kind() == Some(ast::RangeOp::Inclusive) {
                        Expr::bin(e, "+", Expr::raw("1"))
                    } else {
                        e
                    }
                }
                None => Expr::call(p.clone(), "length", vec![]),
            };
            let pre = Expr::call(p.clone(), "substring", vec![Expr::raw("0"), start]);
            let post = Expr::call(p, "substring", vec![end]);
            let v = Expr::bin(Expr::bin(pre, "+", b1), "+", post);
            return Some(expr::place_assign(cx, recv_ast, "=", v, out));
        }
        "truncate" => {
            let p = expr::lower_expr(cx, recv_ast, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (ps, ns) = (render(&p), render(&n));
            push_raw(
                out,
                line,
                format!("if ({ps}.length() > {ns}) {ps} = {ps}.substring(0, {ns});"),
            );
            return Some(Expr::raw("null"));
        }
        // A `char` predicate (closure or `char::is_*`) tested on all chars, the first or the last.
        "contains" | "starts_with" | "ends_with"
            if matches!(arg0_ty.strip_refs(), Ty::Fn { .. })
                || _m
                    .arg_list()
                    .and_then(|a| a.args().next())
                    .is_some_and(|a| {
                        matches!(a, ast::Expr::ClosureExpr(_))
                            || (matches!(a, ast::Expr::PathExpr(_))
                                && !matches!(arg0_ty.strip_refs(), Ty::Char | Ty::Str))
                    }) =>
        {
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let c = cx.fresh_param("c");
            let chars = Expr::call(sv.clone(), "chars", vec![]);
            let chars = match name {
                "contains" => chars,
                "starts_with" => Expr::call(chars, "limit", vec![Expr::raw("1")]),
                _ => Expr::call(
                    chars,
                    "skip",
                    vec![Expr::raw(format!(
                        "Math.max(0, {}.length() - 1)",
                        render(&sv)
                    ))],
                ),
            };
            let boxed = Expr::call(
                chars,
                "mapToObj",
                vec![Expr::raw(format!("{c} -> (char) {c}"))],
            );
            cx.release_param(&c);
            return Some(Expr::call(boxed, "anyMatch", vec![b0]));
        }
        "contains" => {
            if matches!(arg0_ty.strip_refs(), Ty::Char) {
                // No `contains(char)` in Java; `indexOf(int)` takes the char directly.
                return Some(Expr::bin(
                    Expr::call(recv, "indexOf", vec![b0]),
                    ">=",
                    Expr::raw("0"),
                ));
            }
            return Some(Expr::call(recv, "contains", vec![b0]));
        }
        "starts_with" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) {
                Expr::call(Expr::raw("String"), "valueOf", vec![b0])
            } else {
                b0
            };
            return Some(Expr::call(recv, "startsWith", vec![arg]));
        }
        "ends_with" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) {
                Expr::call(Expr::raw("String"), "valueOf", vec![b0])
            } else {
                b0
            };
            return Some(Expr::call(recv, "endsWith", vec![arg]));
        }
        "find" | "rfind" => {
            let getter = if name == "find" {
                "indexOf"
            } else {
                "lastIndexOf"
            };
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let idx = if matches!(arg0_ty.strip_refs(), Ty::Char | Ty::Str) {
                let av = simple(cx, b0, &arg0_ty, line, out);
                Expr::call(sv, getter, vec![av])
            } else {
                // A `char` predicate (closure or `char::is_*`): scan the indices.
                if cx.lambda_depth > 0 {
                    return Some(cx.unsupported(
                        _m.syntax(),
                        "`str::find` with a predicate inside a closure",
                    ));
                }
                let pred = push_local(
                    cx,
                    line,
                    "Function<Character, Boolean>".into(),
                    Some(b0),
                    out,
                    "pred",
                );
                let s = render(&sv);
                let i = cx.fresh("i");
                let range = if name == "find" {
                    format!("IntStream.range(0, {s}.length())")
                } else {
                    format!("IntStream.iterate({s}.length() - 1, {i} -> {i} >= 0, {i} -> {i} - 1)")
                };
                Expr::raw(format!(
                    "{range}.filter({i} -> {pred}.apply({s}.charAt({i}))).findFirst().orElse(-1)"
                ))
            };
            let n = push_local(cx, line, "int".into(), Some(idx), out, "idx");
            return Some(conditional(
                Expr::bin(Expr::name(n.clone()), ">=", Expr::raw("0")),
                opt_of(Expr::name(n.clone())),
                opt_empty(opt_inner(ret).as_ref()),
            ));
        }
        // Both halves of the pair are cut from the same original string.
        "split_once" | "rsplit_once" => {
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let pat = simple(cx, b0, &arg0_ty, line, out);
            let plen = pattern_len(&pat, &arg0_ty);
            let getter = if name == "split_once" {
                "indexOf"
            } else {
                "lastIndexOf"
            };
            let at = push_local(
                cx,
                line,
                "int".into(),
                Some(Expr::call(sv.clone(), getter, vec![pat])),
                out,
                "at",
            );
            let head = Expr::call(
                sv.clone(),
                "substring",
                vec![Expr::raw("0"), Expr::name(at.clone())],
            );
            let tail = Expr::call(
                sv,
                "substring",
                vec![Expr::bin(Expr::name(at.clone()), "+", plen)],
            );
            return Some(conditional(
                Expr::bin(Expr::name(at), ">=", Expr::raw("0")),
                opt_of(Expr::call(Expr::raw("Map"), "entry", vec![head, tail])),
                opt_empty(opt_inner(ret).as_ref()),
            ));
        }
        "replace" => {
            return Some(Expr::call(
                recv,
                "replace",
                vec![char_arg(b0, &arg0_ty), b1],
            ));
        }
        // Java replaces every occurrence; count them with indexOf and the growing result.
        // An empty pattern inserts at every boundary, which indexOf cannot walk.
        "replacen" if empty_str_pat(arg_asts) => {
            return Some(cx.unsupported(_m.syntax(), "replacen with an empty pattern"));
        }
        "replacen" => {
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let pat = simple(cx, b0, &arg0_ty, line, out);
            let rep = simple(cx, b1, &Ty::Str, line, out);
            let n = simple(
                cx,
                args.get(2).cloned().unwrap_or_else(|| Expr::raw("1")),
                &Ty::Int(Int::Usize),
                line,
                out,
            );
            let (ps, ms, ns) = (render(&pat), render(&rep), render(&n));
            let r = cx.fresh("replaced");
            let k = cx.fresh("k");
            let from = cx.fresh("from");
            let at = cx.fresh("at");
            let plen = render(&pattern_len(&pat, &arg0_ty));
            push_raw(
                out,
                line,
                format!(
                    "String {r} = {}; for (int {k} = 0, {from} = 0; {k} < {ns}; {k}++) {{ int {at} = {r}.indexOf({ps}, {from}); \
                     if ({at} < 0) break; {r} = {r}.substring(0, {at}) + {ms} + {r}.substring({at} + {plen}); {from} = {at} + {ms}.length(); }}",
                    render(&sv)
                ),
            );
            return Some(Expr::name(r));
        }
        "trim" => return Some(Expr::call(recv, "strip", vec![])),
        // A literal char/str pattern: one `replaceAll`, the regex escaped here.
        // A char predicate: move the two ends inward while it holds.
        "trim_matches" | "trim_start_matches" | "trim_end_matches"
            if arg_asts.first().is_some_and(|a| {
                matches!(strip_parens(a), ast::Expr::ClosureExpr(_))
                    || matches!(cx.ty(a).strip_refs(), Ty::Fn { .. })
            }) =>
        {
            if cx.lambda_depth > 0 {
                return Some(cx.unsupported(
                    _m.syntax(),
                    "`trim_matches` with a predicate inside a closure",
                ));
            }
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let s = render(&sv);
            let pred = push_local(
                cx,
                line,
                "Function<Character, Boolean>".into(),
                Some(b0),
                out,
                "pred",
            );
            let (start, end) = (cx.fresh("start"), cx.fresh("end"));
            let mut code = format!("int {start} = 0, {end} = {s}.length();");
            if name != "trim_end_matches" {
                code += &format!(
                    " while ({start} < {end} && {pred}.apply({s}.charAt({start}))) {start}++;"
                );
            }
            if name != "trim_start_matches" {
                code += &format!(
                    " while ({end} > {start} && {pred}.apply({s}.charAt({end} - 1))) {end}--;"
                );
            }
            push_raw(out, line, code);
            return Some(Expr::call(
                sv,
                "substring",
                vec![Expr::name(start), Expr::name(end)],
            ));
        }
        "trim_matches" | "trim_start_matches" | "trim_end_matches" => {
            let pat = match arg_asts.first() {
                Some(ast::Expr::Literal(l)) => match l.kind() {
                    ast::LiteralKind::Char(c) => c.value().ok().map(String::from),
                    ast::LiteralKind::String(s) => s.value().ok().map(|v| v.to_string()),
                    _ => None,
                },
                _ => None,
            }?;
            if pat.is_empty() {
                return Some(recv);
            }
            let esc: String = pat
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' {
                        c.to_string()
                    } else {
                        format!("\\{c}")
                    }
                })
                .collect();
            let rep = if pat.chars().count() == 1 {
                format!("{esc}+")
            } else {
                format!("(?:{esc})+")
            };
            let regex = match name {
                "trim_matches" => format!("^{rep}|{rep}$"),
                "trim_start_matches" => format!("^{rep}"),
                _ => format!("{rep}$"),
            };
            let lit = Expr::raw(crate::jir::java_string_literal(&regex));
            return Some(Expr::call(recv, "replaceAll", vec![lit, Expr::str_lit("")]));
        }
        "trim_start" => return Some(Expr::call(recv, "stripLeading", vec![])),
        "trim_end" => return Some(Expr::call(recv, "stripTrailing", vec![])),
        "to_uppercase" | "to_ascii_uppercase" => {
            return Some(Expr::call(
                recv,
                "toUpperCase",
                vec![Expr::raw("Locale.ROOT")],
            ));
        }
        "to_lowercase" | "to_ascii_lowercase" => {
            return Some(Expr::call(
                recv,
                "toLowerCase",
                vec![Expr::raw("Locale.ROOT")],
            ));
        }
        "repeat" => return Some(Expr::call(recv, "repeat", vec![b0])),
        "eq_ignore_ascii_case" => return Some(Expr::call(recv, "equalsIgnoreCase", vec![b0])),
        "cmp" => return Some(Expr::call(recv, "compareTo", vec![b0])),
        // Rust yields UTF-8 bytes; Java's char codes are the same values for ASCII.
        "as_bytes" => {
            let codes = Expr::call(recv, "chars", vec![]);
            return Some(Expr::call(
                Expr::call(codes, "boxed", vec![]),
                "toList",
                vec![],
            ));
        }
        "as_str" | "as_ref" | "to_owned" | "to_string" | "clone" | "into" => return Some(recv),
        "parse" => return Some(parse_expr(cx, _m, recv, ret, line, out)),
        "get" => {
            if let Some(ast::Expr::RangeExpr(r)) = arg_asts.first() {
                return Some(str_get_range(cx, recv, r, ret, line, out));
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
            let test = if name == "strip_prefix" {
                "startsWith"
            } else {
                "endsWith"
            };
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let av = simple(cx, b0, &arg0_ty, line, out);
            let plen = pattern_len(&av, &arg0_ty);
            let av = char_arg(av, &arg0_ty);
            let inner = if name == "strip_prefix" {
                Expr::call(sv.clone(), "substring", vec![plen])
            } else {
                Expr::call(
                    sv.clone(),
                    "substring",
                    vec![
                        Expr::raw("0"),
                        Expr::bin(Expr::call(sv.clone(), "length", vec![]), "-", plen),
                    ],
                )
            };
            return Some(conditional(
                Expr::call(sv, test, vec![av]),
                opt_of(inner),
                opt_empty(opt_inner(ret).as_ref()),
            ));
        }
        _ => {}
    }
    None
}

fn empty_str_pat(arg_asts: &[ast::Expr]) -> bool {
    matches!(
        arg_asts.first().map(strip_parens),
        Some(ast::Expr::Literal(l)) if matches!(
            l.kind(),
            ast::LiteralKind::String(s) if s.value().is_ok_and(|v| v.is_empty())
        )
    )
}

// Java's String methods that take a character pattern need a String in the mixed overloads.
fn char_arg(v: Expr, ty: &Ty) -> Expr {
    if matches!(ty.strip_refs(), Ty::Char) {
        Expr::call(Expr::raw("String"), "valueOf", vec![v])
    } else {
        v
    }
}

// Java expression for the pattern's length in chars: a char pattern is one long.
fn pattern_len(pat: &Expr, ty: &Ty) -> Expr {
    if matches!(ty.strip_refs(), Ty::Char) {
        Expr::raw("1")
    } else {
        Expr::call(pat.clone(), "length", vec![])
    }
}

// `s.get(a..b)` is `None` outside the string, where `substring` would throw.
fn str_get_range(
    cx: &mut Cx,
    recv: Expr,
    r: &ast::RangeExpr,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let s = simple(cx, recv, &Ty::Str, line, out);
    let from = r
        .start()
        .map(|e| expr::lower_expr(cx, &e, out))
        .unwrap_or_else(|| Expr::raw("0"));
    let from = simple(cx, from, &Ty::Int(Int::Usize), line, out);
    let (to, open) = match r.end() {
        Some(e) => {
            let e = expr::lower_expr(cx, &e, out);
            let e = if r.op_kind() == Some(ast::RangeOp::Inclusive) {
                Expr::bin(e, "+", Expr::raw("1"))
            } else {
                e
            };
            (e, false)
        }
        None => (Expr::call(s.clone(), "length", vec![]), true),
    };
    // `length()` is pure, so an open end can stay inline; anything else is evaluated once.
    let to = if open {
        to
    } else {
        simple(cx, to, &Ty::Int(Int::Usize), line, out)
    };
    let bounds = Expr::bin(
        Expr::bin(
            Expr::bin(from.clone(), ">=", Expr::raw("0")),
            "&&",
            Expr::bin(from.clone(), "<=", to.clone()),
        ),
        "&&",
        Expr::bin(to.clone(), "<=", Expr::call(s.clone(), "length", vec![])),
    );
    let sub = Expr::call(s, "substring", vec![from, to]);
    conditional(bounds, opt_of(sub), opt_empty(opt_inner(ret).as_ref()))
}

// `s.parse::<T>().unwrap()` / `T::from_str_radix(..).unwrap()`: a Java parser that throws exactly
// where Rust's parse fails, so the panic (exit 101) survives without building the Result.
pub(super) enum DirectParse {
    // `parse::<String>()` can't fail.
    Identity,
    Call(&'static str),
    // u8/u16: no exact Java parser, so parse as int and reject what's out of range.
    Bounded(u32),
}

pub(super) fn direct_parse(cx: &Cx, e: &ast::Expr) -> Option<DirectParse> {
    let Ty::Result(t, _) = cx.ty(e) else {
        return None;
    };
    // Simple mode: every parse throws on bad input, like a person would write it.
    if crate::mode::throws() {
        return match t.strip_refs() {
            // Simple mode has no unsigned types, so the signed parsers read as intended.
            Ty::Int(Int::U64) => Some(DirectParse::Call("Long.parseLong")),
            Ty::Int(i) if i.is_long() => Some(DirectParse::Call(int_parser(*i)?)),
            Ty::Int(Int::U32 | Int::Usize) => Some(DirectParse::Call("Integer.parseInt")),
            Ty::Int(i @ (Int::I8 | Int::I16)) => Some(DirectParse::Call(int_parser(*i)?)),
            Ty::Int(Int::U16) => Some(DirectParse::Bounded(65535)),
            Ty::Int(_) => Some(DirectParse::Call("Integer.parseInt")),
            Ty::F64 => Some(DirectParse::Call("Double.parseDouble")),
            Ty::F32 => Some(DirectParse::Call("Float.parseFloat")),
            Ty::Bool => Some(DirectParse::Call("Boolean.parseBoolean")),
            Ty::Str => Some(DirectParse::Identity),
            _ => None,
        };
    }
    let parent = ast::MethodCallExpr::cast(e.syntax().parent()?)?;
    if !matches!(method_name(&parent).as_str(), "unwrap" | "expect")
        || parent.receiver()?.syntax() != e.syntax()
    {
        return None;
    }
    match t.strip_refs() {
        Ty::Int(Int::U8) => Some(DirectParse::Bounded(255)),
        Ty::Int(Int::U16) => Some(DirectParse::Bounded(65535)),
        Ty::Int(i) => int_parser(*i).map(DirectParse::Call),
        Ty::Str => Some(DirectParse::Identity),
        _ => None,
    }
}

pub(super) fn lower_direct_parse(
    cx: &mut Cx,
    d: DirectParse,
    mut args: Vec<Expr>,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    match d {
        DirectParse::Identity => args.remove(0),
        DirectParse::Call(f) => Expr::static_call(f, args),
        // Simple mode skips the range check, like the other integer widths.
        DirectParse::Bounded(max) if crate::mode::simple() => {
            let n = Expr::static_call("Integer.parseInt", args);
            if max == 65535 {
                Expr::Cast("short".into(), Box::new(n))
            } else {
                n
            }
        }
        DirectParse::Bounded(max) => {
            let v = cx.fresh("num");
            let call = render(&Expr::static_call("Integer.parseUnsignedInt", args));
            push_raw(
                out,
                line,
                format!(
                    "int {v} = {call}; if ({v} > {max}) throw new NumberFormatException(\"number too large to fit in target type\");"
                ),
            );
            // u16 is a Java short; the range check above makes the narrowing exact.
            if max == 65535 {
                Expr::Cast("short".into(), Box::new(Expr::name(v)))
            } else {
                Expr::name(v)
            }
        }
    }
}

// Java parsers whose accepted inputs and range match Rust's `parse` for that type.
pub(super) fn int_parser(i: Int) -> Option<&'static str> {
    Some(match i {
        Int::I8 => "Byte.parseByte",
        Int::I16 => "Short.parseShort",
        Int::I32 => "Integer.parseInt",
        Int::I64 => "Long.parseLong",
        Int::U32 | Int::Usize => "Integer.parseUnsignedInt",
        Int::U64 => "Long.parseUnsignedLong",
        _ => return None,
    })
}

pub(super) fn parse_expr(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv: Expr,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let target = match ret.strip_refs() {
        Ty::Result(t, _) => (**t).clone(),
        _ => Ty::Other("?".into()),
    };
    // A user `FromStr` already returns the Result; the caller's unwrap/expect/? applies to it.
    if let Ty::Adt { java, .. } = target.strip_refs() {
        return Expr::static_call(format!("{java}.from_str"), vec![recv]);
    }
    if let Some(d) = direct_parse(cx, &ast::Expr::MethodCallExpr(m.clone())) {
        return lower_direct_parse(cx, d, vec![recv], line, out);
    }
    super::option::parse_result(cx, m, recv, ret, &target, line, out)
}

// The value a `FnOnce() -> T` argument produces: `|| expr` inlines to `expr`.
fn supplied(f: Expr, ret: &Ty) -> Expr {
    if let Expr::Lambda {
        params,
        body: crate::jir::LambdaBody::Expr(body),
    } = &f
        && params.is_empty()
    {
        return (**body).clone();
    }
    let t = match ret.strip_refs() {
        Ty::Option(t) => t.boxed(),
        _ => "Object".into(),
    };
    Expr::call(
        Expr::Cast(format!("Supplier<{t}>"), Box::new(f)),
        "get",
        vec![],
    )
}
