// Std integers, floats, Ordering, numeric conversions.

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

// ---- integers --------------------------------------------------------------------------------

pub(super) fn lower_int_method(
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
    let bits =
        |f: &str, a: Vec<Expr>| Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), f, a);
    match name {
        "abs" => Some(narrow(math("abs", vec![recv]), i)),
        "signum" => Some(narrow(
            Expr::call(
                Expr::raw(if long { "Long" } else { "Integer" }),
                "signum",
                vec![recv],
            ),
            i,
        )),
        "pow" => Some(Expr::Cast(
            i.java().into(),
            Box::new(math("pow", vec![recv, b0])),
        )),
        "min" => Some(narrow(math("min", vec![recv, b0]), i)),
        "max" => Some(narrow(math("max", vec![recv, b0]), i)),
        "rem_euclid" => Some(narrow(
            math("floorMod", vec![recv, math("abs", vec![b0])]),
            i,
        )),
        "div_euclid" => Some(narrow(math("floorDiv", vec![recv, b0]), i)),
        "div_ceil" => Some(narrow(
            Expr::bin(
                Expr::bin(Expr::bin(recv, "+", b0.clone()), "-", Expr::raw("1")),
                "/",
                b0,
            ),
            i,
        )),
        "count_ones" => Some(bits("bitCount", vec![recv])),
        "count_zeros" => Some(Expr::bin(
            Expr::raw(if long { "64" } else { "32" }),
            "-",
            bits("bitCount", vec![recv]),
        )),
        "leading_zeros" => Some(bits("numberOfLeadingZeros", vec![recv])),
        "trailing_zeros" => Some(bits("numberOfTrailingZeros", vec![recv])),
        "swap_bytes" => Some(bits("reverseBytes", vec![recv])),
        "reverse_bits" => Some(bits("reverse", vec![recv])),
        "rotate_left" => Some(bits("rotateLeft", vec![recv, b0])),
        "rotate_right" => Some(bits("rotateRight", vec![recv, b0])),
        "is_power_of_two" => Some(Expr::bin(
            Expr::bin(
                Expr::bin(
                    recv.clone(),
                    "&",
                    Expr::bin(recv.clone(), "-", Expr::raw("1")),
                ),
                "==",
                Expr::raw("0"),
            ),
            "&&",
            Expr::bin(recv, ">", Expr::raw("0")),
        )),
        "is_positive" => Some(Expr::bin(recv, ">", Expr::raw("0"))),
        "is_negative" => Some(Expr::bin(recv, "<", Expr::raw("0"))),
        "cmp" => Some(Expr::call(
            Expr::raw(if long { "Long" } else { "Integer" }),
            "compare",
            vec![recv, b0],
        )),
        "partial_cmp" => Some(opt_of(Expr::call(
            Expr::raw(if long { "Long" } else { "Integer" }),
            "compare",
            vec![recv, b0],
        ))),
        "to_string" => Some(Expr::call(Expr::raw("String"), "valueOf", vec![recv])),
        "clone" | "to_owned" => Some(recv),
        "abs_diff" => Some(narrow(math("abs", vec![Expr::bin(recv, "-", b0)]), i)),
        _ if name.starts_with("checked_") => {
            Some(checked_int(cx, recv, b0, b1, i, name, ret, line, out))
        }
        _ if name.starts_with("saturating_") => Some(saturating_int(recv, b0, i, name)),
        _ if name.starts_with("wrapping_") => Some(wrapping_int(recv, b0, i, name)),
        _ => None,
    }
}

pub(super) fn checked_int(
    cx: &mut Cx,
    recv: Expr,
    b0: Expr,
    _b1: Expr,
    i: Int,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let inner = opt_inner(ret);
    let empty = opt_empty(inner.as_ref());
    let yes = |v: Expr| opt_of(narrow(v, i));
    let uint = i.is_unsigned();
    let try_opt = |cx: &mut Cx, out: &mut Vec<Stmt>, v: Expr, exc: &str| {
        let ty = format!(
            "Optional<{}>",
            inner
                .as_ref()
                .map(|t| t.boxed())
                .unwrap_or_else(|| "Integer".into())
        );
        let n = cx.fresh("chk");
        let ex = cx.fresh("ex");
        push_raw(
            out,
            line,
            format!(
                "{ty} {n}; try {{ {n} = Optional.of({}); }} catch ({exc} {ex}) {{ {n} = Optional.empty(); }}",
                render(&v)
            ),
        );
        Expr::name(n)
    };
    match name {
        "checked_add" => match i {
            Int::U8 => conditional(
                Expr::bin(
                    Expr::bin(recv.clone(), "+", b0.clone()),
                    "<=",
                    Expr::raw("255"),
                ),
                yes(Expr::bin(recv, "+", b0)),
                empty,
            ),
            Int::U16 => conditional(
                Expr::bin(
                    Expr::bin(recv.clone(), "+", b0.clone()),
                    "<=",
                    Expr::raw("65535"),
                ),
                yes(Expr::bin(recv, "+", b0)),
                empty,
            ),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(
                    Expr::bin(
                        Expr::Cast("long".into(), Box::new(recv.clone())),
                        "+",
                        b0.clone(),
                    ),
                    "<=",
                    Expr::raw("4294967295L"),
                ),
                yes(Expr::bin(recv, "+", b0)),
                empty,
            ),
            Int::U64 | Int::U128 => try_opt(
                cx,
                out,
                Expr::call(Expr::raw("Math"), "addExact", vec![recv, b0]),
                "ArithmeticException",
            ),
            Int::I8 => range_checked(recv, b0, i, "+", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "+", -32768, 32767, inner.as_ref()),
            _ => try_opt(
                cx,
                out,
                Expr::call(Expr::raw("Math"), "addExact", vec![recv, b0]),
                "ArithmeticException",
            ),
        },
        "checked_sub" => match i {
            Int::U8 | Int::U16 | Int::U32 | Int::Usize => conditional(
                Expr::bin(recv.clone(), ">=", b0.clone()),
                yes(Expr::bin(recv, "-", b0)),
                empty,
            ),
            Int::U64 | Int::U128 => conditional(
                Expr::bin(
                    Expr::call(
                        Expr::raw("Long"),
                        "compareUnsigned",
                        vec![recv.clone(), b0.clone()],
                    ),
                    ">=",
                    Expr::raw("0"),
                ),
                yes(Expr::bin(recv, "-", b0)),
                empty,
            ),
            Int::I8 => range_checked(recv, b0, i, "-", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "-", -32768, 32767, inner.as_ref()),
            _ => try_opt(
                cx,
                out,
                Expr::call(Expr::raw("Math"), "subtractExact", vec![recv, b0]),
                "ArithmeticException",
            ),
        },
        "checked_mul" => match i {
            Int::U8 => conditional(
                Expr::bin(
                    Expr::bin(recv.clone(), "*", b0.clone()),
                    "<=",
                    Expr::raw("255"),
                ),
                yes(Expr::bin(recv, "*", b0)),
                empty,
            ),
            Int::U16 => conditional(
                Expr::bin(
                    Expr::bin(
                        Expr::Cast("long".into(), Box::new(recv.clone())),
                        "*",
                        b0.clone(),
                    ),
                    "<=",
                    Expr::raw("65535"),
                ),
                yes(Expr::Cast(
                    "int".into(),
                    Box::new(Expr::bin(
                        Expr::Cast("long".into(), Box::new(recv)),
                        "*",
                        b0,
                    )),
                )),
                empty,
            ),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(
                    Expr::bin(
                        Expr::Cast("long".into(), Box::new(recv.clone())),
                        "*",
                        b0.clone(),
                    ),
                    "<=",
                    Expr::raw("4294967295L"),
                ),
                yes(Expr::Cast(
                    "int".into(),
                    Box::new(Expr::bin(
                        Expr::Cast("long".into(), Box::new(recv)),
                        "*",
                        b0,
                    )),
                )),
                empty,
            ),
            Int::I8 => range_checked(recv, b0, i, "*", -128, 127, inner.as_ref()),
            Int::I16 => range_checked(recv, b0, i, "*", -32768, 32767, inner.as_ref()),
            _ => try_opt(
                cx,
                out,
                Expr::call(Expr::raw("Math"), "multiplyExact", vec![recv, b0]),
                "ArithmeticException",
            ),
        },
        "checked_div" => {
            let minv = int_min(i);
            let cond = if uint {
                Expr::bin(b0.clone(), "!=", Expr::raw("0"))
            } else {
                Expr::bin(
                    Expr::bin(b0.clone(), "!=", Expr::raw("0")),
                    "&&",
                    Expr::Unary(
                        "!",
                        Box::new(Expr::bin(
                            Expr::bin(recv.clone(), "==", Expr::raw(minv)),
                            "&&",
                            Expr::bin(b0.clone(), "==", Expr::raw("-1")),
                        )),
                    ),
                )
            };
            conditional(cond, yes(Expr::bin(recv, "/", b0)), empty)
        }
        "checked_rem" => conditional(
            Expr::bin(b0.clone(), "!=", Expr::raw("0")),
            yes(Expr::bin(recv, "%", b0)),
            empty,
        ),
        "checked_neg" => {
            let minv = int_min(i);
            conditional(
                Expr::bin(recv.clone(), "==", Expr::raw(minv)),
                empty,
                yes(Expr::Unary("-", Box::new(recv))),
            )
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

pub(super) fn conditional(cond: Expr, a: Expr, b: Expr) -> Expr {
    Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
}

pub(super) fn range_checked(
    recv: Expr,
    b: Expr,
    i: Int,
    op: &'static str,
    lo: i64,
    hi: i64,
    _inner: Option<&Ty>,
) -> Expr {
    let val = Expr::bin(recv, op, b);
    let cond = Expr::bin(
        Expr::bin(val.clone(), ">=", Expr::raw(lo.to_string())),
        "&&",
        Expr::bin(val.clone(), "<=", Expr::raw(hi.to_string())),
    );
    Expr::Ternary(
        Box::new(cond),
        Box::new(opt_of(narrow(val, i))),
        Box::new(Expr::raw("Optional.empty()")),
    )
}

pub(super) fn int_min(i: Int) -> &'static str {
    match i {
        Int::I8 => "-128",
        Int::I16 => "-32768",
        Int::I64 | Int::I128 => "Long.MIN_VALUE",
        _ => "Integer.MIN_VALUE",
    }
}

pub(super) fn saturating_int(recv: Expr, b: Expr, i: Int, name: &str) -> Expr {
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    match name {
        "saturating_add" => match i {
            Int::U8 => narrow(
                math("min", vec![Expr::bin(recv, "+", b), Expr::raw("255")]),
                i,
            ),
            Int::U16 => narrow(
                math(
                    "min",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b),
                        Expr::raw("65535L"),
                    ],
                ),
                i,
            ),
            Int::U32 | Int::Usize => narrow(
                math(
                    "min",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b),
                        Expr::raw("4294967295L"),
                    ],
                ),
                i,
            ),
            Int::I8 | Int::I16 => narrow(
                math("min", vec![Expr::raw(int_max(i)), Expr::bin(recv, "+", b)]),
                i,
            ),
            Int::I32 => narrow(
                math(
                    "max",
                    vec![
                        Expr::raw("Integer.MIN_VALUE"),
                        math(
                            "min",
                            vec![
                                Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b),
                                Expr::raw("Integer.MAX_VALUE"),
                            ],
                        ),
                    ],
                ),
                i,
            ),
            _ => Expr::bin(recv, "+", b),
        },
        "saturating_sub" => match i {
            Int::U8 | Int::U16 | Int::U32 | Int::Usize => narrow(
                math("max", vec![Expr::bin(recv, "-", b), Expr::raw("0")]),
                i,
            ),
            Int::I8 | Int::I16 => narrow(
                math("max", vec![Expr::raw(int_min(i)), Expr::bin(recv, "-", b)]),
                i,
            ),
            Int::I32 => narrow(
                math(
                    "max",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "-", b),
                        Expr::raw("Integer.MIN_VALUE"),
                    ],
                ),
                i,
            ),
            _ => Expr::bin(recv, "-", b),
        },
        "saturating_mul" => match i {
            Int::U8 => narrow(
                math("min", vec![Expr::bin(recv, "*", b), Expr::raw("255")]),
                i,
            ),
            Int::U16 => narrow(
                math(
                    "min",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b),
                        Expr::raw("65535L"),
                    ],
                ),
                i,
            ),
            Int::U32 | Int::Usize => narrow(
                math(
                    "min",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b),
                        Expr::raw("4294967295L"),
                    ],
                ),
                i,
            ),
            Int::I32 => narrow(
                math(
                    "max",
                    vec![
                        Expr::raw("Integer.MIN_VALUE"),
                        math(
                            "min",
                            vec![
                                Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b),
                                Expr::raw("Integer.MAX_VALUE"),
                            ],
                        ),
                    ],
                ),
                i,
            ),
            _ => Expr::bin(recv, "*", b),
        },
        "saturating_abs" => {
            let m = int_min(i);
            conditional(
                Expr::bin(recv.clone(), "==", Expr::raw(m)),
                narrow(Expr::raw(int_max(i)), i),
                narrow(math("abs", vec![recv]), i),
            )
        }
        "saturating_neg" => {
            let m = int_min(i);
            conditional(
                Expr::bin(recv.clone(), "==", Expr::raw(m)),
                narrow(Expr::raw(int_max(i)), i),
                narrow(Expr::Unary("-", Box::new(recv)), i),
            )
        }
        _ => Expr::bin(recv, "*", b),
    }
}

pub(super) fn int_max(i: Int) -> &'static str {
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

pub(super) fn wrapping_int(recv: Expr, b: Expr, i: Int, name: &str) -> Expr {
    match name {
        "wrapping_add" => narrow(Expr::bin(recv, "+", b), i),
        "wrapping_sub" => narrow(Expr::bin(recv, "-", b), i),
        "wrapping_mul" => narrow(Expr::bin(recv, "*", b), i),
        "wrapping_div" => narrow(Expr::bin(recv, "/", b), i),
        "wrapping_rem" => narrow(Expr::bin(recv, "%", b), i),
        "wrapping_neg" => narrow(Expr::Unary("-", Box::new(recv)), i),
        "wrapping_shl" => narrow(Expr::bin(recv, "<<", b), i),
        "wrapping_shr" => narrow(
            Expr::bin(recv, if i.is_unsigned() { ">>>" } else { ">>" }, b),
            i,
        ),
        "wrapping_abs" => narrow(Expr::call(Expr::raw("Math"), "abs", vec![recv]), i),
        _ => recv,
    }
}

// ---- floats ----------------------------------------------------------------------------------

pub(super) fn lower_float_method(
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
    let cast = |v: Expr| {
        if f32 {
            Expr::Cast("float".into(), Box::new(v))
        } else {
            v
        }
    };
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0.0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0.0"));
    Some(match name {
        "abs" => math("abs", vec![recv]),
        "min" => math("min", vec![recv, b0]),
        "max" => math("max", vec![recv, b0]),
        "sqrt" => cast(math("sqrt", vec![recv])),
        "floor" => cast(math("floor", vec![recv])),
        "ceil" => cast(math("ceil", vec![recv])),
        "trunc" => cast(Expr::Ternary(
            Box::new(Expr::bin(recv.clone(), "<", Expr::raw("0"))),
            Box::new(math("ceil", vec![recv.clone()])),
            Box::new(math("floor", vec![recv])),
        )),
        "round" => {
            let s = simple(cx, recv, &if f32 { Ty::F32 } else { Ty::F64 }, line, out);
            let r = render(&s);
            cast(Expr::raw(format!(
                "Math.copySign(Math.floor(Math.abs({r}) + 0.5), {r})"
            )))
        }
        "powi" | "powf" => cast(math("pow", vec![recv, b0])),
        "mul_add" => Expr::bin(Expr::bin(recv, "*", b0), "+", b1),
        "rem_euclid" => {
            let s = simple(cx, recv, &if f32 { Ty::F32 } else { Ty::F64 }, line, out);
            let r = render(&s);
            let b = render(&b0);
            Expr::raw(format!(
                "({r} - Math.floor({r} / Math.abs({b})) * Math.abs({b}))"
            ))
        }
        "is_nan" => Expr::call(Expr::raw(cls), "isNaN", vec![recv]),
        "is_infinite" => Expr::call(Expr::raw(cls), "isInfinite", vec![recv]),
        "is_finite" => Expr::bin(
            Expr::Unary(
                "!",
                Box::new(Expr::call(Expr::raw(cls), "isInfinite", vec![recv.clone()])),
            ),
            "&&",
            Expr::Unary(
                "!",
                Box::new(Expr::call(Expr::raw(cls), "isNaN", vec![recv])),
            ),
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

// ---- Ordering ----
pub(super) fn lower_ordering_method(
    cx: &mut Cx,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let arg0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    // `then*` reads the receiver twice; hoist anything with side effects.
    let mut once = |recv: Expr| -> Expr {
        if matches!(recv, Expr::Raw(_) | Expr::Name(_)) {
            return recv;
        }
        let tmp = cx.fresh("ord");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "int".into(),
                name: tmp.clone(),
                init: Some(recv),
            },
        ));
        Expr::name(tmp)
    };
    let zero_check = |v: &Expr| Expr::bin(v.clone(), "!=", Expr::raw("0"));
    Some(match name {
        // `a.then(b)` / `a.then_with(|| b)`: keep `a` unless it is Equal.
        "then" => {
            let v = once(recv);
            Expr::Ternary(Box::new(zero_check(&v)), Box::new(v), Box::new(arg0))
        }
        "then_with" => {
            let v = once(recv);
            let f = Expr::call(
                Expr::Cast("Supplier<Integer>".into(), Box::new(arg0)),
                "get",
                vec![],
            );
            Expr::Ternary(Box::new(zero_check(&v)), Box::new(v), Box::new(f))
        }
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
