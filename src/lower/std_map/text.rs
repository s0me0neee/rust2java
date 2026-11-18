// Std String/&str, char, bool.

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
            "then_some" => opt_of(b0),
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
        "is_ascii_whitespace" => Expr::bin(
            Expr::bin(recv.clone(), "==", Expr::raw("' '")),
            "||",
            Expr::bin(
                Expr::bin(recv.clone(), ">=", Expr::raw("'\\t'")),
                "&&",
                Expr::bin(recv, "<=", Expr::raw("'\\r'")),
            ),
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
        "clear" => {
            return Some(expr::place_assign(
                cx,
                recv_ast,
                "=",
                Expr::str_lit(""),
                out,
            ));
        }
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
            push_raw(
                out,
                line,
                format!("if ({n}.isPresent()) {ps} = {ps}.substring(0, {ps}.length() - 1);"),
            );
            return Some(Expr::name(n));
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
        "contains" => {
            let arg = if matches!(arg0_ty.strip_refs(), Ty::Char) {
                Expr::call(Expr::raw("String"), "valueOf", vec![b0])
            } else {
                b0
            };
            return Some(Expr::call(recv, "contains", vec![arg]));
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
            let test = if name == "strip_prefix" {
                "startsWith"
            } else {
                "endsWith"
            };
            let sv = simple(cx, recv, &Ty::Str, line, out);
            let av = simple(cx, b0, &arg0_ty, line, out);
            let inner = if name == "strip_prefix" {
                Expr::call(
                    sv.clone(),
                    "substring",
                    vec![Expr::call(av.clone(), "length", vec![])],
                )
            } else {
                Expr::call(
                    sv.clone(),
                    "substring",
                    vec![
                        Expr::raw("0"),
                        Expr::bin(
                            Expr::call(sv.clone(), "length", vec![]),
                            "-",
                            Expr::call(av.clone(), "length", vec![]),
                        ),
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
    // `FromStr` already returns a Result; the outer `.unwrap()` is dropped by
    // `is_unwrapped_source`, so unwrap here where the value is wanted.
    if let Ty::Adt { java, .. } = target.strip_refs() {
        let call = Expr::static_call(format!("{java}.from_str"), vec![recv]);
        let outer = m.syntax().parent().and_then(ast::MethodCallExpr::cast);
        return match consumed_by(cx, &ast::Expr::MethodCallExpr(m.clone())).as_deref() {
            Some("unwrap") => Expr::call(call, "unwrap", vec![]),
            Some("expect") => {
                let msg = outer
                    .and_then(|o| o.arg_list())
                    .and_then(|a| a.args().next())
                    .map(|a| expr::lower_expr(cx, &a, out))
                    .unwrap_or_else(|| Expr::str_lit(""));
                Expr::call(call, "expect", vec![msg])
            }
            _ => call,
        };
    }
    let call = match target.strip_refs() {
        Ty::Int(i) if i.is_long() => Expr::call(Expr::raw("Long"), "parseLong", vec![recv.clone()]),
        Ty::Int(_) => Expr::call(Expr::raw("Integer"), "parseInt", vec![recv.clone()]),
        Ty::F32 => Expr::call(Expr::raw("Float"), "parseFloat", vec![recv.clone()]),
        Ty::F64 => Expr::call(Expr::raw("Double"), "parseDouble", vec![recv.clone()]),
        Ty::Bool => Expr::call(Expr::raw("Boolean"), "parseBoolean", vec![recv.clone()]),
        Ty::Char => Expr::call(recv.clone(), "charAt", vec![Expr::raw("0")]),
        _ => return cx.unsupported(m.syntax(), "`parse` to this type"),
    };
    if matches!(
        consumed_by(cx, &ast::Expr::MethodCallExpr(m.clone())).as_deref(),
        Some("unwrap" | "expect")
    ) {
        return call;
    }
    wrap_try_ok(cx, line, ret, call, "NumberFormatException", out)
}
