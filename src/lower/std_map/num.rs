// Std integers, floats, Ordering, numeric conversions.

use super::*;

// Hook for std free functions: numeric `From` / `TryFrom`, e.g. `i64::from(x)`, `u8::try_from(y)`.
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let owner = key.split("::").next().unwrap_or("");
    let num_owner = Int::from_name(owner).is_some() || matches!(owner, "f32" | "f64" | "char");
    let from_call = key == "From::from" || (num_owner && key.ends_with("::from"));
    let try_from_call = key == "TryFrom::try_from" || (num_owner && key.ends_with("::try_from"));
    if !from_call && !try_from_call {
        return None;
    }
    let asts = arg_asts(c);
    let from = cx.ty(asts.first()?);
    let conversion = match (from.strip_refs(), ret.strip_refs()) {
        (Ty::Int(_), Ty::Int(_)) => true,
        (Ty::Int(_), Ty::F32 | Ty::F64) => true,
        (Ty::F32, Ty::F64) => true,
        (Ty::Char, Ty::Int(_)) | (Ty::Int(_), Ty::Char) => true,
        (Ty::Bool, Ty::Int(_)) => true,
        _ => false,
    };
    if from_call && !conversion {
        return None;
    }
    if try_from_call {
        let Ty::Result(t, _) = ret.strip_refs() else {
            return None;
        };
        if !matches!(
            (from.strip_refs(), t.strip_refs()),
            (Ty::Int(_), Ty::Int(_))
        ) {
            return None;
        }
    }
    let args = expr::call_args(cx, c, out);
    let v = args.into_iter().next()?;
    if try_from_call {
        try_from_num(cx, &from, ret, v)
    } else {
        num_from(&from, ret, v)
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
    // Numeric `into()`: `let x: f64 = n.into();`. rust-analyzer can leave it unresolved.
    if !matches!(key, "Into::into" | "?::into") {
        return None;
    }
    let ret = cx.ty(&ast::Expr::MethodCallExpr(m.clone()));
    let compatible = match (recv_ty.strip_refs(), ret.strip_refs()) {
        (Ty::Int(_), Ty::Int(_) | Ty::F32 | Ty::F64) => true,
        (Ty::F32, Ty::F64) => true,
        (Ty::Char, Ty::Int(_)) => true,
        _ => false,
    };
    if !compatible {
        return None;
    }
    let recv_ast = m.receiver()?;
    let recv = expr::lower_expr(cx, &recv_ast, out);
    num_from(recv_ty, &ret, recv)
}

// ---- numeric conversions ---------------------------------------------------------------------

// The value of an integer/char literal expression, if it is one: plain/underscored numbers with
// an optional Java cast in front, or a char literal.
fn literal_value(e: &Expr) -> Option<i128> {
    let Expr::Raw(text) = e else {
        return None;
    };
    literal_text(text.trim())
}

fn literal_text(s: &str) -> Option<i128> {
    if let Some(rest) = s.strip_prefix('(') {
        // `(int) 65`, `(short) 0x1234`: read the cast, then the literal behind it.
        let end = rest.find(')')?;
        if !matches!(&rest[..end], "int" | "long" | "short" | "byte" | "char") {
            return None;
        }
        return literal_text(rest[end + 1..].trim());
    }
    if let Some(c) = char_literal(s) {
        return Some(c as i128);
    }
    let digits = s.strip_suffix(['L', 'l']).unwrap_or(s).replace('_', "");
    let (radix, body) = if let Some(h) = digits.strip_prefix("0x") {
        (16, h)
    } else if let Some(b) = digits.strip_prefix("0b") {
        (2, b)
    } else if let Some(o) = digits.strip_prefix("0o") {
        (8, o)
    } else {
        // A leading zero is Java's octal only for `0` itself; Rust has no octal prefix.
        (10, digits.as_str())
    };
    i128::from_str_radix(body, radix).ok()
}

fn char_literal(s: &str) -> Option<char> {
    let body = s.strip_prefix('\'')?.strip_suffix('\'')?;
    let mut chars = body.chars();
    match chars.next()? {
        '\\' => match chars.next()? {
            'n' => Some('\n'),
            'r' => Some('\r'),
            't' => Some('\t'),
            '0' => Some('\0'),
            '\'' => Some('\''),
            '\\' => Some('\\'),
            'u' => {
                let hex: String = chars.take(4).collect();
                u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32)
            }
            _ => None,
        },
        c if chars.next().is_none() => Some(c),
        _ => None,
    }
}

// A mask that cannot change the value: a positive literal that already fits the Java signed
// representation, or a value that is masked already.
fn mask_is_noop(v: &Expr, bits: u32) -> bool {
    if let Some(n) = literal_value(v) {
        return n >= 0 && n < (1i128 << (bits - 1));
    }
    matches!(v, Expr::Binary(_, "&", m) if literal_value(m).is_some_and(|m| m >= (1i128 << bits) - 1))
}

// `v & mask`, unless the operand provably already fits.
fn masked(v: Expr, mask: &'static str, bits: u32) -> Expr {
    if mask_is_noop(&v, bits) {
        v
    } else {
        Expr::bin(v, "&", Expr::raw(mask))
    }
}

fn num_from(from: &Ty, to: &Ty, v: Expr) -> Option<Expr> {
    let (from, to) = (from.strip_refs(), to.strip_refs());
    match (from, to) {
        (Ty::Int(a), Ty::Int(b)) => Some(int_convert(*a, *b, v)),
        (Ty::Int(a), Ty::F32) => Some(int_to_float(*a, true, v)),
        (Ty::Int(a), Ty::F64) => Some(int_to_float(*a, false, v)),
        (Ty::F32, Ty::F64) => Some(Expr::Cast("double".into(), Box::new(v))),
        (Ty::Char, Ty::Int(b)) => int_as(Int::U16, *b, v),
        (Ty::Int(a), Ty::Char) => Some(int_to_char(*a, v)),
        (Ty::Bool, Ty::Int(_)) => Some(Expr::Ternary(
            Box::new(v),
            Box::new(Expr::raw("1")),
            Box::new(Expr::raw("0")),
        )),
        _ => None,
    }
}

// Java's byte/short widen with sign extension, so unsigned sources get masked first.
fn int_convert(a: Int, b: Int, v: Expr) -> Expr {
    let to = b.java();
    match a {
        // usize stays signed for readability, like its comparisons and arithmetic.
        Int::I8
        | Int::I16
        | Int::I32
        | Int::Isize
        | Int::I64
        | Int::I128
        | Int::U8
        | Int::Usize => Expr::Cast(to.into(), Box::new(v)),
        Int::U16 => {
            let masked = masked(v, "0xFFFF", 16);
            Expr::Cast(to.into(), Box::new(masked))
        }
        Int::U32 => {
            if b.is_long() {
                let widened = Expr::Cast("long".into(), Box::new(v));
                masked(widened, "0xFFFFFFFFL", 32)
            } else {
                Expr::Cast(to.into(), Box::new(v))
            }
        }
        Int::U64 | Int::U128 => Expr::Cast(to.into(), Box::new(v)),
    }
}

fn int_to_float(a: Int, to_f32: bool, v: Expr) -> Expr {
    let ty = if to_f32 { "float" } else { "double" };
    let cls = if to_f32 { "Float" } else { "Double" };
    match a {
        Int::U16 => {
            let masked = masked(v, "0xFFFF", 16);
            Expr::Cast(ty.into(), Box::new(masked))
        }
        Int::U32 => {
            let widened = Expr::Cast("long".into(), Box::new(v));
            let masked = masked(widened, "0xFFFFFFFFL", 32);
            Expr::Cast(ty.into(), Box::new(masked))
        }
        Int::U64 | Int::U128 => {
            // Parsing the unsigned decimal form rounds the same way as Rust's cast.
            let text = Expr::call(Expr::raw("Long"), "toUnsignedString", vec![v]);
            let parse = format!("parse{}", if to_f32 { "Float" } else { "Double" });
            Expr::call(Expr::raw(cls), parse, vec![text])
        }
        _ => Expr::Cast(ty.into(), Box::new(v)),
    }
}

// TryFrom as `Result`: an `Ok` with the (possibly narrowing) cast or a range check.
fn try_from_num(cx: &mut Cx, from: &Ty, ret: &Ty, v: Expr) -> Option<Expr> {
    let Ty::Result(t, e) = ret.strip_refs() else {
        return None;
    };
    let (Ty::Int(a), Ty::Int(b)) = (from.strip_refs(), t.strip_refs()) else {
        return None;
    };
    cx.needs_result = true;
    let args = format!("{}, {}", t.boxed(), e.boxed());
    let ok = Expr::New {
        ty: format!("Ok<{args}>"),
        args: vec![int_convert(*a, *b, v.clone())],
    };
    let Some(cond) = range_check(*a, *b, &v) else {
        return Some(ok);
    };
    let fail = Expr::New {
        ty: format!("Err<{args}>"),
        args: vec![Expr::New {
            ty: "RuntimeException".into(),
            args: vec![Expr::str_lit(
                "out of range integral type conversion attempted",
            )],
        }],
    };
    Some(conditional(cond, ok, fail))
}

fn int_bounds(i: Int) -> (i128, i128) {
    match i {
        Int::I8 => (-128, 127),
        Int::I16 => (-32768, 32767),
        Int::I32 | Int::Isize => (i32::MIN as i128, i32::MAX as i128),
        Int::I64 | Int::I128 => (i64::MIN as i128, i64::MAX as i128),
        Int::U8 => (0, 255),
        Int::U16 => (0, 65535),
        Int::U32 => (0, u32::MAX as i128),
        Int::Usize => (0, i64::MAX as i128),
        Int::U64 | Int::U128 => (0, u64::MAX as i128),
    }
}

fn bound_lit(n: i128) -> String {
    match n {
        n if n == i32::MAX as i128 => "Integer.MAX_VALUE".into(),
        n if n == i32::MIN as i128 => "Integer.MIN_VALUE".into(),
        n if n == i64::MAX as i128 => "Long.MAX_VALUE".into(),
        n if n == i64::MIN as i128 => "Long.MIN_VALUE".into(),
        n if n == u64::MAX as i128 => "-1L".into(),
        n if n > i32::MAX as i128 || n < i32::MIN as i128 => format!("{n}L"),
        n => n.to_string(),
    }
}

// A Java condition that the source value fits the target range; None when it always does.
fn range_check(a: Int, b: Int, v: &Expr) -> Option<Expr> {
    let (slo, shi) = int_bounds(a);
    let (tlo, thi) = int_bounds(b);
    if tlo <= slo && shi <= thi {
        return None;
    }
    let unsigned_wide = matches!(a, Int::U32 | Int::Usize);
    let unsigned_long = matches!(a, Int::U64 | Int::U128);
    let mut conds = Vec::new();
    if unsigned_wide || unsigned_long {
        let cls = if unsigned_long { "Long" } else { "Integer" };
        if tlo > 0 {
            conds.push(Expr::bin(
                Expr::call(
                    Expr::raw(cls),
                    "compareUnsigned",
                    vec![v.clone(), Expr::raw(bound_lit(tlo))],
                ),
                ">=",
                Expr::raw("0"),
            ));
        }
        if thi < shi {
            conds.push(Expr::bin(
                Expr::call(
                    Expr::raw(cls),
                    "compareUnsigned",
                    vec![v.clone(), Expr::raw(bound_lit(thi))],
                ),
                "<=",
                Expr::raw("0"),
            ));
        }
    } else {
        let base = if a == Int::U16 {
            Expr::bin(v.clone(), "&", Expr::raw("0xFFFF"))
        } else {
            v.clone()
        };
        if tlo > slo {
            conds.push(Expr::bin(base.clone(), ">=", Expr::raw(bound_lit(tlo))));
        }
        if thi < shi {
            conds.push(Expr::bin(base, "<=", Expr::raw(bound_lit(thi))));
        }
    }
    conds.into_iter().reduce(|a, b| Expr::bin(a, "&&", b))
}

// `as` casts: truncation/extension for integers, saturation for float to integer.
pub(crate) fn num_cast(from: &Ty, to: &Ty, v: Expr) -> Option<Expr> {
    match (from.strip_refs(), to.strip_refs()) {
        (Ty::Int(a), Ty::Int(b)) => int_as(*a, *b, v),
        (Ty::Int(a), Ty::F32) => Some(int_to_float(*a, true, v)),
        (Ty::Int(a), Ty::F64) => Some(int_to_float(*a, false, v)),
        (Ty::F32 | Ty::F64, Ty::Int(b)) => Some(float_as(*b, v)),
        (Ty::F32, Ty::F64) => Some(Expr::Cast("double".into(), Box::new(v))),
        (Ty::F64, Ty::F32) => Some(Expr::Cast("float".into(), Box::new(v))),
        (Ty::Char, Ty::Int(b)) => int_as(Int::U16, *b, v),
        (Ty::Int(a), Ty::Char) => Some(int_to_char(*a, v)),
        (Ty::Bool, Ty::Int(_)) => Some(Expr::Ternary(
            Box::new(v),
            Box::new(Expr::raw("1")),
            Box::new(Expr::raw("0")),
        )),
        _ => None,
    }
}

fn int_as(a: Int, b: Int, v: Expr) -> Option<Expr> {
    if a.java() == b.java() {
        // Same Java type: only `as u8` still has to drop the upper bits.
        return match b {
            Int::U8 if a != Int::U8 => Some(masked(v, "0xFF", 8)),
            _ => None,
        };
    }
    Some(match b {
        Int::U8 => {
            let masked = masked(v, "0xFF", 8);
            if a.is_long() {
                Expr::Cast("int".into(), Box::new(masked))
            } else {
                char_code(masked)
            }
        }
        Int::I8 => Expr::Cast("byte".into(), Box::new(v)),
        Int::I16 | Int::U16 => Expr::Cast(b.java().into(), Box::new(v)),
        Int::I32 | Int::U32 | Int::Isize | Int::Usize => {
            if a == Int::U16 {
                char_code(masked(v, "0xFFFF", 16))
            } else {
                Expr::Cast("int".into(), Box::new(v))
            }
        }
        Int::I64 | Int::U64 | Int::I128 | Int::U128 => match a {
            Int::U16 => Expr::Cast("long".into(), Box::new(masked(v, "0xFFFF", 16))),
            Int::U32 | Int::Usize => {
                masked(Expr::Cast("long".into(), Box::new(v)), "0xFFFFFFFFL", 32)
            }
            _ => Expr::Cast("long".into(), Box::new(v)),
        },
    })
}

// A char literal stays an `int` value even in generic positions.
fn char_code(e: Expr) -> Expr {
    if matches!(&e, Expr::Raw(s) if s.starts_with('\'')) {
        Expr::Cast("int".into(), Box::new(e))
    } else {
        e
    }
}

// `as char` from `u8`: a literal is already the code point; anything else may be a boxed Integer.
fn int_to_char(a: Int, v: Expr) -> Expr {
    if a == Int::U8 && matches!(&v, Expr::Raw(_)) {
        return Expr::Cast("char".into(), Box::new(v));
    }
    let code = masked(v, "0xFF", 8);
    let code = if a.is_long() {
        Expr::Cast("int".into(), Box::new(code))
    } else {
        code
    };
    Expr::Cast("char".into(), Box::new(code))
}

// `as` from a float saturates (NaN becomes 0); Java only saturates to the signed bounds.
fn float_as(b: Int, v: Expr) -> Expr {
    let clamp = |lo: &str, hi: &str| {
        Expr::call(
            Expr::raw("Math"),
            "max",
            vec![
                Expr::raw(lo),
                Expr::call(Expr::raw("Math"), "min", vec![Expr::raw(hi), v.clone()]),
            ],
        )
    };
    match b {
        Int::I32 | Int::Isize => Expr::Cast("int".into(), Box::new(v)),
        Int::I64 | Int::I128 => Expr::Cast("long".into(), Box::new(v)),
        Int::U8 => Expr::Cast("int".into(), Box::new(clamp("0.0", "255.0"))),
        Int::U16 => Expr::Cast("short".into(), Box::new(clamp("0.0", "65535.0"))),
        Int::I8 => Expr::Cast("byte".into(), Box::new(clamp("-128.0", "127.0"))),
        Int::I16 => Expr::Cast("short".into(), Box::new(clamp("-32768.0", "32767.0"))),
        Int::U32 | Int::Usize => Expr::Cast(
            "int".into(),
            Box::new(Expr::Cast(
                "long".into(),
                Box::new(clamp("0.0", "4294967295.0")),
            )),
        ),
        Int::U64 | Int::U128 => Expr::Cast(
            "long".into(),
            Box::new(clamp("0.0", "1.8446744073709552E19")),
        ),
    }
}

// `/` and `%` on unsigned types need Java's unsigned helpers.
pub(crate) fn unsigned_arith(ty: &Ty, op: ast::ArithOp, a: Expr, b: Expr) -> Option<Expr> {
    let Ty::Int(i) = ty.strip_refs() else {
        return None;
    };
    // `usize`/`u8` are Java ints that never exceed the signed range in practice: keep `/` and `%`.
    if !i.is_unsigned() || matches!(i, Int::Usize | Int::U8) {
        return None;
    }
    match op {
        ast::ArithOp::Div => Some(unsigned_div_rem(*i, a, b, false)),
        ast::ArithOp::Rem => Some(unsigned_div_rem(*i, a, b, true)),
        _ => None,
    }
}

// Whether Display/Debug of this integer type differs from Java's signed rendering.
pub(crate) fn unsigned_display(i: Int) -> bool {
    // `usize` is a Java `int`, so its value is already what Rust prints.
    matches!(i, Int::U16 | Int::U32 | Int::U64 | Int::U128)
}

// Unsigned literals above the signed range need a cast (u32) or two's-complement digits (u64).
pub(crate) fn unsigned_literal(i: Int, digits: &str) -> Option<String> {
    let (radix, body) = if let Some(h) = digits.strip_prefix("0x") {
        (16, h)
    } else if let Some(b) = digits.strip_prefix("0b") {
        (2, b)
    } else if let Some(o) = digits.strip_prefix("0o") {
        (8, o)
    } else {
        (10, digits)
    };
    let value = u128::from_str_radix(&body.replace('_', ""), radix).ok()?;
    match i {
        // u16 lives in a Java `short`, where only 0..=32767 is a constant expression.
        Int::U16 if value > i16::MAX as u128 => Some(format!("(short) {digits}")),
        Int::U32 | Int::Usize if value > i32::MAX as u128 => Some(format!("(int) {digits}L")),
        Int::U64 | Int::U128 if value > i64::MAX as u128 => Some(format!("{}L", value as i64)),
        _ => None,
    }
}

// Rust formats unsigned types as such; Java's own `toString` prints them signed.
pub(crate) fn unsigned_string(i: Int, v: Expr) -> Option<Expr> {
    match i {
        Int::U16 => {
            let masked = Expr::bin(v, "&", Expr::raw("0xFFFF"));
            Some(Expr::call(Expr::raw("String"), "valueOf", vec![masked]))
        }
        Int::U32 => Some(Expr::call(
            Expr::raw("Integer"),
            "toUnsignedString",
            vec![v],
        )),
        Int::U64 | Int::U128 => Some(Expr::call(Expr::raw("Long"), "toUnsignedString", vec![v])),
        _ => None,
    }
}

// Comparison of two values of an integer or `Ordering` type, as a Java int. `None` when Java's
// own operators already give the right answer.
pub(crate) fn compare_expr(ty: &Ty, a: Expr, b: Expr) -> Option<Expr> {
    match ty.strip_refs() {
        Ty::Ordering => Some(Expr::call(
            Expr::raw("Integer"),
            "compare",
            vec![signum(a), signum(b)],
        )),
        // `usize` stays signed like its Display, so `i < v.len()` reads naturally.
        Ty::Int(i) if i.is_unsigned() && !matches!(i, Int::U8 | Int::Usize) => {
            Some(unsigned_compare(*i, a, b))
        }
        _ => None,
    }
}

// `Integer.signum`, dropped when the operand already yields only -1/0/1.
pub(crate) fn signum(e: Expr) -> Expr {
    let normalized = match &e {
        Expr::Call {
            recv: Some(recv),
            name,
            ..
        } => {
            matches!(name.as_str(), "compare" | "compareUnsigned" | "signum")
                && matches!(&**recv, Expr::Raw(s) if is_compare_class(s))
        }
        Expr::Call { name, .. } => {
            let (cls, op) = name.split_once('.').unwrap_or(("", name));
            is_compare_class(cls) && matches!(op, "compare" | "compareUnsigned" | "signum")
        }
        _ => false,
    };
    if normalized {
        e
    } else {
        Expr::call(Expr::raw("Integer"), "signum", vec![e])
    }
}

fn is_compare_class(s: &str) -> bool {
    matches!(
        s,
        "Integer" | "Long" | "Short" | "Byte" | "Double" | "Float" | "Boolean" | "Character"
    )
}

fn unsigned_compare(i: Int, a: Expr, b: Expr) -> Expr {
    match i {
        Int::U16 => Expr::call(
            Expr::raw("Integer"),
            "compare",
            vec![masked(a, "0xFFFF", 16), masked(b, "0xFFFF", 16)],
        ),
        Int::U32 | Int::Usize => Expr::call(Expr::raw("Integer"), "compareUnsigned", vec![a, b]),
        _ => Expr::call(Expr::raw("Long"), "compareUnsigned", vec![a, b]),
    }
}

// ---- integers --------------------------------------------------------------------------------

pub(super) fn lower_int_method(
    cx: &mut Cx,
    recv: Expr,
    args: Vec<Expr>,
    i: Int,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let long = i.is_long();
    let unsigned = i.is_unsigned();
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0"));
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    let bits =
        |f: &str, a: Vec<Expr>| Expr::call(Expr::raw(if long { "Long" } else { "Integer" }), f, a);
    let width = bit_width(i);
    match name {
        "abs" => {
            if unsigned {
                Some(narrow(recv, i))
            } else {
                Some(narrow(math("abs", vec![recv]), i))
            }
        }
        "signum" => Some(if unsigned {
            narrow(
                Expr::Ternary(
                    Box::new(Expr::bin(recv, "==", Expr::raw("0"))),
                    Box::new(Expr::raw("0")),
                    Box::new(Expr::raw("1")),
                ),
                i,
            )
        } else {
            narrow(
                Expr::call(
                    Expr::raw(if long { "Long" } else { "Integer" }),
                    "signum",
                    vec![recv],
                ),
                i,
            )
        }),
        // A double is exact for 32-bit results; 64-bit ones wrap exactly through BigInteger.
        "pow" if !i.is_long() => Some(Expr::Cast(
            i.java().into(),
            Box::new(math("pow", vec![recv, b0])),
        )),
        "pow" | "wrapping_pow" => Some(match (literal_int(&recv), literal_int(&b0)) {
            (Some(a), Some(e)) if e <= 64 => Expr::raw(fold_pow(a, e as u32, i)),
            _ => narrow(big_pow(cx, recv, b0, i.is_long()), i),
        }),
        "checked_pow" => {
            let width = if matches!(i, Int::I8 | Int::U8) {
                8
            } else if matches!(i, Int::I16 | Int::U16) {
                16
            } else if i.is_long() {
                64
            } else {
                32
            };
            let bits = if i.is_unsigned() { width } else { width - 1 };
            let r = big_pow(cx, recv.clone(), b0.clone(), i.is_long());
            let big = Expr::call(Expr::raw("BigInteger"), "valueOf", vec![recv]);
            let pow = Expr::call(big, "pow", vec![b0]);
            let in_bits = Expr::bin(
                Expr::call(pow.clone(), "bitLength", vec![]),
                "<=",
                Expr::raw(bits.to_string()),
            );
            let fits = if i.is_unsigned() {
                Expr::bin(
                    Expr::bin(Expr::call(pow, "signum", vec![]), ">=", Expr::raw("0")),
                    "&&",
                    in_bits,
                )
            } else {
                in_bits
            };
            Some(conditional(
                fits,
                opt_of(narrow(r, i)),
                opt_empty(opt_inner(ret).as_ref()),
            ))
        }
        "min" | "max" => Some(min_max(cx, recv, b0, i, name == "max", line, out)),
        "is_multiple_of" => {
            let rem = if matches!(i, Int::U32 | Int::U64 | Int::U128) {
                unsigned_div_rem(i, recv.clone(), b0.clone(), true)
            } else {
                Expr::bin(recv.clone(), "%", b0.clone())
            };
            let test = Expr::bin(rem, "==", Expr::raw("0"));
            let nonzero_lit = matches!(&b0, Expr::Raw(t) if t.parse::<u64>().is_ok_and(|v| v != 0));
            Some(if nonzero_lit {
                test
            } else {
                // `x.is_multiple_of(0)` is `x == 0` in Rust; Java's `%` would throw.
                Expr::Ternary(
                    Box::new(Expr::bin(b0, "==", Expr::raw("0"))),
                    Box::new(Expr::bin(recv, "==", Expr::raw("0"))),
                    Box::new(test),
                )
            })
        }
        "clamp" => Some(clamp(cx, recv, b0, b1, i, line, out)),
        "rem_euclid" => Some(if unsigned {
            let rem = unsigned_div_rem(i, recv, b0, true);
            narrow(rem, i)
        } else {
            narrow(math("floorMod", vec![recv, math("abs", vec![b0])]), i)
        }),
        "div_euclid" => Some(if unsigned {
            narrow(unsigned_div_rem(i, recv, b0, false), i)
        } else {
            // Rust rounds by the remainder, so a negative divisor differs from `Math.floorDiv`.
            let (a, b) = (
                simple(cx, recv, &Ty::Int(i), line, out),
                simple(cx, b0, &Ty::Int(i), line, out),
            );
            let (ar, br) = (render(&a), render(&b));
            narrow(
                Expr::raw(format!(
                    "(({ar} - Math.floorMod({ar}, Math.abs({br}))) / {br})"
                )),
                i,
            )
        }),
        "div_ceil" => Some(narrow(
            Expr::bin(
                Expr::bin(Expr::bin(recv, "+", b0.clone()), "-", Expr::raw("1")),
                "/",
                b0,
            ),
            i,
        )),
        "count_ones" => Some(bits("bitCount", vec![mask_width(i, recv)])),
        "count_zeros" => Some(Expr::bin(
            Expr::raw(width.to_string()),
            "-",
            bits("bitCount", vec![mask_width(i, recv)]),
        )),
        "leading_zeros" => {
            let zeros = bits("numberOfLeadingZeros", vec![mask_width(i, recv)]);
            Some(if width == 32 || width == 64 {
                zeros
            } else {
                Expr::bin(zeros, "-", Expr::raw((32 - width).to_string()))
            })
        }
        "ilog2" => Some(Expr::bin(
            Expr::raw((if width == 64 { 63 } else { 31 }).to_string()),
            "-",
            bits("numberOfLeadingZeros", vec![mask_width(i, recv)]),
        )),
        // Digit count, exact where `Math.log10` rounds up just below a power of ten.
        "ilog10" => {
            let text = if i.is_unsigned() && width == 64 {
                Expr::call(Expr::raw("Long"), "toUnsignedString", vec![recv])
            } else {
                Expr::call(Expr::raw("String"), "valueOf", vec![recv])
            };
            Some(Expr::bin(
                Expr::call(text, "length", vec![]),
                "-",
                Expr::raw("1"),
            ))
        }
        "trailing_zeros" => {
            let zeros = bits("numberOfTrailingZeros", vec![mask_width(i, recv)]);
            Some(if width == 32 || width == 64 {
                zeros
            } else {
                math("min", vec![zeros, Expr::raw(width.to_string())])
            })
        }
        "swap_bytes" => Some(match width {
            8 => narrow(recv, i),
            16 => Expr::call(
                Expr::raw("Short"),
                "reverseBytes",
                vec![Expr::Cast("short".into(), Box::new(recv))],
            ),
            _ => bits("reverseBytes", vec![recv]),
        }),
        "reverse_bits" => Some(narrow(
            match width {
                8 => Expr::bin(bits("reverse", vec![recv]), ">>>", Expr::raw("24")),
                16 => Expr::bin(bits("reverse", vec![recv]), ">>>", Expr::raw("16")),
                _ => bits("reverse", vec![recv]),
            },
            i,
        )),
        "rotate_left" | "rotate_right" => Some(rotate(
            cx,
            recv,
            b0,
            i,
            width,
            name == "rotate_left",
            line,
            out,
        )),
        "to_be_bytes" | "to_le_bytes" | "to_ne_bytes" => {
            // `[u8; N]` is a list of 0..=255 ints, most significant first for big-endian.
            let x = super::simple(cx, recv, &Ty::Int(i), line, out);
            let n = width / 8;
            let bytes: Vec<Expr> = (0..n)
                .map(|k| {
                    let shift = if name == "to_be_bytes" {
                        8 * (n - 1 - k)
                    } else {
                        8 * k
                    };
                    let shifted = if shift == 0 {
                        x.clone()
                    } else {
                        Expr::bin(x.clone(), ">>>", Expr::raw(shift.to_string()))
                    };
                    let byte = Expr::bin(shifted, "&", Expr::raw("0xFF"));
                    if long {
                        Expr::Cast("int".into(), Box::new(byte))
                    } else {
                        byte
                    }
                })
                .collect();
            let list = Expr::call(Expr::raw("List"), "of", bytes);
            Some(Expr::New {
                ty: "ArrayList<>".into(),
                args: vec![list],
            })
        }
        "next_power_of_two" => {
            let x = super::simple(cx, recv, &Ty::Int(i), line, out);
            let xs = render(&x);
            Some(narrow(
                Expr::raw(format!(
                    "({xs} <= 1 ? 1 : {}.highestOneBit({xs} - 1) << 1)",
                    if long { "Long" } else { "Integer" }
                )),
                i,
            ))
        }
        "is_power_of_two" => {
            let nonzero = if unsigned {
                Expr::bin(recv.clone(), "!=", Expr::raw("0"))
            } else {
                Expr::bin(recv.clone(), ">", Expr::raw("0"))
            };
            Some(Expr::bin(
                Expr::bin(
                    Expr::bin(recv.clone(), "&", Expr::bin(recv, "-", Expr::raw("1"))),
                    "==",
                    Expr::raw("0"),
                ),
                "&&",
                nonzero,
            ))
        }
        "is_positive" => Some(if unsigned {
            Expr::bin(recv, "!=", Expr::raw("0"))
        } else {
            Expr::bin(recv, ">", Expr::raw("0"))
        }),
        "is_negative" => Some(if unsigned {
            Expr::raw("false")
        } else {
            Expr::bin(recv, "<", Expr::raw("0"))
        }),
        "cmp" => Some(compare_int(recv, b0, i)),
        "partial_cmp" => Some(opt_of(compare_int(recv, b0, i))),
        "to_string" => Some(
            unsigned_string(i, recv.clone())
                .unwrap_or_else(|| Expr::call(Expr::raw("String"), "valueOf", vec![recv])),
        ),
        "clone" | "to_owned" => Some(recv),
        "abs_diff" => {
            let (a, b) = (
                simple(cx, recv, &Ty::Int(i), line, out),
                simple(cx, b0, &Ty::Int(i), line, out),
            );
            let cond = if unsigned {
                Expr::bin(
                    unsigned_compare(i, a.clone(), b.clone()),
                    ">=",
                    Expr::raw("0"),
                )
            } else {
                Expr::bin(a.clone(), ">=", b.clone())
            };
            let diff = Expr::Ternary(
                Box::new(cond),
                Box::new(Expr::bin(a.clone(), "-", b.clone())),
                Box::new(Expr::bin(b, "-", a)),
            );
            // abs_diff returns the same-width unsigned type, which is `short` only at 16 bits.
            Some(if matches!(i, Int::I16 | Int::U16) {
                Expr::Cast("short".into(), Box::new(diff))
            } else {
                diff
            })
        }
        // `|x|` always fits the unsigned type: Java's `Math.abs(MIN)` is MIN, i.e. 2^(n-1) unsigned.
        "unsigned_abs" => Some(Expr::call(Expr::raw("Math"), "abs", vec![recv])),
        // `(wrapped, overflowed)`: the wrapping result, and whether the checked one is None.
        "overflowing_add" | "overflowing_sub" | "overflowing_mul" => {
            let op = &name["overflowing_".len()..];
            let (a, b) = (
                simple(cx, recv, &Ty::Int(i), line, out),
                simple(cx, b0, &Ty::Int(i), line, out),
            );
            let wrapped = wrapping_int(a.clone(), b.clone(), i, &format!("wrapping_{op}"));
            let opt = Ty::Option(Box::new(Ty::Int(i)));
            let checked = checked_int(cx, a, b, b1, i, &format!("checked_{op}"), &opt, line, out);
            Some(Expr::call(
                Expr::raw("Map"),
                "entry",
                vec![wrapped, Expr::call(checked, "isEmpty", vec![])],
            ))
        }
        // Only the variants these helpers implement; anything else is reported, not guessed.
        "checked_add" | "checked_sub" | "checked_mul" | "checked_div" | "checked_rem"
        | "checked_neg" | "checked_abs" => {
            Some(checked_int(cx, recv, b0, b1, i, name, ret, line, out))
        }
        "saturating_add" | "saturating_sub" | "saturating_mul" | "saturating_abs"
        | "saturating_neg" => {
            // Operands appear more than once in the overflow tests.
            let recv = simple(cx, recv, &Ty::Int(i), line, out);
            let b0 = simple(cx, b0, &Ty::Int(i), line, out);
            Some(saturating_int(recv, b0, i, name))
        }
        "wrapping_add" | "wrapping_sub" | "wrapping_mul" | "wrapping_div" | "wrapping_rem"
        | "wrapping_neg" | "wrapping_shl" | "wrapping_shr" | "wrapping_abs" => {
            Some(wrapping_int(recv, b0, i, name))
        }
        _ => None,
    }
}

fn bit_width(i: Int) -> u32 {
    match i {
        Int::I8 | Int::U8 => 8,
        Int::I16 | Int::U16 => 16,
        Int::I32 | Int::U32 => 32,
        _ => 64,
    }
}

// `u32::from_be_bytes(b)`: the list folded most significant byte first.
pub(super) fn from_bytes(key: &str, bytes: Expr) -> Option<Expr> {
    let (owner, name) = key.split_once("::")?;
    let i = Int::from_name(owner)?;
    let src = if name == "from_le_bytes" {
        Expr::call(bytes, "reversed", vec![])
    } else {
        bytes
    };
    let (map, unit) = if i.is_long() {
        ("mapToLong", "0L")
    } else {
        ("mapToInt", "0")
    };
    let folded = Expr::raw(format!(
        "{}.stream().{map}(b -> b).reduce({unit}, (a, b) -> a << 8 | b)",
        render(&src)
    ));
    Some(narrow(folded, i))
}

// Java's bit helpers always work on 32/64 bits; mask narrow values and their sign bits away.
fn mask_width(i: Int, v: Expr) -> Expr {
    match i {
        Int::I8 => masked(v, "0xFF", 8),
        Int::I16 | Int::U16 => masked(v, "0xFFFF", 16),
        _ => v,
    }
}

fn mask_for(i: Int) -> &'static str {
    match i {
        Int::I8 => "0xFF",
        Int::I16 | Int::U16 => "0xFFFF",
        _ => "",
    }
}

fn compare_int(a: Expr, b: Expr, i: Int) -> Expr {
    if i.is_unsigned() && !matches!(i, Int::U8) {
        return unsigned_compare(i, a, b);
    }
    Expr::call(
        Expr::raw(if i.is_long() { "Long" } else { "Integer" }),
        "compare",
        vec![a, b],
    )
}

pub(super) fn min_max(
    cx: &mut Cx,
    a: Expr,
    b: Expr,
    i: Int,
    max: bool,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    if i.is_unsigned() && !matches!(i, Int::U8) {
        let (a, b) = (
            simple(cx, a, &Ty::Int(i), line, out),
            simple(cx, b, &Ty::Int(i), line, out),
        );
        let cond = Expr::bin(
            unsigned_compare(i, a.clone(), b.clone()),
            if max { ">=" } else { "<=" },
            Expr::raw("0"),
        );
        return narrow(conditional(cond, a, b), i);
    }
    narrow(
        Expr::call(
            Expr::raw("Math"),
            if max { "max" } else { "min" },
            vec![a, b],
        ),
        i,
    )
}

// `x.clamp(lo, hi)` is `lo.max(hi.min(x))`.
fn clamp(
    cx: &mut Cx,
    recv: Expr,
    lo: Expr,
    hi: Expr,
    i: Int,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    if i.is_unsigned() && !matches!(i, Int::U8) {
        let (lo, hi) = (
            simple(cx, lo, &Ty::Int(i), line, out),
            simple(cx, hi, &Ty::Int(i), line, out),
        );
        let x = simple(cx, recv, &Ty::Int(i), line, out);
        let upper = conditional(
            Expr::bin(
                unsigned_compare(i, x.clone(), hi.clone()),
                "<=",
                Expr::raw("0"),
            ),
            x,
            hi,
        );
        let upper = simple(cx, upper, &Ty::Int(i), line, out);
        let lower = conditional(
            Expr::bin(
                unsigned_compare(i, upper.clone(), lo.clone()),
                ">=",
                Expr::raw("0"),
            ),
            upper,
            lo,
        );
        return narrow(lower, i);
    }
    let math = |f: &str, a: Vec<Expr>| Expr::call(Expr::raw("Math"), f, a);
    if !i.is_long() {
        // Java 21's clamp also evaluates the receiver first, like Rust.
        return narrow(math("clamp", vec![recv, lo, hi]), i);
    }
    narrow(math("max", vec![lo, math("min", vec![hi, recv])]), i)
}

// `/` and `%` for unsigned types, where the Java value is the same bit pattern.
fn unsigned_div_rem(i: Int, a: Expr, b: Expr, rem: bool) -> Expr {
    match i {
        Int::U32 | Int::Usize => Expr::call(
            Expr::raw("Integer"),
            if rem {
                "remainderUnsigned"
            } else {
                "divideUnsigned"
            },
            vec![a, b],
        ),
        Int::U64 | Int::U128 => Expr::call(
            Expr::raw("Long"),
            if rem {
                "remainderUnsigned"
            } else {
                "divideUnsigned"
            },
            vec![a, b],
        ),
        // u8 values are plain Java ints in 0..255, so they need no reinterpretation.
        Int::U8 => Expr::bin(a, if rem { "%" } else { "/" }, b),
        _ => {
            let (a, b) = (masked(a, "0xFFFF", 16), masked(b, "0xFFFF", 16));
            Expr::bin(a, if rem { "%" } else { "/" }, b)
        }
    }
}

// Rotation is defined on the type's own width; Java's rotates use the Java width.
fn rotate(
    cx: &mut Cx,
    recv: Expr,
    b: Expr,
    i: Int,
    width: u32,
    left: bool,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    if width == 32 || width == 64 {
        let cls = if i.is_long() { "Long" } else { "Integer" };
        return Expr::call(
            Expr::raw(cls),
            if left { "rotateLeft" } else { "rotateRight" },
            vec![recv, b],
        );
    }
    let v = simple(cx, recv, &Ty::Int(i), line, out);
    let k = simple(cx, b, &Ty::Int(Int::U32), line, out);
    let mask = mask_for(i);
    let shift = width - 1;
    let vr = render(&v);
    let kr = render(&k);
    let rotated = if left {
        format!(
            "(({vr} & {mask}) << ({kr} & {shift}) | ({vr} & {mask}) >>> ({width} - ({kr} & {shift})))"
        )
    } else {
        format!(
            "(({vr} & {mask}) >>> ({kr} & {shift}) | ({vr} & {mask}) << ({width} - ({kr} & {shift})))"
        )
    };
    narrow(Expr::raw(format!("({rotated} & {mask})")), i)
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
    // Unsigned values are signed Java numbers; widen them without sign extension.
    let wide = |e: Expr| unsigned_wide(i, e);
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
                    Expr::bin(wide(recv.clone()), "+", wide(b0.clone())),
                    "<=",
                    Expr::raw("65535L"),
                ),
                yes(Expr::bin(recv, "+", b0)),
                empty,
            ),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(
                    Expr::bin(wide(recv.clone()), "+", wide(b0.clone())),
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
                Expr::bin(
                    unsigned_compare(i, recv.clone(), b0.clone()),
                    ">=",
                    Expr::raw("0"),
                ),
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
                    Expr::bin(wide(recv.clone()), "*", wide(b0.clone())),
                    "<=",
                    Expr::raw("65535L"),
                ),
                yes(Expr::Cast(
                    "int".into(),
                    Box::new(Expr::bin(wide(recv), "*", wide(b0))),
                )),
                empty,
            ),
            Int::U32 | Int::Usize => conditional(
                Expr::bin(
                    Expr::bin(wide(recv.clone()), "*", wide(b0.clone())),
                    "<=",
                    Expr::raw("4294967295L"),
                ),
                yes(Expr::Cast(
                    "int".into(),
                    Box::new(Expr::bin(wide(recv), "*", wide(b0))),
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
            let value = if uint {
                unsigned_div_rem(i, recv, b0, false)
            } else {
                Expr::bin(recv, "/", b0)
            };
            conditional(cond, yes(value), empty)
        }
        "checked_rem" => {
            let value = if uint {
                unsigned_div_rem(i, recv, b0.clone(), true)
            } else {
                Expr::bin(recv, "%", b0.clone())
            };
            conditional(Expr::bin(b0, "!=", Expr::raw("0")), yes(value), empty)
        }
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

fn int_cast(v: Expr) -> Expr {
    Expr::Cast("int".into(), Box::new(v))
}

// Widening of an unsigned 32-bit value to long without sign extension.
fn unsigned_wide(i: Int, e: Expr) -> Expr {
    let mask = match i {
        Int::U32 | Int::Usize => "0xFFFFFFFFL",
        Int::U16 => "0xFFFFL",
        _ => return Expr::Cast("long".into(), Box::new(e)),
    };
    Expr::bin(Expr::Cast("long".into(), Box::new(e)), "&", Expr::raw(mask))
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
                        Expr::bin(unsigned_wide(i, recv), "+", unsigned_wide(i, b)),
                        Expr::raw("65535L"),
                    ],
                ),
                i,
            ),
            Int::U32 | Int::Usize => narrow(
                int_cast(math(
                    "min",
                    vec![
                        Expr::bin(unsigned_wide(i, recv), "+", unsigned_wide(i, b)),
                        Expr::raw("4294967295L"),
                    ],
                )),
                i,
            ),
            Int::I8 | Int::I16 => narrow(
                math("min", vec![Expr::raw(int_max(i)), Expr::bin(recv, "+", b)]),
                i,
            ),
            Int::I32 => narrow(
                math(
                    "clamp",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "+", b),
                        Expr::raw("Integer.MIN_VALUE"),
                        Expr::raw("Integer.MAX_VALUE"),
                    ],
                ),
                i,
            ),
            _ => saturating_wide(recv, b, i, "add"),
        },
        "saturating_sub" => match i {
            Int::U8 | Int::U16 | Int::U32 | Int::Usize => {
                if matches!(i, Int::U32 | Int::Usize) {
                    int_cast(conditional(
                        Expr::bin(
                            unsigned_compare(i, recv.clone(), b.clone()),
                            ">=",
                            Expr::raw("0"),
                        ),
                        Expr::bin(recv, "-", b),
                        Expr::raw("0"),
                    ))
                } else if i == Int::U16 {
                    // A u16 is a signed Java `short`: compare the unsigned values.
                    narrow(
                        math(
                            "max",
                            vec![
                                Expr::bin(unsigned_wide(i, recv), "-", unsigned_wide(i, b)),
                                Expr::raw("0L"),
                            ],
                        ),
                        i,
                    )
                } else {
                    narrow(
                        math("max", vec![Expr::bin(recv, "-", b), Expr::raw("0")]),
                        i,
                    )
                }
            }
            Int::I8 | Int::I16 => narrow(
                math("max", vec![Expr::raw(int_min(i)), Expr::bin(recv, "-", b)]),
                i,
            ),
            Int::I32 => narrow(
                int_cast(math(
                    "max",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "-", b),
                        Expr::raw("Integer.MIN_VALUE"),
                    ],
                )),
                i,
            ),
            _ => saturating_wide(recv, b, i, "sub"),
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
                        Expr::bin(unsigned_wide(i, recv), "*", unsigned_wide(i, b)),
                        Expr::raw("65535L"),
                    ],
                ),
                i,
            ),
            Int::U32 | Int::Usize => narrow(
                int_cast(math(
                    "min",
                    vec![
                        Expr::bin(unsigned_wide(i, recv), "*", unsigned_wide(i, b)),
                        Expr::raw("4294967295L"),
                    ],
                )),
                i,
            ),
            Int::I32 => narrow(
                math(
                    "clamp",
                    vec![
                        Expr::bin(Expr::Cast("long".into(), Box::new(recv)), "*", b),
                        Expr::raw("Integer.MIN_VALUE"),
                        Expr::raw("Integer.MAX_VALUE"),
                    ],
                ),
                i,
            ),
            _ => saturating_wide(recv, b, i, "mul"),
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
    let wrap = |v: Expr| -> Expr {
        if matches!(i, Int::U8) {
            Expr::bin(v, "&", Expr::raw("0xFF"))
        } else {
            narrow(v, i)
        }
    };
    match name {
        "wrapping_add" => wrap(Expr::bin(recv, "+", b)),
        "wrapping_sub" => wrap(Expr::bin(recv, "-", b)),
        "wrapping_mul" => wrap(Expr::bin(recv, "*", b)),
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
    recv: Expr,
    args: Vec<Expr>,
    f32: bool,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = ret;
    let cls = if f32 { "Float" } else { "Double" };
    let fty = if f32 { Ty::F32 } else { Ty::F64 };
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
        // Java's `%` keeps the dividend's sign, like `x - x.trunc()`.
        "fract" => Expr::bin(recv, "%", Expr::raw(if f32 { "1.0f" } else { "1.0" })),
        "round" => {
            let s = simple(cx, recv, &fty, line, out);
            let r = render(&s);
            cast(Expr::raw(format!(
                "Math.copySign(Math.floor(Math.abs({r}) + 0.5), {r})"
            )))
        }
        "powi" | "powf" => cast(math("pow", vec![recv, b0])),
        "mul_add" => Expr::bin(Expr::bin(recv, "*", b0), "+", b1),
        "rem_euclid" => {
            let s = simple(cx, recv, &fty, line, out);
            let r = render(&s);
            let b = render(&b0);
            Expr::raw(format!(
                "({r} - Math.floor({r} / Math.abs({b})) * Math.abs({b}))"
            ))
        }
        "is_nan" => Expr::call(Expr::raw(cls), "isNaN", vec![recv]),
        "is_infinite" => Expr::call(Expr::raw(cls), "isInfinite", vec![recv]),
        "is_finite" => Expr::call(Expr::raw(cls), "isFinite", vec![recv]),
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
        "clamp" => cast(math("clamp", vec![recv, b0, b1])),
        "cmp" | "total_cmp" => Expr::call(Expr::raw(cls), "compare", vec![recv, b0]),
        "partial_cmp" => {
            let a = simple(cx, recv, &fty, line, out);
            let b = simple(cx, b0, &fty, line, out);
            let (ar, br) = (render(&a), render(&b));
            // Rust returns None when either side is NaN; Java's compare has a total order.
            Expr::raw(format!(
                "({cls}.isNaN({ar}) || {cls}.isNaN({br}) ? Optional.<Integer>empty() : Optional.of({cls}.compare({ar}, {br})))"
            ))
        }
        "to_string" => crate::lower::fmt::display_float(cx, recv, fty == Ty::F32),
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

fn literal_int(e: &Expr) -> Option<i128> {
    match e {
        Expr::Raw(s) => s.trim_end_matches(['L', 'l']).replace('_', "").parse().ok(),
        _ => None,
    }
}

// `10i64.pow(12)`: the wrapped constant itself.
fn fold_pow(a: i128, e: u32, i: Int) -> String {
    let mut r: u128 = 1;
    for _ in 0..e {
        r = r.wrapping_mul(a as u128);
    }
    // The bits of the Java representation: `u8` is a 0..=255 int, the rest are two's complement.
    let width = match i {
        Int::I8 | Int::U8 => 8,
        Int::I16 | Int::U16 => 16,
        _ if i.is_long() => 64,
        _ => 32,
    };
    let bits = (r & ((1u128 << width) - 1)) as i128;
    let v = if i != Int::U8 && bits >= 1i128 << (width - 1) {
        bits - (1i128 << width)
    } else {
        bits
    };
    if i.is_long() {
        format!("{v}L")
    } else {
        v.to_string()
    }
}

// `BigInteger.valueOf(a).pow(e).longValue()`: the low bits are exactly Rust's wrapping result.
fn big_pow(cx: &mut Cx, a: Expr, e: Expr, long: bool) -> Expr {
    cx.import("java.math.BigInteger");
    let big = Expr::call(Expr::raw("BigInteger"), "valueOf", vec![a]);
    Expr::call(
        Expr::call(big, "pow", vec![e]),
        if long { "longValue" } else { "intValue" },
        vec![],
    )
}

// 64-bit saturation from the wrapped result's overflow bits (`isize` is a Java int: 32-bit rules).
fn saturating_wide(a: Expr, b: Expr, i: Int, op: &str) -> Expr {
    let (sa, sb) = (render(&a), render(&b));
    if i == Int::Isize {
        let sym = match op {
            "add" => "+",
            "sub" => "-",
            _ => "*",
        };
        return Expr::raw(format!(
            "(int) Math.max(Integer.MIN_VALUE, Math.min(Integer.MAX_VALUE, (long) {sa} {sym} {sb}))"
        ));
    }
    let text = match (op, i.is_unsigned()) {
        ("add", false) => format!(
            "(({sa} ^ ({sa} + {sb})) & ({sb} ^ ({sa} + {sb}))) < 0 ? ({sa} < 0 ? Long.MIN_VALUE : Long.MAX_VALUE) : {sa} + {sb}"
        ),
        ("sub", false) => format!(
            "(({sa} ^ {sb}) & ({sa} ^ ({sa} - {sb}))) < 0 ? ({sa} < 0 ? Long.MIN_VALUE : Long.MAX_VALUE) : {sa} - {sb}"
        ),
        ("mul", false) => format!(
            "Math.multiplyHigh({sa}, {sb}) != (({sa} * {sb}) >> 63) ? (({sa} ^ {sb}) < 0 ? Long.MIN_VALUE : Long.MAX_VALUE) : {sa} * {sb}"
        ),
        ("add", true) => format!("Long.compareUnsigned({sa} + {sb}, {sa}) < 0 ? -1L : {sa} + {sb}"),
        ("sub", true) => format!("Long.compareUnsigned({sa}, {sb}) < 0 ? 0L : {sa} - {sb}"),
        _ => format!("Math.unsignedMultiplyHigh({sa}, {sb}) != 0 ? -1L : {sa} * {sb}"),
    };
    Expr::raw(format!("({text})"))
}
