// Std Option and Result methods.

use super::*;
use crate::jir::{Arm, ArmBody};
use crate::lower::stmt;

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

fn lambda_call(lambda: Expr, ret: &Ty) -> Expr {
    // `|| expr` is the expression itself: it runs inside the caller's own lambda anyway.
    if let Expr::Lambda {
        params,
        body: LambdaBody::Expr(body),
    } = &lambda
        && params.is_empty()
    {
        return (**body).clone();
    }
    Expr::call(
        Expr::Cast(format!("Supplier<{}>", ret.boxed()), Box::new(lambda)),
        "get",
        vec![],
    )
}

fn debug_lambda(cx: &mut Cx, ty: &Ty, base: &str) -> Expr {
    let name = cx.fresh(base);
    let body = crate::lower::fmt::debug(cx, Expr::name(name.clone()), ty);
    Expr::Lambda {
        params: vec![name],
        body: LambdaBody::Expr(Box::new(body)),
    }
}

fn result_types(cx: &Cx, e: &ast::Expr) -> Option<(Ty, Ty)> {
    match cx.ty(e).strip_refs() {
        Ty::Result(ok, err) => Some(((**ok).clone(), (**err).clone())),
        _ => None,
    }
}

fn is_parse_error(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(n)
        if n.contains("ParseIntError") || n.contains("ParseFloatError"))
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
    let inner = opt_inner_from_recv(m, cx);
    if matches!(name, "unwrap" | "expect")
        && let ast::Expr::MethodCallExpr(next) = recv_ast
        && crate::lower::iter::is_unwrapped_next(cx, next)
    {
        return Some(recv);
    }
    // Simple mode: a nullable value (`m.get(k)`, `q.poll()`) is used as is; `null` is "None".
    if crate::mode::simple()
        && let Expr::Call {
            recv: Some(r),
            name: wrap,
            args: wrapped,
        } = &recv
        && matches!(r.as_ref(), Expr::Raw(c) if c == "Optional")
        && wrap == "ofNullable"
        && wrapped.len() == 1
    {
        let v = wrapped[0].clone();
        match name {
            "unwrap" | "expect" => return Some(v),
            "is_some" => return Some(Expr::bin(v, "!=", Expr::raw("null"))),
            "is_none" => return Some(Expr::bin(v, "==", Expr::raw("null"))),
            _ => {}
        }
    }
    // Simple mode: `Optional.of(e).orElseThrow()` is `e`, and so is `a.partial_cmp(&b).unwrap()`'s
    // `(nan ? Optional.empty() : Optional.of(compare))` (NaN left to Java's total order).
    if crate::mode::simple() && matches!(name, "unwrap" | "unwrap_unchecked" | "expect") {
        let of = |e: &Expr| match e {
            Expr::Call { recv: Some(o), name, args }
                if name == "of" && args.len() == 1 && matches!(&**o, Expr::Raw(r) if r == "Optional") =>
            {
                Some(args[0].clone())
            }
            _ => None,
        };
        let some = match &recv {
            Expr::Ternary(_, empty, value) if matches!(&**empty, Expr::Raw(r) if r.starts_with("Optional.")) => of(value),
            other => of(other),
        };
        if some.is_some() {
            return some;
        }
    }
    match name {
        "is_some" => Some(Expr::call(recv, "isPresent", vec![])),
        "is_none" => Some(Expr::call(recv, "isEmpty", vec![])),
        "unwrap" | "unwrap_unchecked" => Some(Expr::call(recv, "orElseThrow", vec![])),
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
            vec![default_value_cx(
                cx,
                &inner.unwrap_or(Ty::Other("?".into())),
            )],
        )),
        "map" | "and_then" | "filter" => {
            let java = if name == "and_then" { "flatMap" } else { name };
            Some(Expr::call(recv, java, vec![b0]))
        }
        "map_or" => {
            // Rust: map_or(default, f)
            let mapped = Expr::call(recv, "map", vec![b1_of(&args)]);
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
        // Simple mode: the missing value throws right away.
        "ok_or" | "ok_or_else" if crate::mode::throws() => {
            let err_ty = arg_asts(m)
                .first()
                .map(|a| crate::lower::throws::payload_ty(cx, a))
                .unwrap_or(Ty::Other("?".into()));
            let err = match (name, b0) {
                (
                    "ok_or_else",
                    Expr::Lambda {
                        params,
                        body: LambdaBody::Expr(b),
                    },
                ) if params.is_empty() => *b,
                ("ok_or_else", f) => Expr::call(f, "get", vec![]),
                (_, e) => e,
            };
            let thrown = crate::lower::throws::exception_of(cx, err, &err_ty);
            Some(Expr::call(
                recv,
                "orElseThrow",
                vec![Expr::Lambda {
                    params: vec![],
                    body: LambdaBody::Expr(Box::new(thrown)),
                }],
            ))
        }
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
            let result_ty = format!("Result<{}, {}>", ok_ty, err_ty);
            let mapped = Expr::call(
                recv,
                format!("<{result_ty}>map"),
                vec![Expr::raw(format!("{v} -> new Ok<>({v})"))],
            );
            Some(Expr::call(
                mapped,
                "orElse",
                vec![Expr::raw(format!("new Err<>({})", render(&b0)))],
            ))
        }
        "ok_or_else" => {
            cx.needs_result = true;
            let v = cx.fresh("v");
            let ok_ty = inner
                .as_ref()
                .map(|t| t.boxed())
                .unwrap_or_else(|| "Object".into());
            let err_ty = match ret.strip_refs() {
                Ty::Result(_, e) => (**e).clone(),
                _ => Ty::Other("?".into()),
            };
            let mapped = Expr::call(
                recv,
                format!("<Result<{}, {}>>map", ok_ty, err_ty.boxed()),
                vec![Expr::raw(format!("{v} -> new Ok<>({v})"))],
            );
            let fallback = lambda_call(b0, &err_ty);
            Some(Expr::call(
                mapped,
                "orElseGet",
                vec![Expr::Lambda {
                    params: vec![],
                    body: LambdaBody::Expr(Box::new(Expr::New {
                        ty: "Err<>".into(),
                        args: vec![fallback],
                    })),
                }],
            ))
        }
        "as_ref" | "as_mut" | "as_deref" | "cloned" | "copied" => Some(recv),
        // `Option<Result<T, E>>` -> `Result<Option<T>, E>`.
        "transpose" => {
            let Ty::Result(ok, err) = ret.strip_refs() else {
                return None;
            };
            cx.needs_result = true;
            // `Optional.of(new Ok<>(3))` infers `Ok<Integer, Object>`; a typed local fixes it.
            let recv = match recv {
                Expr::Name(_) => recv,
                _ if cx.lambda_depth > 0 => recv,
                _ => {
                    let Ty::Option(value) = ok.strip_refs() else {
                        return None;
                    };
                    let opt = Ty::Option(Box::new(Ty::Result(value.clone(), err.clone())));
                    let ty = stmt::decl_type_explicit(&opt);
                    Expr::name(push_local(cx, line, ty, Some(recv), out, "opt"))
                }
            };
            let r = cx.fresh("r");
            let mapped = Expr::call(
                recv,
                format!("<Result<{}, {}>>map", ok.boxed(), err.boxed()),
                vec![Expr::raw(format!("{r} -> {r}.map(Optional::of)"))],
            );
            Some(Expr::call(
                mapped,
                "orElseGet",
                vec![Expr::raw("() -> new Ok<>(Optional.empty())")],
            ))
        }
        "flatten" => {
            // `Option<Option<T>>`: `orElse` yields the contained `Option<T>`; its type parameter
            // target-types the empty argument.
            Some(Expr::call(
                recv,
                "orElse",
                vec![Expr::raw("Optional.empty()")],
            ))
        }
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
            // The target's declared type supplies the witness here.
            let assign = expr::place_assign(cx, recv_ast, "=", Expr::raw("Optional.empty()"), out);
            out.push(Stmt::new(line, StmtKind::Expr(assign)));
            Some(Expr::name(n))
        }
        "insert" => {
            let assign = expr::place_assign(cx, recv_ast, "=", opt_of(b0), out);
            out.push(Stmt::new(line, StmtKind::Expr(assign)));
            Some(Expr::call(recv, "get", vec![]))
        }
        "get_or_insert" | "get_or_insert_with" => {
            let ty = inner.clone().unwrap_or(Ty::Other("?".into()));
            let value = match (name, arg_asts(m).first()) {
                ("get_or_insert", _) => b0,
                (_, Some(a)) => {
                    super::coll::ctor_value(cx, a, &ty, out).unwrap_or_else(|| lambda_call(b0, &ty))
                }
                _ => lambda_call(b0, &ty),
            };
            // `if (o.isEmpty()) o = Optional.of(v);`, then the value is `o.get()`.
            let mut fill = Vec::new();
            let assign = expr::place_assign(cx, recv_ast, "=", opt_of(value), &mut fill);
            out.extend(fill);
            out.push(Stmt::new(
                line,
                StmtKind::Raw(format!(
                    "if ({}.isEmpty()) {};",
                    render(&recv),
                    render(&assign)
                )),
            ));
            Some(Expr::call(recv, "get", vec![]))
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
            let mapped = Expr::call(recv, "map", vec![b1_of(&args)]);
            Some(Expr::call(mapped, "orElseGet", vec![b0]))
        }
        "zip" => {
            let b = m
                .arg_list()
                .and_then(|l| l.args().next())
                .and_then(|e| match cx.ty(&e).strip_refs() {
                    Ty::Option(t) => Some((**t).clone()),
                    _ => None,
                })
                .unwrap_or(Ty::Other("?".into()));
            // Hoisted when not simple, so the argument is still evaluated eagerly like Rust's.
            let other = simple(cx, b0, &Ty::Option(Box::new(b)), line, out);
            let x = cx.fresh("x");
            let y = cx.fresh("y");
            let entry = Expr::call(
                Expr::raw("Map"),
                "entry",
                vec![Expr::name(x.clone()), Expr::name(y.clone())],
            );
            let nested = Expr::call(
                other,
                "map",
                vec![Expr::Lambda {
                    params: vec![y],
                    body: LambdaBody::Expr(Box::new(entry)),
                }],
            );
            Some(Expr::call(
                recv,
                "flatMap",
                vec![Expr::Lambda {
                    params: vec![x],
                    body: LambdaBody::Expr(Box::new(nested)),
                }],
            ))
        }
        "and" => {
            let b = match ret.strip_refs() {
                Ty::Option(t) => (**t).clone(),
                _ => Ty::Other("?".into()),
            };
            let other = simple(cx, b0, &Ty::Option(Box::new(b)), line, out);
            let x = cx.fresh("x");
            Some(Expr::call(
                recv,
                "flatMap",
                vec![Expr::Lambda {
                    params: vec![x],
                    body: LambdaBody::Expr(Box::new(other)),
                }],
            ))
        }
        "xor" => {
            let inner = inner.unwrap_or(Ty::Other("?".into()));
            let r = simple(cx, recv, &Ty::Option(Box::new(inner.clone())), line, out);
            let other = simple(cx, b0, &Ty::Option(Box::new(inner.clone())), line, out);
            let r_has = Expr::call(r.clone(), "isPresent", vec![]);
            let o_has = Expr::call(other.clone(), "isPresent", vec![]);
            let some = Expr::Ternary(Box::new(r_has.clone()), Box::new(r), Box::new(other));
            Some(Expr::Ternary(
                Box::new(Expr::bin(r_has, "!=", o_has)),
                Box::new(some),
                Box::new(opt_empty(Some(&inner))),
            ))
        }
        "inspect" => {
            let r = simple(
                cx,
                recv,
                &Ty::Option(Box::new(inner.unwrap_or(Ty::Other("?".into())))),
                line,
                out,
            );
            out.push(Stmt::new(
                line,
                StmtKind::Expr(Expr::call(r.clone(), "ifPresent", vec![b0])),
            ));
            Some(r)
        }
        "iter" => Some(Expr::call(recv, "stream", vec![])),
        _ => None,
    }
}

fn b1_of(args: &[Expr]) -> Expr {
    args.get(1).cloned().unwrap_or_else(|| Expr::raw("null"))
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
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    // A line of a line-mode stdin `lines()` stream is a plain String at runtime.
    if matches!(name, "unwrap" | "expect") && super::io::phantom_line_result(cx, recv_ast) {
        return Some(recv);
    }
    // A `read_to_string()` receiver already lowered to the unwrapped value when it saw
    // the `.unwrap()` call; pass it through.
    if matches!(name, "unwrap" | "expect") && is_unwrapped_source(cx, recv_ast) {
        return Some(recv);
    }
    if crate::mode::throws() {
        return crate::lower::throws::result_method(cx, _m, recv_ast, recv, args, name, ret, line, out);
    }
    let types = result_types(cx, recv_ast);
    let ok_ty = types
        .as_ref()
        .map(|(t, _)| t.clone())
        .unwrap_or(Ty::Other("?".into()));
    let err_ty = types
        .as_ref()
        .map(|(_, e)| e.clone())
        .unwrap_or(Ty::Other("?".into()));
    match name {
        "is_ok" => Some(Expr::call(recv, "is_ok", vec![])),
        "is_err" => Some(Expr::call(recv, "is_err", vec![])),
        "ok" => Some(Expr::call(recv, "ok", vec![])),
        "err" => Some(Expr::call(recv, "err", vec![])),
        // The added Debug text lands on stderr only, so a parse error can use the plain overload.
        "unwrap" if is_parse_error(&err_ty) => Some(Expr::call(recv, "unwrap", vec![])),
        "expect" if is_parse_error(&err_ty) => Some(Expr::call(recv, "expect", vec![b0])),
        "unwrap" => Some(Expr::call(
            recv,
            "unwrap",
            vec![debug_lambda(cx, &err_ty, "err_value")],
        )),
        "expect" => Some(Expr::call(
            recv,
            "expect",
            vec![b0, debug_lambda(cx, &err_ty, "err_value")],
        )),
        "unwrap_err" => Some(Expr::call(recv, "unwrap_err", vec![])),
        "expect_err" => Some(Expr::call(
            recv,
            "expect_err",
            vec![b0, debug_lambda(cx, &ok_ty, "ok_value")],
        )),
        "map" | "map_err" | "and_then" | "or_else" => Some(Expr::call(recv, name, vec![b0])),
        "unwrap_or" => Some(Expr::call(recv, "unwrap_or", vec![b0])),
        "unwrap_or_else" => Some(Expr::call(recv, "unwrap_or_else", vec![b0])),
        // `map_or(d, f)` / `map_or_else(fe, f)`: map the Ok value, then fall back on Err.
        "map_or" | "map_or_else" => {
            let f = args.get(1).cloned()?;
            let mapped = Expr::call(recv, "map", vec![f]);
            let fallback = if name == "map_or" {
                "unwrap_or"
            } else {
                "unwrap_or_else"
            };
            Some(Expr::call(mapped, fallback, vec![b0]))
        }
        "unwrap_or_default" => Some(Expr::call(
            recv,
            "unwrap_or",
            vec![default_value_cx(cx, &ok_ty)],
        )),
        "is_ok_and" => {
            let mapped = Expr::call(recv, "map", vec![b0]);
            Some(Expr::call(mapped, "unwrap_or", vec![Expr::raw("false")]))
        }
        "is_err_and" => {
            let err = Expr::call(recv, "err", vec![]);
            let mapped = Expr::call(err, "map", vec![b0]);
            Some(Expr::call(mapped, "orElse", vec![Expr::raw("false")]))
        }
        "inspect_err" => Some(Expr::call(recv, "inspect_err", vec![b0])),
        "iter" => Some(Expr::call(Expr::call(recv, "ok", vec![]), "stream", vec![])),
        "flatten" => {
            let x = cx.fresh("inner");
            Some(Expr::call(
                recv,
                "and_then",
                vec![Expr::Lambda {
                    params: vec![x.clone()],
                    body: LambdaBody::Expr(Box::new(Expr::name(x))),
                }],
            ))
        }
        "transpose" => transpose(cx, recv, ret, line, out),
        "clone" | "as_ref" | "as_mut" | "cloned" | "copied" => Some(recv),
        _ => None,
    }
}

// `Result<Option<T>, E>` -> `Optional<Result<T, E>>`.
fn transpose(cx: &mut Cx, recv: Expr, ret: &Ty, line: u32, out: &mut Vec<Stmt>) -> Option<Expr> {
    let (ok, err) = match ret.strip_refs() {
        Ty::Option(t) => match t.strip_refs() {
            Ty::Result(a, b) => ((**a).clone(), (**b).clone()),
            _ => return None,
        },
        _ => return None,
    };
    let name = cx.fresh("transposed");
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: stmt::decl_type_explicit(ret),
            name: name.clone(),
            init: None,
        },
    ));
    let o = cx.fresh("o");
    let e = cx.fresh("e");
    let v = cx.fresh("v");
    let mapped = Expr::raw(format!(
        "{}.<Result<{}, {}>>map({v} -> new Ok<>({v}))",
        render(&Expr::name(o.clone())),
        ok.boxed(),
        err.boxed()
    ));
    let arms = vec![
        Arm {
            line,
            label: format!("Ok(var {o})"),
            body: ArmBody::Expr(Expr::Assign(
                Box::new(Expr::name(name.clone())),
                "=",
                Box::new(mapped),
            )),
        },
        Arm {
            line,
            label: format!("Err(var {e})"),
            body: ArmBody::Expr(Expr::Assign(
                Box::new(Expr::name(name.clone())),
                "=",
                Box::new(opt_of(Expr::New {
                    ty: "Err<>".into(),
                    args: vec![Expr::name(e)],
                })),
            )),
        },
    ];
    out.push(Stmt::new(line, StmtKind::Switch { scrut: recv, arms }));
    Some(Expr::name(name))
}

pub(super) fn is_unwrapped_source(cx: &Cx, recv_ast: &ast::Expr) -> bool {
    match recv_ast {
        ast::Expr::MethodCallExpr(mc) => {
            matches!(
                method_name(mc).as_str(),
                "read_line" | "read_to_string" | "read_to_end" | "write_all" | "write" | "flush"
            ) || (method_name(mc) == "parse" && super::text::direct_parse(cx, recv_ast).is_some())
                || (method_name(mc) == "read"
                    && mc.receiver().is_some_and(|r| {
                        matches!(cx.ty(&r).strip_refs(), Ty::Other(n)
                            if matches!(n.as_str(), "Stdin" | "StdinLock" | "BufReader" | "File"))
                    }))
        }
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
            (text.ends_with("from_str_radix") && super::text::direct_parse(cx, recv_ast).is_some())
                || text.ends_with("read_to_string")
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

// ---- parse errors ----------------------------------------------------------------------------

// Parses into the Rust error text, so `{}` matches rustc; `{:?}` maps it back to the kind.
// `.ok()`, `.unwrap_or(..)`, `if let Ok(x)`, `Err(_)` arms and `|_|` closures never read the error.
fn error_discarded(m: &ast::MethodCallExpr) -> bool {
    let Some(parent) = m.syntax().parent() else {
        return false;
    };
    if let Some(p) = ast::MethodCallExpr::cast(parent.clone()) {
        return p.receiver().is_some_and(|r| r.syntax() == m.syntax())
            && match method_name(&p).as_str() {
                "ok" | "is_ok" | "is_err" | "unwrap_or" | "unwrap_or_default" => true,
                "map_err" | "unwrap_or_else" => closure_ignores_arg(&p),
                _ => false,
            };
    }
    if let Some(le) = ast::LetExpr::cast(parent.clone()) {
        return le
            .pat()
            .is_some_and(|p| match tuple_struct_name(&p).as_deref() {
                Some("Ok") => true,
                Some("Err") => pat_binds_nothing(&p),
                _ => false,
            });
    }
    ast::MatchExpr::cast(parent).is_some_and(|me| match_discards_error(&me))
}

// The first arm that can match `Err` decides whether the error value is ever read.
fn match_discards_error(me: &ast::MatchExpr) -> bool {
    let Some(list) = me.match_arm_list() else {
        return false;
    };
    for arm in list.arms() {
        let Some(p) = arm.pat() else {
            return false;
        };
        if tuple_struct_name(&p).as_deref() == Some("Ok") {
            continue;
        }
        return arm.guard().is_none()
            && tuple_struct_name(&p).as_deref() == Some("Err")
            && pat_binds_nothing(&p);
    }
    false
}

fn tuple_struct_name(p: &ast::Pat) -> Option<String> {
    let ast::Pat::TupleStructPat(tp) = p else {
        return None;
    };
    let path = tp.path()?;
    path.segment()?.name_ref().map(|n| n.text().to_string())
}

fn pat_binds_nothing(p: &ast::Pat) -> bool {
    match p {
        ast::Pat::TupleStructPat(tp) => tp
            .fields()
            .all(|f| matches!(f, ast::Pat::WildcardPat(_) | ast::Pat::RestPat(_))),
        ast::Pat::WildcardPat(_) | ast::Pat::RestPat(_) => true,
        _ => false,
    }
}

fn closure_ignores_arg(m: &ast::MethodCallExpr) -> bool {
    let Some(ast::Expr::ClosureExpr(c)) = m.arg_list().and_then(|l| l.args().next()) else {
        return false;
    };
    c.param_list()
        .and_then(|pl| pl.params().next())
        .and_then(|p| p.pat())
        .is_some_and(|p| matches!(p, ast::Pat::WildcardPat(_)))
}

pub(super) fn parse_result(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    input: Expr,
    ret: &Ty,
    target: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    cx.pending_name = cx.take_let_target(m.syntax());
    let e = parse_result_inner(cx, m, input, ret, target, line, out);
    cx.pending_name = None;
    e
}

fn parse_result_inner(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    input: Expr,
    ret: &Ty,
    target: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    match target.strip_refs() {
        Ty::Int(i) => {
            let exact = !error_discarded(m);
            parse_int(cx, input, ret, *i, None, exact, line, out)
        }
        Ty::F32 | Ty::F64 => parse_float(
            cx,
            input,
            ret,
            target.strip_refs() == &Ty::F32,
            !error_discarded(m),
            line,
            out,
        ),
        Ty::Bool => {
            cx.needs_result = true;
            let ty = result_java(ret);
            let name = cx.result_name();
            push_raw(
                out,
                line,
                format!(
                    "{ty} {name} = new Ok<>(Boolean.parseBoolean({}));",
                    render(&input)
                ),
            );
            Expr::name(name)
        }
        Ty::Char => {
            cx.needs_result = true;
            let ty = result_java(ret);
            let name = cx.result_name();
            push_raw(
                out,
                line,
                format!("{ty} {name} = new Ok<>({}.charAt(0));", render(&input)),
            );
            Expr::name(name)
        }
        _ => cx.unsupported(m.syntax(), "`parse` to this type"),
    }
}

pub(super) fn parse_int(
    cx: &mut Cx,
    input: Expr,
    ret: &Ty,
    i: Int,
    radix: Option<Expr>,
    exact: bool,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    cx.needs_result = true;
    if !exact
        && radix.is_none()
        && let Some(f) = super::text::int_parser(i)
    {
        let ty = result_java(ret);
        let res = cx.result_name();
        let ex = cx.fresh_param("ex");
        push_raw(
            out,
            line,
            format!(
                "{ty} {res}; try {{ {res} = new Ok<>({f}({})); }} catch (NumberFormatException {ex}) {{ {res} = new Err<>({ex}); }}",
                render(&input)
            ),
        );
        cx.release_param(&ex);
        return Expr::name(res);
    }
    // The input is read more than once; a plain name or literal needs no copy.
    let s = if super::is_simple(&input) {
        render(&input)
    } else {
        let s = cx.fresh("text");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "String".into(),
                name: s.clone(),
                init: Some(input),
            },
        ));
        s
    };
    let ty = result_java(ret);
    let res = cx.result_name();
    let raw = cx.fresh_param("num");
    let ex = cx.fresh_param("ex");
    let parsed = match (&radix, i) {
        (Some(r), Int::U64 | Int::U128) => format!("Long.parseUnsignedLong({s}, {})", render(r)),
        (Some(r), _) => format!("Long.parseLong({s}, {})", render(r)),
        (None, Int::U64 | Int::U128) => format!("Long.parseUnsignedLong({s})"),
        (None, _) => format!("Long.parseLong({s})"),
    };
    // These parsers already reject everything outside Java's long, so no range check is needed.
    let full_long = matches!(i, Int::I64 | Int::I128 | Int::U64 | Int::U128);
    let (min, max, unsigned) = int_range(i);
    let value = if i.is_long() {
        narrow(Expr::raw(raw.clone()), i)
    } else {
        Expr::Cast(i.java().into(), Box::new(Expr::raw(raw.clone())))
    };
    let ok = if full_long {
        None
    } else if unsigned {
        Some(format!("{raw} >= 0 && {raw} <= {max}"))
    } else {
        Some(format!("{raw} >= {min} && {raw} <= {max}"))
    };
    let radix_val = radix.as_ref().and_then(|r| match r {
        Expr::Raw(text) => text
            .trim()
            .parse::<u32>()
            .ok()
            .filter(|v| (2..=36).contains(v)),
        _ => None,
    });
    let failure = if exact {
        int_error(&s, unsigned, radix_val, radix.is_some())
    } else {
        String::new()
    };
    let body = match &ok {
        Some(cond) => format!(
            "long {raw} = {parsed}; if ({cond}) {{ {res} = new Ok<>({}); }} else {{ {res} = new Err<>(new NumberFormatException({failure})); }}",
            render(&value)
        ),
        None => format!("{res} = new Ok<>({parsed});"),
    };
    push_raw(
        out,
        line,
        format!(
            "{ty} {res}; try {{ {body} }} catch (NumberFormatException {ex}) {{ {res} = new Err<>(new NumberFormatException({failure})); }}"
        ),
    );
    cx.release_param(&ex);
    cx.release_param(&raw);
    Expr::name(res)
}

pub(super) fn parse_float(
    cx: &mut Cx,
    input: Expr,
    ret: &Ty,
    f32: bool,
    exact: bool,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    cx.needs_result = true;
    let ty = result_java(ret);
    let res = cx.result_name();
    let ex = cx.fresh_param("ex");
    let parser = if f32 {
        "Float.parseFloat"
    } else {
        "Double.parseDouble"
    };
    if !exact {
        push_raw(
            out,
            line,
            format!(
                "{ty} {res}; try {{ {res} = new Ok<>({parser}({})); }} catch (NumberFormatException {ex}) {{ {res} = new Err<>({ex}); }}",
                render(&input)
            ),
        );
        cx.release_param(&ex);
        return Expr::name(res);
    }
    // The input is read more than once; a plain name or literal needs no copy.
    let s = if super::is_simple(&input) {
        render(&input)
    } else {
        let s = cx.fresh("text");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "String".into(),
                name: s.clone(),
                init: Some(input),
            },
        ));
        s
    };
    let failure = format!(
        "({s}.isEmpty() ? \"cannot parse float from empty string\" : \"invalid float literal\")"
    );
    push_raw(
        out,
        line,
        format!(
            "{ty} {res}; try {{ {res} = new Ok<>({parser}({s})); }} catch (NumberFormatException {ex}) {{ {res} = new Err<>(new NumberFormatException({failure})); }}"
        ),
    );
    cx.release_param(&ex);
    Expr::name(res)
}

fn digit_class(radix: u32) -> String {
    let lower: String = (0..radix)
        .filter_map(|d| char::from_digit(d, radix))
        .collect();
    let upper: String = lower
        .chars()
        .filter(|c| c.is_ascii_lowercase())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    format!("{lower}{upper}")
}

fn int_range(i: Int) -> (String, String, bool) {
    let max = super::num::int_max(i).to_string();
    let min = match i {
        Int::I8 => "-128".to_string(),
        Int::I16 => "-32768".to_string(),
        Int::I32 | Int::Isize => "Integer.MIN_VALUE".to_string(),
        Int::I64 | Int::I128 => "Long.MIN_VALUE".to_string(),
        _ => "0".to_string(),
    };
    let unsigned = i.is_unsigned();
    (min, max, unsigned)
}

fn int_error(s: &str, unsigned: bool, radix: Option<u32>, has_radix: bool) -> String {
    if let Some(r) = radix {
        let digits = digit_class(r);
        return if unsigned {
            format!(
                "({s}.isEmpty() ? \"cannot parse integer from empty string\" : ({s}.matches(\"[+{digits}]+\") ? \"number too large to fit in target type\" : \"invalid digit found in string\"))"
            )
        } else {
            format!(
                "({s}.isEmpty() ? \"cannot parse integer from empty string\" : ({s}.matches(\"[+-]?[{digits}]+\") ? ({s}.startsWith(\"-\") ? \"number too small to fit in target type\" : \"number too large to fit in target type\") : \"invalid digit found in string\"))"
            )
        };
    }
    if has_radix {
        return format!(
            "({s}.isEmpty() ? \"cannot parse integer from empty string\" : ({s}.matches(\"[+-]?[0-9a-zA-Z]+\") ? ({s}.startsWith(\"-\") ? ({}) : \"number too large to fit in target type\") : \"invalid digit found in string\"))",
            if unsigned {
                "\"invalid digit found in string\"".to_string()
            } else {
                "\"number too small to fit in target type\"".to_string()
            }
        );
    }
    if unsigned {
        format!(
            "({s}.isEmpty() ? \"cannot parse integer from empty string\" : ({s}.matches(\"[+]?[0-9]+\") ? \"number too large to fit in target type\" : \"invalid digit found in string\"))"
        )
    } else {
        format!(
            "({s}.isEmpty() ? \"cannot parse integer from empty string\" : ({s}.matches(\"[+-]?[0-9]+\") ? ({s}.startsWith(\"-\") ? \"number too small to fit in target type\" : \"number too large to fit in target type\") : \"invalid digit found in string\"))"
        )
    }
}
