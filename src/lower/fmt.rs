// Format strings (`format_args!` family) -> Java string concatenation / String.format.

use ra_ap_hir::ScopeDef;
use ra_ap_syntax::{AstNode, AstToken, SyntaxNode, ast, ast::HasName};

use super::std_map::{signum, unsigned_display, unsigned_string};
use super::{Cx, expr};
use crate::jir::{Arm, ArmBody, Expr, LambdaBody, Stmt, StmtKind};
use crate::ty::Ty;

// Finds the builtin `format_args!` node inside a std macro's (nested) expansion.
pub fn format_args_of(cx: &mut Cx, mc: &ast::MacroCall) -> Option<ast::FormatArgsExpr> {
    fn search(cx: &Cx, node: &SyntaxNode, depth: u32) -> Option<ast::FormatArgsExpr> {
        if depth > 4 {
            return None;
        }
        for n in node.descendants() {
            if let Some(fa) = ast::FormatArgsExpr::cast(n.clone()) {
                return Some(fa);
            }
            if let Some(inner) = ast::MacroCall::cast(n)
                && let Some(exp) = cx.sema.expand_macro_call(&inner)
                && let Some(fa) = search(cx, &exp.value, depth + 1)
            {
                return Some(fa);
            }
        }
        None
    }
    let exp = cx.sema.expand_macro_call(mc)?;
    search(cx, &exp.value, 0)
}

pub fn is_empty_string(e: &Expr) -> bool {
    matches!(e, Expr::Raw(s) if s == "\"\"")
}

enum Piece {
    Lit(String),
    Arg { pos: ArgRef, spec: String },
}

#[derive(Clone)]
enum ArgRef {
    Next,
    Index(usize),
    Name(String),
}

#[derive(Clone)]
enum Count {
    Fixed(String),
    Argument(ArgRef),
    Star,
}

#[derive(Clone)]
struct Spec {
    fill: char,
    align: Option<char>,
    plus: bool,
    alternate: bool,
    zero: bool,
    width: Option<Count>,
    precision: Option<Count>,
    kind: String,
}

struct Counts {
    width: Option<Expr>,
    precision: Option<Expr>,
}

fn parse_count(chars: &[char], pos: &mut usize, precision: bool) -> Option<Count> {
    if precision && chars.get(*pos) == Some(&'*') {
        *pos += 1;
        return Some(Count::Star);
    }
    let start = *pos;
    if chars.get(*pos).is_some_and(char::is_ascii_digit) {
        while chars.get(*pos).is_some_and(char::is_ascii_digit) {
            *pos += 1;
        }
    } else {
        while chars
            .get(*pos)
            .is_some_and(|c| c.is_alphanumeric() || *c == '_')
        {
            *pos += 1;
        }
    }
    if start == *pos {
        return None;
    }
    let text: String = chars[start..*pos].iter().collect();
    if chars.get(*pos) == Some(&'$') {
        *pos += 1;
        Some(Count::Argument(match text.parse() {
            Ok(i) => ArgRef::Index(i),
            Err(_) => ArgRef::Name(text),
        }))
    } else if text.chars().all(|c| c.is_ascii_digit()) {
        Some(Count::Fixed(text))
    } else {
        *pos = start;
        None
    }
}

fn parse_spec(spec: &str) -> Option<Spec> {
    let chars: Vec<char> = spec.chars().collect();
    let mut i = 0;
    let mut fill = ' ';
    let mut align = None;
    if chars.get(1).is_some_and(|c| matches!(c, '<' | '>' | '^')) {
        fill = chars[0];
        align = Some(chars[1]);
        i = 2;
    } else if chars.first().is_some_and(|c| matches!(c, '<' | '>' | '^')) {
        align = chars.first().copied();
        i = 1;
    }
    let plus = chars.get(i) == Some(&'+');
    if plus {
        i += 1;
    }
    let alternate = chars.get(i) == Some(&'#');
    if alternate {
        i += 1;
    }
    let zero = chars.get(i) == Some(&'0');
    if zero {
        i += 1;
    }
    let width = parse_count(&chars, &mut i, false);
    let precision = if chars.get(i) == Some(&'.') {
        i += 1;
        Some(parse_count(&chars, &mut i, true)?)
    } else {
        None
    };
    let kind: String = chars[i..].iter().collect();
    if !matches!(kind.as_str(), "" | "?" | "x" | "X" | "o" | "b" | "e" | "E") {
        return None;
    }
    Some(Spec {
        fill,
        align,
        plus,
        alternate,
        zero,
        width,
        precision,
        kind,
    })
}

fn parse_template(t: &str) -> Option<Vec<Piece>> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' if chars.peek() == Some(&'{') => {
                chars.next();
                lit.push('{');
            }
            '}' if chars.peek() == Some(&'}') => {
                chars.next();
                lit.push('}');
            }
            '{' => {
                let mut inner = String::new();
                for c in chars.by_ref() {
                    if c == '}' {
                        break;
                    }
                    inner.push(c);
                }
                if !lit.is_empty() {
                    pieces.push(Piece::Lit(std::mem::take(&mut lit)));
                }
                let (arg, spec) = match inner.split_once(':') {
                    Some((a, s)) => (a.trim().to_string(), s.to_string()),
                    None => (inner.trim().to_string(), String::new()),
                };
                let pos = if arg.is_empty() {
                    ArgRef::Next
                } else if let Ok(i) = arg.parse() {
                    ArgRef::Index(i)
                } else {
                    ArgRef::Name(arg)
                };
                pieces.push(Piece::Arg { pos, spec });
            }
            c => lit.push(c),
        }
    }
    if !lit.is_empty() {
        pieces.push(Piece::Lit(lit));
    }
    Some(pieces)
}

// Lowers the format string + args to a Java String expression.
pub fn lower_format(cx: &mut Cx, fa: &ast::FormatArgsExpr, out: &mut Vec<Stmt>) -> Expr {
    lower_format_impl(cx, fa, out, false)
}

// For println!/eprintln!/writeln!: the expanded template carries a trailing `\n` that Java's
// println adds itself.
pub fn lower_format_ln(cx: &mut Cx, fa: &ast::FormatArgsExpr, out: &mut Vec<Stmt>) -> Expr {
    lower_format_impl(cx, fa, out, true)
}

fn lower_format_impl(
    cx: &mut Cx,
    fa: &ast::FormatArgsExpr,
    out: &mut Vec<Stmt>,
    strip_nl: bool,
) -> Expr {
    let template = match fa.template() {
        Some(ast::Expr::Literal(l)) => match l.kind() {
            ast::LiteralKind::String(s) => s.value().map(|v| v.to_string()).ok(),
            _ => None,
        },
        _ => None,
    };
    let Some(template) = template else {
        return cx.unsupported(fa.syntax(), "non-literal format string");
    };
    let Some(mut pieces) = parse_template(&template) else {
        return cx.unsupported(fa.syntax(), "format string");
    };
    if strip_nl && let Some(Piece::Lit(s)) = pieces.last_mut() {
        if s.ends_with('\n') {
            s.pop();
        }
        if s.is_empty() {
            pieces.pop();
        }
    }
    let mut specs = Vec::new();
    for p in &pieces {
        if let Piece::Arg { spec, .. } = p {
            let Some(parsed) = parse_spec(spec) else {
                return cx.unsupported(fa.syntax(), &format!("format spec `{{:{spec}}}`"));
            };
            specs.push(parsed);
        }
    }
    // Arguments in source order; each keeps the statements its own lowering hoisted.
    let mut args: Vec<(Option<String>, Expr, Ty, Vec<Stmt>)> = Vec::new();
    let mut hints: Vec<Option<String>> = Vec::new();
    for a in fa.args() {
        let Some(e) = a.expr() else { continue };
        let ty = if matches!(&e, ast::Expr::MacroExpr(m) if m.macro_call().is_some_and(|mc| matches!(super::macros::macro_name(&mc).as_str(), "format_args" | "format_args_nl")))
        {
            Ty::Str
        } else {
            cx.ty(&e)
        };
        if super::std_map::ptr::cell_value(cx, &e) {
            // Rust prints `Cell { value: 5 }`; a pointer to the backing array would be wrong.
            let _ = cx.unsupported(e.syntax(), "Debug/Display of a `Cell`/`RefCell` value");
        }
        let mut pre = Vec::new();
        let v = expr::lower_expr(cx, &e, &mut pre);
        let named = a.name().map(|n| n.text().to_string());
        hints.push(named.clone().or_else(|| super::name_hint(&e)));
        args.push((named, v, ty, pre));
    }
    let temps = args_needing_temps(&pieces, &specs, &args);
    let mut positional: Vec<(Expr, Ty)> = Vec::new();
    let mut named: Vec<(String, Expr, Ty)> = Vec::new();
    for (((name, v, ty, pre), temp), hint) in args.into_iter().zip(temps).zip(hints) {
        out.extend(pre);
        let v = if temp {
            let tmp = cx.fresh(hint.as_deref().unwrap_or("fmt_arg"));
            out.push(Stmt::new(
                cx.line(fa.syntax()),
                StmtKind::Local {
                    ty: if matches!(ty.strip_refs(), Ty::Unit) {
                        "Void".into()
                    } else {
                        super::stmt::decl_type(&ty)
                    },
                    name: tmp.clone(),
                    init: Some(v),
                },
            ));
            Expr::name(tmp)
        } else {
            v
        };
        match name {
            Some(n) => named.push((n, v, ty)),
            None => positional.push((v, ty)),
        }
    }
    if let Some(e) = lower_printf(cx, fa.syntax(), &pieces, &specs, &positional, &named, out) {
        return e;
    }
    let mut next = 0;
    let mut parts: Vec<(Expr, bool)> = Vec::new();
    let mut specs = specs.into_iter();
    for p in pieces {
        match p {
            Piece::Lit(s) => parts.push((Expr::str_lit(&s), true)),
            Piece::Arg { pos, .. } => {
                let parsed = specs.next().expect("spec parsed above");
                let precision = parsed
                    .precision
                    .as_ref()
                    .map(|c| count_expr(cx, fa.syntax(), c, &positional, &named, &mut next));
                let width = parsed
                    .width
                    .as_ref()
                    .map(|c| count_expr(cx, fa.syntax(), c, &positional, &named, &mut next));
                let (v, ty) = resolve_arg(cx, fa.syntax(), &pos, &positional, &named, &mut next);
                let precision = precision.map(|p| hoist_count(cx, fa.syntax(), p, out));
                let width = width.map(|w| hoist_count(cx, fa.syntax(), w, out));
                let is_string = !plain_display(&parsed, &ty, &v);
                let counts = Counts { width, precision };
                let text = if is_string || parsed.kind != "?" {
                    format_one(cx, fa.syntax(), v, &ty, &parsed, &counts, out)
                } else {
                    v
                };
                parts.push((text, is_string));
            }
        }
    }
    concat_typed(parts)
}

fn has_flags(spec: &Spec) -> bool {
    spec.width.is_some()
        || spec.precision.is_some()
        || spec.plus
        || spec.zero
        || spec.alternate
        || !matches!(spec.kind.as_str(), "" | "?")
}

// Java conversion for one flagged piece when `String.format` reproduces Rust exactly, as format
// segments (literal text or a dynamic count) and whether the argument is the value itself
// (`%d`/`%x`/`%f`) rather than its Rust text (`%s`).
fn printf_conv(
    spec: &Spec,
    ty: &Ty,
    width: Option<Expr>,
    precision: Option<Expr>,
) -> Option<(Vec<Expr>, bool)> {
    if spec.fill != ' ' || spec.align == Some('^') {
        return None;
    }
    let t = ty.strip_refs();
    let has_prec = precision.is_some();
    // Debug of str/char ignores width, like Rust's.
    if spec.kind == "?" && matches!(t, Ty::Str | Ty::Char) && !has_prec && !spec.plus && !spec.zero
    {
        return Some((vec![Expr::str_lit("%s")], false));
    }
    let numeric = matches!(t, Ty::Int(_) | Ty::F32 | Ty::F64);
    let text_ok = !spec.plus && !spec.zero;
    let (conv, value) = match (spec.kind.as_str(), t) {
        ("" | "?", Ty::Int(_)) if !has_prec => ("d", true),
        ("x", Ty::Int(_)) if !spec.plus => ("x", true),
        ("X" | "o", Ty::Int(_)) if !spec.plus && !spec.alternate => {
            (if spec.kind == "X" { "X" } else { "o" }, true)
        }
        ("X" | "o" | "b", Ty::Int(_)) if text_ok => ("s", false),
        ("" | "?", Ty::F32 | Ty::F64) if has_prec => ("f", true),
        ("" | "?", Ty::F32 | Ty::F64) if text_ok => ("s", false),
        // Rust pads non-numerics with spaces even under `0`.
        ("", Ty::Str) if !spec.plus => ("s", false),
        ("", Ty::Char) | ("" | "?", Ty::Bool) if !has_prec && !spec.plus => ("s", false),
        _ => return None,
    };
    let mut flags = String::from("%");
    let zero = spec.zero && value;
    if width.is_some() && !zero && (spec.align == Some('<') || (spec.align.is_none() && !numeric)) {
        flags.push('-');
    }
    if spec.plus {
        flags.push('+');
    }
    // Java rejects `#` on `%d`/`%s`; Rust ignores it outside radix formats anyway.
    if spec.alternate && matches!(spec.kind.as_str(), "x" | "X" | "o" | "b") && value {
        flags.push('#');
    }
    if zero {
        flags.push('0');
    }
    let mut segs = vec![Expr::str_lit(&flags)];
    segs.extend(width);
    if let Some(p) = precision {
        segs.push(Expr::str_lit("."));
        segs.push(p);
    }
    segs.push(Expr::str_lit(conv));
    Some((segs, value))
}

// Joins `printf_conv` segments into one format-string expression (`"%0" + w + "x"`).
fn printf_format(segs: Vec<Expr>) -> Option<Expr> {
    let mut parts = Vec::new();
    let mut text = String::new();
    for seg in segs {
        match seg {
            Expr::Raw(r) if r.starts_with('"') => text.push_str(&r[1..r.len() - 1]),
            Expr::Raw(r) if r.chars().all(|c| c.is_ascii_digit()) => text.push_str(&r),
            e => {
                parts.push(Expr::str_lit(&text));
                text.clear();
                parts.push(e);
            }
        }
    }
    parts.push(Expr::str_lit(&text));
    parts
        .into_iter()
        .filter(|e| !is_empty_string(e))
        .reduce(|a, b| Expr::bin(a, "+", b))
}

// One `String.format` for a single flagged piece. `lower_printf` already does this for a whole
// template; this covers the pieces left over when another spec forces the concatenation path.
fn printf_piece(cx: &mut Cx, v: Expr, ty: &Ty, spec: &Spec, counts: &Counts) -> Option<Expr> {
    if counts.width.is_none()
        && !spec.plus
        && !(counts.precision.is_some() && spec.kind.is_empty() && ty.is_str())
    {
        return None;
    }
    let t = ty.strip_refs();
    // Debug of str/char ignores width, so those never reach the padding code anyway.
    if spec.kind == "?" && matches!(t, Ty::Str | Ty::Char) {
        return None;
    }
    // `%d` prints unsigned values signed; pad their unsigned text instead.
    if matches!(spec.kind.as_str(), "" | "?") && matches!(t, Ty::Int(i) if unsigned_display(*i)) {
        let Ty::Int(i) = t else { unreachable!() };
        if spec.plus
            || spec.zero
            || counts.precision.is_some()
            || spec.fill != ' '
            || spec.align == Some('^')
        {
            return None;
        }
        let mut segs = vec![Expr::str_lit(if spec.align == Some('<') {
            "%-"
        } else {
            "%"
        })];
        segs.push(counts.width.clone()?);
        segs.push(Expr::str_lit("s"));
        return Some(Expr::call(
            Expr::raw("String"),
            "format",
            vec![printf_format(segs)?, unsigned_string(*i, v)?],
        ));
    }
    let (segs, value) = printf_conv(spec, ty, counts.width.clone(), counts.precision.clone())?;
    let arg = if value {
        let mut arg = v;
        // Narrow signed values are sign-extended; Java needs the unsigned window.
        if let Ty::Int(i) = t
            && matches!(spec.kind.as_str(), "x" | "X" | "o")
            && matches!(
                i,
                crate::ty::Int::I8 | crate::ty::Int::I16 | crate::ty::Int::U16
            )
        {
            let mask = if *i == crate::ty::Int::I8 {
                "255"
            } else {
                "65535"
            };
            arg = Expr::bin(arg, "&", Expr::raw(mask));
        }
        arg
    } else {
        match t {
            // Debug forces `.0`, Display drops it; `%s` pads whichever text this is.
            Ty::F32 | Ty::F64 if spec.kind == "?" => debug_float(cx, v, *t == Ty::F32),
            Ty::F32 | Ty::F64 => display_float(cx, v, *t == Ty::F32),
            // Radix text (alternate prefixes) is built by the concatenation path.
            Ty::Int(_) => return None,
            _ => v,
        }
    };
    let mut call_args = Vec::new();
    if value && matches!(t, Ty::F32 | Ty::F64) {
        call_args.push(Expr::raw("Locale.ROOT"));
    }
    call_args.push(printf_format(segs)?);
    call_args.push(arg);
    Some(Expr::call(Expr::raw("String"), "format", call_args))
}

// Templates with width/precision/sign/radix flags become one `String.format` when every piece
// maps onto a Java conversion; returns None (nothing emitted) otherwise.
fn lower_printf(
    cx: &mut Cx,
    node: &SyntaxNode,
    pieces: &[Piece],
    specs: &[Spec],
    positional: &[(Expr, Ty)],
    named: &[(String, Expr, Ty)],
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    if !specs.iter().any(has_flags) {
        return None;
    }
    let mut next = 0;
    let mut resolved = Vec::new();
    let mut spec_iter = specs.iter();
    for p in pieces {
        let Piece::Arg { pos, .. } = p else { continue };
        let spec = spec_iter.next()?;
        let precision = spec
            .precision
            .as_ref()
            .map(|c| count_expr(cx, node, c, positional, named, &mut next));
        let width = spec
            .width
            .as_ref()
            .map(|c| count_expr(cx, node, c, positional, named, &mut next));
        let (v, ty) = resolve_arg(cx, node, pos, positional, named, &mut next);
        if matches!(ty.strip_refs(), Ty::Other(_)) || (spec.kind == "?" && !debug_supported(&ty)) {
            return None;
        }
        // Unsigned values need `toUnsignedString`; the concatenation path does that.
        if matches!(spec.kind.as_str(), "" | "?")
            && matches!(ty.strip_refs(), Ty::Int(i) if unsigned_display(*i))
        {
            return None;
        }
        let conv = if has_flags(spec) {
            printf_conv(spec, &ty, width, precision)?
        } else {
            let int = matches!(ty.strip_refs(), Ty::Int(_));
            (vec![Expr::str_lit(if int { "%d" } else { "%s" })], int)
        };
        resolved.push((spec, v, ty, conv));
    }
    // Format text: adjacent literals merge; dynamic counts splice in with `+`.
    let mut format: Vec<Expr> = Vec::new();
    let mut text = String::new();
    let mut args = Vec::new();
    let mut float = false;
    let mut resolved = resolved.into_iter();
    for p in pieces {
        match p {
            Piece::Lit(s) => {
                let quoted = crate::emit::render_expr(&Expr::str_lit(&s.replace('%', "%%")));
                text.push_str(&quoted[1..quoted.len() - 1]);
            }
            Piece::Arg { .. } => {
                let (spec, v, ty, (segs, value)) = resolved.next()?;
                for seg in segs {
                    match seg {
                        Expr::Raw(r) if r.starts_with('"') => text.push_str(&r[1..r.len() - 1]),
                        Expr::Raw(r) if r.chars().all(|c| c.is_ascii_digit()) => text.push_str(&r),
                        e => {
                            format.push(Expr::raw(format!("\"{text}\"")));
                            text.clear();
                            format.push(e);
                        }
                    }
                }
                float |= value && text.ends_with('f');
                let arg = if plain_display(spec, &ty, &v) {
                    v
                } else if !value {
                    let plain = Spec {
                        width: None,
                        precision: None,
                        plus: false,
                        zero: false,
                        ..spec.clone()
                    };
                    format_one(
                        cx,
                        node,
                        v,
                        &ty,
                        &plain,
                        &Counts {
                            width: None,
                            precision: None,
                        },
                        out,
                    )
                } else if let Ty::Int(i) = ty.strip_refs()
                    && matches!(spec.kind.as_str(), "x" | "X" | "o")
                    && matches!(
                        i,
                        crate::ty::Int::I8 | crate::ty::Int::I16 | crate::ty::Int::U16
                    )
                {
                    let mask = if *i == crate::ty::Int::I8 {
                        "255"
                    } else {
                        "65535"
                    };
                    Expr::bin(v, "&", Expr::raw(mask))
                } else {
                    v
                };
                args.push(arg);
            }
        }
    }
    format.push(Expr::raw(format!("\"{text}\"")));
    let format = format
        .into_iter()
        .filter(|e| !is_empty_string(e))
        .reduce(|a, b| Expr::bin(a, "+", b))?;
    let mut call_args = Vec::new();
    if float {
        call_args.push(Expr::raw("Locale.ROOT"));
    }
    call_args.push(format);
    call_args.extend(args);
    Some(Expr::call(Expr::raw("String"), "format", call_args))
}

// Pieces whose Java expression is the value itself (not a String) inside the concatenation.
fn plain_display(spec: &Spec, ty: &Ty, v: &Expr) -> bool {
    if spec.kind == "?" {
        return !has_flags(spec) && debug_is_to_string(ty) && !is_target_typed(v);
    }
    spec.kind.is_empty()
        && spec.width.is_none()
        && spec.precision.is_none()
        && !spec.plus
        && !ty.is_str()
        && !ty.is_float()
        && std_error_name(ty).is_none()
        && !matches!(ty.strip_refs(), Ty::Int(i) if unsigned_display(*i))
}

// `stream.collect(...)` infers its type from the call site, which breaks overloads like println.
// `String.valueOf(x)` / `sb.toString()` where the consumer (print, append) takes any value.
pub fn as_any_value(e: Expr) -> Expr {
    match e {
        Expr::Call {
            recv: Some(r),
            name,
            mut args,
        } if matches!(r.as_ref(), Expr::Raw(c) if c == "String")
            && name == "valueOf"
            && args.len() == 1
            && !is_target_typed(&args[0]) =>
        {
            args.remove(0)
        }
        Expr::Call {
            recv: Some(r),
            name,
            args,
        } if name == "toString" && args.is_empty() && matches!(r.as_ref(), Expr::Name(_)) => *r,
        e => e,
    }
}

pub fn is_target_typed(e: &Expr) -> bool {
    matches!(e, Expr::Call { name, .. } if name == "collect")
}

// Simple mode: types whose `toString` stands in for Debug. User types with a Display keep their
// `debug()`, and tuples (`Map.Entry` prints `a=1`) and `Ordering` (an int) keep the exact text.
fn debug_via_to_string(cx: &Cx, ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Bool | Ty::Char | Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Str => true,
        Ty::Vec(t) | Ty::Deque(t) | Ty::Set { t, .. } | Ty::Heap(t) | Ty::Option(t) => {
            debug_via_to_string(cx, t)
        }
        Ty::Map { k, v, .. } => debug_via_to_string(cx, k) && debug_via_to_string(cx, v),
        t @ Ty::Adt { .. } => super::items::debug_via_to_string(cx, t),
        _ => false,
    }
}

// Java's `toString` already prints Rust's Debug text for these (`[1, 2, 3]`, `true`).
pub(crate) fn debug_is_to_string(ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Int(i) => !unsigned_display(*i),
        Ty::Bool => true,
        Ty::Vec(t) | Ty::Deque(t) => debug_is_to_string(t),
        _ => false,
    }
}

// Rust evaluates every argument once, in order, before formatting. An argument can be inlined
// into the concatenation when that is indistinguishable: pure places always, other expressions
// when used once, not duplicated by the formatter, and not overtaken by a later hoisted argument.
fn args_needing_temps(
    pieces: &[Piece],
    specs: &[Spec],
    args: &[(Option<String>, Expr, Ty, Vec<Stmt>)],
) -> Vec<bool> {
    let n = args.len();
    let positional = args.iter().filter(|a| a.0.is_none()).count();
    let index = |r: &ArgRef, next: &mut usize| -> Option<usize> {
        match r {
            ArgRef::Next => {
                *next += 1;
                Some(*next - 1)
            }
            ArgRef::Index(i) => Some(*i),
            ArgRef::Name(name) => args
                .iter()
                .skip(positional)
                .position(|a| a.0.as_deref() == Some(name))
                .map(|i| i + positional),
        }
    };
    let mut uses = vec![0usize; n];
    let mut temp = vec![false; n];
    let mut order = Vec::new();
    let mut next = 0;
    let mut specs = specs.iter();
    for p in pieces {
        let Piece::Arg { pos, .. } = p else { continue };
        let spec = specs.next().expect("one spec per argument piece");
        for c in [&spec.precision, &spec.width].into_iter().flatten() {
            let r = match c {
                Count::Fixed(_) => None,
                Count::Argument(a) => index(a, &mut next),
                Count::Star => index(&ArgRef::Next, &mut next),
            };
            if let Some(i) = r.filter(|&i| i < n) {
                temp[i] = true;
            }
        }
        let Some(i) = index(pos, &mut next).filter(|&i| i < n) else {
            continue;
        };
        uses[i] += 1;
        order.push(i);
        let duplicated =
            spec.kind == "?" && matches!(args[i].2.strip_refs(), Ty::F32 | Ty::F64 | Ty::Tuple(_));
        if duplicated {
            temp[i] = true;
        }
    }
    let pure: Vec<bool> = args.iter().map(|a| is_pure(&a.1)).collect();
    for i in 0..n {
        if uses[i] != 1 {
            temp[i] = true;
        }
    }
    let inline_order: Vec<usize> = order
        .into_iter()
        .filter(|&i| !pure[i] && !temp[i])
        .collect();
    if inline_order.windows(2).any(|w| w[0] >= w[1]) {
        temp.iter_mut().zip(&pure).for_each(|(t, p)| *t |= !p);
    }
    // Anything hoisted for a later argument runs before an inlined earlier one.
    if let Some(barrier) = (0..n)
        .rev()
        .find(|&j| (temp[j] && !pure[j]) || !args[j].3.is_empty())
    {
        for i in 0..barrier {
            temp[i] |= !pure[i];
        }
    }
    temp.iter().zip(&pure).map(|(t, p)| *t && !p).collect()
}

pub(crate) fn is_pure(e: &Expr) -> bool {
    match e {
        Expr::Name(_) => true,
        Expr::Field(base, _) => is_pure(base),
        Expr::Unary("-", inner) => is_pure(inner),
        Expr::Raw(s) => {
            s.starts_with('"')
                || s.starts_with('\'')
                || s.chars()
                    .all(|c| c.is_alphanumeric() || "_.[]$".contains(c))
        }
        _ => false,
    }
}

fn hoist_count(cx: &mut Cx, node: &SyntaxNode, value: Expr, out: &mut Vec<Stmt>) -> Expr {
    if matches!(&value, Expr::Raw(s) if s.chars().all(|c| c.is_ascii_digit())) {
        return value;
    }
    let name = cx.fresh("fmt_count");
    out.push(Stmt::new(
        cx.line(node),
        StmtKind::Local {
            ty: "int".into(),
            name: name.clone(),
            init: Some(value),
        },
    ));
    Expr::name(name)
}

fn resolve_arg(
    cx: &mut Cx,
    node: &SyntaxNode,
    arg: &ArgRef,
    positional: &[(Expr, Ty)],
    named: &[(String, Expr, Ty)],
    next: &mut usize,
) -> (Expr, Ty) {
    match arg {
        ArgRef::Next => {
            let i = *next;
            *next += 1;
            positional.get(i).cloned().unwrap_or_else(|| {
                (
                    cx.unsupported(node, "format argument count"),
                    Ty::Other("?".into()),
                )
            })
        }
        ArgRef::Index(i) => positional.get(*i).cloned().unwrap_or_else(|| {
            (
                cx.unsupported(node, "format argument index"),
                Ty::Other("?".into()),
            )
        }),
        ArgRef::Name(n) => named
            .iter()
            .find(|(m, _, _)| m == n)
            .map(|(_, v, t)| (v.clone(), t.clone()))
            .unwrap_or_else(|| captured(cx, node, n)),
    }
}

fn count_expr(
    cx: &mut Cx,
    node: &SyntaxNode,
    count: &Count,
    positional: &[(Expr, Ty)],
    named: &[(String, Expr, Ty)],
    next: &mut usize,
) -> Expr {
    match count {
        Count::Fixed(n) => Expr::raw(n),
        Count::Argument(a) => resolve_arg(cx, node, a, positional, named, next).0,
        Count::Star => resolve_arg(cx, node, &ArgRef::Next, positional, named, next).0,
    }
}

// Inline-captured `{name}`: a local in scope at the macro call.
fn captured(cx: &mut Cx, node: &SyntaxNode, name: &str) -> (Expr, Ty) {
    let mut found = None;
    if let Some(scope) = cx.sema.scope(node) {
        scope.process_all_names(&mut |n, def| {
            if found.is_none()
                && n.as_str() == name
                && let ScopeDef::Local(l) = def
            {
                found = Some((l.ty(cx.db), cx.cells.contains(&l)));
            }
        });
    }
    let java = cx.lookup(name);
    match found {
        Some((t, is_cell)) => {
            if super::std_map::ptr::cell_value_type(cx, &t) {
                cx.unsupported(node, "Debug/Display of a `Cell`/`RefCell` value");
            }
            let e = if is_cell {
                Expr::raw(format!("{java}[0]"))
            } else {
                Expr::name(java)
            };
            (e, cx.convert(&t))
        }
        None => (Expr::name(java), Ty::Other("?".into())),
    }
}

fn format_one(
    cx: &mut Cx,
    node: &SyntaxNode,
    v: Expr,
    ty: &Ty,
    spec: &Spec,
    counts: &Counts,
    out: &mut Vec<Stmt>,
) -> Expr {
    let width = counts.width.clone();
    let precision = counts.precision.clone();
    if matches!(ty.strip_refs(), Ty::Other(_))
        && std_error_name(ty).is_none()
        && !(spec.kind.is_empty() && is_io_error(ty))
    {
        return cx.unsupported(node, &format!("format argument type `{}`", ty.describe()));
    }
    let numeric = matches!(ty.strip_refs(), Ty::Int(_) | Ty::F32 | Ty::F64);
    // Simple mode: `{:?}` is Java's `toString` (`[1, 2]`, `{a=1}`, `Optional[3]`, unquoted text).
    if crate::mode::simple()
        && spec.kind == "?"
        && !spec.alternate
        && width.is_none()
        && precision.is_none()
        && !spec.plus
        && !spec.zero
        && debug_via_to_string(cx, ty)
        && !is_target_typed(&v)
    {
        // A user type isn't a String: `"x" + p` needs a string on one side, `String s = p` fails.
        if matches!(ty.strip_refs(), Ty::Adt { .. }) {
            return Expr::call(Expr::raw("String"), "valueOf", vec![v]);
        }
        return v;
    }
    if spec.alternate && spec.kind == "?" {
        if width.is_some() || precision.is_some() || spec.plus || spec.zero {
            return cx.unsupported(node, "flags on a pretty Debug format");
        }
        let compact = debug(cx, v, ty);
        return debug_pretty(cx, compact);
    }
    if spec.kind == "?" {
        if !debug_supported(ty) {
            return cx.unsupported(node, &format!("Debug for `{}`", ty.describe()));
        }
        match ty.strip_refs() {
            Ty::Unit | Ty::Str | Ty::Char => return debug(cx, v, ty),
            Ty::Vec(_)
            | Ty::Deque(_)
            | Ty::Set { .. }
            | Ty::Map { .. }
            | Ty::Option(_)
            | Ty::Result(_, _)
            | Ty::Tuple(_) => {
                return debug_with_spec(cx, node, v, ty, spec, counts, out);
            }
            Ty::Adt { .. } if width.is_some() || precision.is_some() || spec.plus || spec.zero => {
                return cx.unsupported(node, "format flags for user Debug");
            }
            _ => {}
        }
    }
    // A single Java conversion is shorter than hand-built padding and matches Rust's text.
    if let Some(e) = printf_piece(cx, v.clone(), ty, spec, counts) {
        return e;
    }
    let mut text = if spec.kind == "?" {
        if ty.is_float()
            && let Some(p) = precision
        {
            let fmt = Expr::bin(
                Expr::bin(Expr::str_lit("%."), "+", p),
                "+",
                Expr::str_lit("f"),
            );
            Expr::call(
                Expr::raw("String"),
                "format",
                vec![Expr::raw("Locale.ROOT"), fmt, v],
            )
        } else {
            debug(cx, v, ty)
        }
    } else if matches!(spec.kind.as_str(), "e" | "E") {
        if !matches!(ty.strip_refs(), Ty::Int(_) | Ty::F32 | Ty::F64) {
            return cx.unsupported(node, "`{:e}` of a non-number");
        }
        let sci = sci_float(cx, v, ty.strip_refs() == &Ty::F32, precision);
        if spec.kind == "E" {
            Expr::call(sci, "replace", vec![Expr::raw("'e'"), Expr::raw("'E'")])
        } else {
            sci
        }
    } else if !spec.kind.is_empty() {
        let Ty::Int(i) = ty.strip_refs() else {
            return cx.unsupported(node, "non-integer radix format");
        };
        let conv = match spec.kind.as_str() {
            "x" => "toHexString",
            "X" => "toHexString",
            "o" => "toOctalString",
            "b" => "toBinaryString",
            _ => unreachable!(),
        };
        let mut number = v;
        if matches!(
            i,
            crate::ty::Int::I8 | crate::ty::Int::I16 | crate::ty::Int::U16
        ) {
            let mask = if *i == crate::ty::Int::I8 {
                "255"
            } else {
                "65535"
            };
            number = Expr::bin(number, "&", Expr::raw(mask));
        }
        let class = if i.is_long() { "Long" } else { "Integer" };
        let mut digits = Expr::call(Expr::raw(class), conv, vec![number]);
        if spec.kind == "X" {
            digits = Expr::call(digits, "toUpperCase", vec![Expr::raw("Locale.ROOT")]);
        }
        if spec.alternate {
            let prefix = match spec.kind.as_str() {
                "x" => "0x",
                "X" => "0x",
                "o" => "0o",
                _ => "0b",
            };
            digits = Expr::bin(Expr::str_lit(prefix), "+", digits);
        }
        digits
    } else if let Some(p) = precision {
        if ty.is_float() {
            let fmt = Expr::bin(
                Expr::bin(Expr::str_lit("%."), "+", p),
                "+",
                Expr::str_lit("f"),
            );
            Expr::call(
                Expr::raw("String"),
                "format",
                vec![Expr::raw("Locale.ROOT"), fmt, v],
            )
        } else if ty.is_str() {
            let tmp = hoist_string(cx, node, v, out);
            let len = Expr::call(
                tmp.clone(),
                "codePointCount",
                vec![Expr::raw("0"), Expr::call(tmp.clone(), "length", vec![])],
            );
            let count = Expr::call(Expr::raw("Math"), "min", vec![p, len]);
            let end = Expr::call(
                tmp.clone(),
                "offsetByCodePoints",
                vec![Expr::raw("0"), count],
            );
            Expr::call(tmp, "substring", vec![Expr::raw("0"), end])
        } else {
            return cx.unsupported(node, "precision on this format type");
        }
    } else {
        display(cx, node, v, ty, out)
    };
    if spec.plus || width.is_some() {
        // `Display` of an unsigned integer is already a String; other scalars need converting.
        let already_text =
            ty.is_str() || matches!(ty.strip_refs(), Ty::Int(i) if unsigned_display(*i));
        if spec.kind.is_empty() && counts.precision.is_none() && !already_text && !ty.is_float() {
            text = Expr::call(Expr::raw("String"), "valueOf", vec![text]);
        }
    } else {
        return text;
    }
    if spec.plus && numeric {
        if matches!(spec.kind.as_str(), "x" | "X" | "o" | "b") {
            // Radix digits never carry a sign, so Rust always adds the `+` here.
            text = Expr::bin(Expr::str_lit("+"), "+", text);
        } else {
            let tmp = hoist_string(cx, node, text, out);
            let negative = Expr::call(tmp.clone(), "startsWith", vec![Expr::str_lit("-")]);
            text = Expr::Ternary(
                Box::new(negative),
                Box::new(tmp.clone()),
                Box::new(Expr::bin(Expr::str_lit("+"), "+", tmp)),
            );
        }
    }
    let Some(width) = width else { return text };
    let tmp = hoist_string(cx, node, text, out);
    let len = Expr::call(
        tmp.clone(),
        "codePointCount",
        vec![Expr::raw("0"), Expr::call(tmp.clone(), "length", vec![])],
    );
    let pad = Expr::call(
        Expr::raw("Math"),
        "max",
        vec![Expr::raw("0"), Expr::bin(width, "-", len)],
    );
    if spec.zero && numeric {
        let prefix = if spec.alternate && matches!(spec.kind.as_str(), "x" | "X" | "o" | "b") {
            2
        } else {
            0
        };
        let zeros = Expr::call(Expr::str_lit("0"), "repeat", vec![pad]);
        // Radix digits start right after the prefix, with no sign to skip.
        let sign = if !spec.kind.is_empty() {
            (prefix != 0).then(|| Expr::raw("2"))
        } else {
            let raw = crate::emit::render_expr(&tmp);
            Some(Expr::raw(format!(
                "({raw}.startsWith(\"-\") || {raw}.startsWith(\"+\") ? 1 : 0) + {prefix}"
            )))
        };
        return match sign {
            None => Expr::bin(zeros, "+", tmp),
            Some(sign) => {
                let left = Expr::call(tmp.clone(), "substring", vec![Expr::raw("0"), sign.clone()]);
                let right = Expr::call(tmp, "substring", vec![sign]);
                Expr::bin(Expr::bin(left, "+", zeros), "+", right)
            }
        };
    }
    let align = spec.align.unwrap_or(if numeric { '>' } else { '<' });
    let fill = Expr::str_lit(&spec.fill.to_string());
    match align {
        '<' => Expr::bin(tmp, "+", Expr::call(fill, "repeat", vec![pad])),
        '>' => Expr::bin(Expr::call(fill, "repeat", vec![pad]), "+", tmp),
        _ => {
            // Centering needs both sides; keep the pad in one temp instead of re-deriving it.
            let name = cx.fresh("fmt_pad");
            out.push(Stmt::new(
                cx.line(node),
                StmtKind::Local {
                    ty: "int".into(),
                    name: name.clone(),
                    init: Some(pad),
                },
            ));
            let pad = Expr::name(name);
            let left = Expr::bin(pad.clone(), "/", Expr::raw("2"));
            let right = Expr::bin(pad, "-", left.clone());
            Expr::bin(
                Expr::bin(Expr::call(fill.clone(), "repeat", vec![left]), "+", tmp),
                "+",
                Expr::call(fill, "repeat", vec![right]),
            )
        }
    }
}

fn hoist_string(cx: &mut Cx, node: &SyntaxNode, value: Expr, out: &mut Vec<Stmt>) -> Expr {
    // A plain name or literal can be read twice as it is.
    if is_pure(&value) {
        return value;
    }
    let name = cx.fresh("fmt_text");
    out.push(Stmt::new(
        cx.line(node),
        StmtKind::Local {
            ty: "String".into(),
            name: name.clone(),
            init: Some(value),
        },
    ));
    Expr::name(name)
}

// `{}`: Rust Display as a Java expression.
pub fn display(cx: &mut Cx, _node: &SyntaxNode, v: Expr, ty: &Ty, _out: &mut Vec<Stmt>) -> Expr {
    if let Some(e) = std_error_display(cx, v.clone(), ty) {
        return e;
    }
    match ty.strip_refs() {
        Ty::F32 | Ty::F64 => display_float(cx, v, *ty.strip_refs() == Ty::F32),
        // A generic value may be a float, whose Java text differs (`2.0` for Rust's `2`).
        Ty::Param(_) => {
            display_float(cx, Expr::raw("0.0"), false);
            file_helper(
                cx,
                "static String display_any(Object o) { return o instanceof Double d ? display_float(d) : String.valueOf(o); }".into(),
            );
            Expr::static_call("display_any", vec![v])
        }
        // Rust prints unsigned values as such; Java's conversion is signed.
        Ty::Int(i) => match unsigned_string(*i, v.clone()) {
            Some(e) => e,
            None => v,
        },
        _ => v,
    }
}

fn is_unmapped(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(_))
}

// Rust std errors keep rustc's Display text in a `NumberFormatException`; `Box<dyn Error>`
// is the payload itself (a String for message errors).
fn std_error_name(ty: &Ty) -> Option<&str> {
    let Ty::Other(name) = ty.strip_refs() else {
        return None;
    };
    if name.contains("ParseIntError")
        || name.contains("ParseFloatError")
        || name.contains("dyn Error")
        || name.contains("Box<dyn Error")
    {
        Some(name)
    } else {
        None
    }
}

fn parse_error_text(v: Expr) -> Expr {
    // Throws mode only meets these errors as caught `NumberFormatException`s.
    if crate::mode::throws() && matches!(v, Expr::Name(_)) {
        return Expr::call(v, "getMessage", vec![]);
    }
    Expr::call(
        Expr::Cast("NumberFormatException".into(), Box::new(v)),
        "getMessage",
        vec![],
    )
}

// One switch over the Display text; the messages of the int and float errors are disjoint.
fn parse_debug_switch(msg: Expr) -> Expr {
    let messages = [
        (
            "cannot parse integer from empty string",
            "ParseIntError { kind: Empty }",
        ),
        (
            "number too large to fit in target type",
            "ParseIntError { kind: PosOverflow }",
        ),
        (
            "number too small to fit in target type",
            "ParseIntError { kind: NegOverflow }",
        ),
        (
            "invalid digit found in string",
            "ParseIntError { kind: InvalidDigit }",
        ),
        (
            "cannot parse float from empty string",
            "ParseFloatError { kind: Empty }",
        ),
        ("invalid float literal", "ParseFloatError { kind: Invalid }"),
    ];
    let mut arms: Vec<Arm> = messages
        .iter()
        .map(|(msg, debug)| Arm {
            line: 1,
            label: crate::emit::render_expr(&Expr::str_lit(msg)),
            body: ArmBody::Expr(Expr::str_lit(debug)),
        })
        .collect();
    arms.push(Arm {
        line: 1,
        label: "default".into(),
        body: ArmBody::Expr(Expr::call(
            Expr::raw("String"),
            "valueOf",
            vec![msg.clone()],
        )),
    });
    Expr::Switch {
        scrut: Box::new(msg),
        arms,
    }
}

pub fn std_error_debug(cx: &mut Cx, v: Expr, ty: &Ty) -> Option<Expr> {
    let name = std_error_name(ty)?;
    if name.contains("ParseIntError") || name.contains("ParseFloatError") {
        return Some(parse_debug_switch(parse_error_text(v)));
    }
    let nf = cx.fresh("error");
    let s = cx.fresh("error");
    let quoted = concat(vec![
        Expr::str_lit("\""),
        Expr::name(s.clone()),
        Expr::str_lit("\""),
    ]);
    let object = Expr::Cast("Object".into(), Box::new(v.clone()));
    Some(Expr::Ternary(
        Box::new(Expr::InstanceOf(
            Box::new(object.clone()),
            format!("NumberFormatException {nf}"),
        )),
        Box::new(parse_debug_switch(parse_error_text(Expr::name(nf)))),
        Box::new(Expr::Ternary(
            Box::new(Expr::InstanceOf(Box::new(object), format!("String {s}"))),
            Box::new(quoted),
            Box::new(Expr::call(Expr::raw("String"), "valueOf", vec![v])),
        )),
    ))
}

// `std::io::Error` (a Java IOException).
fn is_io_error(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(n) if n == "Error" || n.ends_with("io::Error"))
}

// Display of an io::Error: Rust's text for the common file errors, the Java message otherwise.
fn io_error_text(cx: &mut Cx, v: Expr) -> Expr {
    cx.import("java.nio.file.*");
    let object = Expr::Cast("Object".into(), Box::new(v.clone()));
    let known = [
        (
            "NoSuchFileException",
            "No such file or directory (os error 2)",
        ),
        ("AccessDeniedException", "Permission denied (os error 13)"),
        ("FileAlreadyExistsException", "File exists (os error 17)"),
    ];
    let fallback = Expr::call(
        Expr::Cast("Exception".into(), Box::new(v)),
        "getMessage",
        vec![],
    );
    known.iter().rev().fold(fallback, |rest, (class, text)| {
        Expr::Ternary(
            Box::new(Expr::InstanceOf(Box::new(object.clone()), (*class).into())),
            Box::new(Expr::str_lit(text)),
            Box::new(rest),
        )
    })
}

pub fn std_error_display(cx: &mut Cx, v: Expr, ty: &Ty) -> Option<Expr> {
    if is_io_error(ty) {
        return Some(io_error_text(cx, v));
    }
    let name = std_error_name(ty)?;
    if name.contains("ParseIntError") || name.contains("ParseFloatError") {
        return Some(parse_error_text(v));
    }
    let bound = cx.fresh("err_value");
    Some(Expr::Ternary(
        Box::new(Expr::InstanceOf(
            Box::new(Expr::Cast("Object".into(), Box::new(v.clone()))),
            format!("NumberFormatException {bound}"),
        )),
        Box::new(parse_error_text(Expr::name(bound))),
        Box::new(Expr::call(Expr::raw("String"), "valueOf", vec![v])),
    ))
}

// Adds a static helper method to this file's trailer line once.
fn file_helper(cx: &mut Cx, decl: String) {
    if !cx.file_trailer.contains(&decl) {
        cx.file_trailer.insert(0, decl);
    }
}

// Rust's float Display: shortest digits without exponent, `NaN`/`inf`/`-inf`, and `-0`.
pub(crate) fn display_float(cx: &mut Cx, v: Expr, f32: bool) -> Expr {
    cx.import("java.math.BigDecimal");
    // `BigDecimal.valueOf(float)` would widen first and print the double's extra digits.
    let (t, cls, big) = if f32 {
        ("float", "Float", "new BigDecimal(String.valueOf(x))")
    } else {
        ("double", "Double", "BigDecimal.valueOf(x)")
    };
    file_helper(
        cx,
        format!(
            "static String display_float({t} x) {{ if ({cls}.isNaN(x)) return \"NaN\"; if ({cls}.isInfinite(x)) return x > 0 ? \"inf\" : \"-inf\"; \
             if (x == 0) return 1 / x < 0 ? \"-0\" : \"0\"; return {big}.stripTrailingZeros().toPlainString(); }}"
        ),
    );
    Expr::static_call("display_float", vec![v])
}

pub fn debug_supported(ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Unit
        | Ty::Bool
        | Ty::Char
        | Ty::Int(_)
        | Ty::F32
        | Ty::F64
        | Ty::Str
        | Ty::Ordering
        | Ty::Param(_)
        | Ty::Adt { .. } => true,
        Ty::Other(_) => std_error_name(ty).is_some(),
        Ty::Vec(t) | Ty::Set { t, .. } | Ty::Deque(t) | Ty::Option(t) => debug_supported(t),
        Ty::Map { k, v, .. } => debug_supported(k) && debug_supported(v),
        // A std error payload (e.g. `ParseIntError`) has no Rust-shaped Debug; print its text.
        Ty::Result(k, v) => debug_supported(k) && (debug_supported(v) || is_unmapped(v)),
        Ty::Tuple(ts) => ts.iter().all(debug_supported),
        _ => false,
    }
}

fn debug_lambda(
    cx: &mut Cx,
    node: &SyntaxNode,
    name: String,
    body: Expr,
    mut pre: Vec<Stmt>,
) -> Expr {
    if pre.is_empty() {
        return lambda(cx, name, body);
    }
    cx.release_param(&name);
    let line = cx.line(node);
    pre.push(Stmt::new(line, StmtKind::Return(Some(body))));
    Expr::Lambda {
        params: vec![name],
        body: LambdaBody::Block(crate::jir::Block {
            stmts: pre,
            close_line: line,
        }),
    }
}

fn debug_with_spec(
    cx: &mut Cx,
    node: &SyntaxNode,
    value: Expr,
    ty: &Ty,
    spec: &Spec,
    counts: &Counts,
    out: &mut Vec<Stmt>,
) -> Expr {
    // No width/precision/flags: the plain `debug` form is already the shortest.
    if !has_flags(spec) && spec.align.is_none() {
        return debug(cx, value, ty);
    }
    match ty.strip_refs() {
        Ty::Vec(inner) | Ty::Deque(inner) | Ty::Set { t: inner, .. } => {
            let name = cx.fresh_param("fmt_item");
            let mut pre = Vec::new();
            let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
            let mapper = debug_lambda(cx, node, name, item, pre);
            let stream = Expr::call(value, "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![mapper]);
            let (open, close) = if matches!(ty.strip_refs(), Ty::Set { .. }) {
                ("{", "}")
            } else {
                ("[", "]")
            };
            let joining = Expr::call(
                Expr::raw("Collectors"),
                "joining",
                vec![
                    Expr::str_lit(", "),
                    Expr::str_lit(open),
                    Expr::str_lit(close),
                ],
            );
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Map { k, v, .. } => {
            let name = cx.fresh_param("fmt_entry");
            let mut pre = Vec::new();
            let key = format_one(
                cx,
                node,
                Expr::call(Expr::name(&name), "getKey", vec![]),
                k,
                spec,
                counts,
                &mut pre,
            );
            let val = format_one(
                cx,
                node,
                Expr::call(Expr::name(&name), "getValue", vec![]),
                v,
                spec,
                counts,
                &mut pre,
            );
            let item = concat(vec![key, Expr::str_lit(": "), val]);
            let mapper = debug_lambda(cx, node, name, item, pre);
            let stream = Expr::call(Expr::call(value, "entrySet", vec![]), "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![mapper]);
            let joining = Expr::call(
                Expr::raw("Collectors"),
                "joining",
                vec![Expr::str_lit(", "), Expr::str_lit("{"), Expr::str_lit("}")],
            );
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Option(inner) => {
            let name = cx.fresh_param("fmt_some");
            let mut pre = Vec::new();
            let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
            let some = concat(vec![Expr::str_lit("Some("), item, Expr::str_lit(")")]);
            let mapper = debug_lambda(cx, node, name, some, pre);
            Expr::call(
                Expr::call(value, "map", vec![mapper]),
                "orElse",
                vec![Expr::str_lit("None")],
            )
        }
        Ty::Tuple(ts) => {
            let mut parts = vec![Expr::str_lit("(")];
            for (i, t) in ts.iter().enumerate() {
                if i != 0 {
                    parts.push(Expr::str_lit(", "));
                }
                let field = expr::tuple_field(value.clone(), i, ts);
                parts.push(format_one(cx, node, field, t, spec, counts, out));
            }
            if ts.len() == 1 {
                parts.push(Expr::str_lit(","));
            }
            parts.push(Expr::str_lit(")"));
            concat(parts)
        }
        Ty::Result(ok, err) => {
            let mut arms = Vec::new();
            for (variant, inner) in [("Ok", ok.as_ref()), ("Err", err.as_ref())] {
                let name = cx.fresh("fmt_value");
                let mut pre = Vec::new();
                let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
                let text = concat(vec![
                    Expr::str_lit(&format!("{variant}(")),
                    item,
                    Expr::str_lit(")"),
                ]);
                let body = if pre.is_empty() {
                    ArmBody::Expr(text)
                } else {
                    let line = cx.line(node);
                    pre.push(Stmt::new(
                        line,
                        StmtKind::Raw(format!("yield {};", crate::emit::render_expr(&text))),
                    ));
                    ArmBody::Block(crate::jir::Block {
                        stmts: pre,
                        close_line: line,
                    })
                };
                arms.push(Arm {
                    line: cx.line(node),
                    label: format!("{variant}<{}, {}>(var {name})", ok.boxed(), err.boxed()),
                    body,
                });
            }
            Expr::Switch {
                scrut: Box::new(value),
                arms,
            }
        }
        _ => unreachable!(),
    }
}

fn lambda(cx: &mut Cx, name: String, body: Expr) -> Expr {
    cx.release_param(&name);
    Expr::Lambda {
        params: vec![name],
        body: LambdaBody::Expr(Box::new(body)),
    }
}

// The `map` step for a sequence element: a method reference reads best when the element's Debug
// text is just Java's `toString` or the user type's generated `debug()`.
fn debug_mapper(cx: &mut Cx, ty: &Ty) -> Expr {
    if debug_is_to_string(ty) {
        return Expr::raw("String::valueOf");
    }
    if let Ty::Adt { java, args, .. } = ty.strip_refs()
        && args.iter().all(Ty::is_nameable)
    {
        return Expr::raw(format!("{java}::debug"));
    }
    let name = cx.fresh_param("fmt_item");
    let item = debug(cx, Expr::name(&name), ty);
    lambda(cx, name, item)
}

// Rust's `{:?}` escaping for str/char, emitted once per file as `debug_str` on the trailer line.
fn debug_quote(cx: &mut Cx, value: Expr, quote: char) -> Expr {
    let helper = "static String debug_str(String s, char q) { StringBuilder b = new StringBuilder().append(q); \
                  s.codePoints().forEach(c -> b.append(c == q ? \"\\\\\" + q : c == '\\\\' ? \"\\\\\\\\\" : c == '\\n' ? \"\\\\n\" : c == '\\r' ? \"\\\\r\" : c == '\\t' ? \"\\\\t\" : c == 0 ? \"\\\\0\" \
                  : Character.isISOControl(c) ? \"\\\\u{\" + Integer.toHexString(c) + \"}\" : Character.toString(c))); return b.append(q).toString(); }";
    file_helper(cx, helper.to_string());
    let value = if quote == '\'' {
        Expr::call(Expr::raw("String"), "valueOf", vec![value])
    } else {
        value
    };
    let q = if quote == '\'' { "'\\''" } else { "'\"'" };
    Expr::static_call("debug_str", vec![value, Expr::raw(q)])
}

fn debug_sequence(cx: &mut Cx, v: Expr, inner: &Ty, open: &str, close: &str) -> Expr {
    let stream = Expr::call(v, "stream", vec![]);
    let mapped = Expr::call(stream, "map", vec![debug_mapper(cx, inner)]);
    let joining = Expr::call(
        Expr::raw("Collectors"),
        "joining",
        vec![
            Expr::str_lit(", "),
            Expr::str_lit(open),
            Expr::str_lit(close),
        ],
    );
    Expr::call(mapped, "collect", vec![joining])
}

// `{:#?}`: the compact Debug text re-indented the way Rust's alternate form lays it out (one
// item per line, trailing commas), skipping over quoted strings and chars.
fn debug_pretty(cx: &mut Cx, compact: Expr) -> Expr {
    let helper = concat!(
        r#"static String debug_pretty(String s) { StringBuilder b = new StringBuilder(); int depth = 0; char q = 0; "#,
        r#"for (int i = 0; i < s.length(); i++) { char c = s.charAt(i); "#,
        r#"if (q != 0) { b.append(c); if (c == '\\') b.append(s.charAt(++i)); else if (c == q) q = 0; continue; } "#,
        r#"if (c == '"' || c == '\'') { q = c; b.append(c); continue; } "#,
        r#"if (c == '{' || c == '[' || c == '(') { char close = c == '{' ? '}' : c == '[' ? ']' : ')'; "#,
        r#"if (i + 1 < s.length() && s.charAt(i + 1) == close) { b.append(c).append(close); i++; continue; } "#,
        r#"depth++; b.append(c).append('\n').append("    ".repeat(depth)); if (c == '{' && i + 1 < s.length() && s.charAt(i + 1) == ' ') i++; continue; } "#,
        r#"if (c == '}' || c == ']' || c == ')') { depth--; while (b.charAt(b.length() - 1) == ' ' || b.charAt(b.length() - 1) == '\n') b.setLength(b.length() - 1); "#,
        r#"if (b.charAt(b.length() - 1) != ',') b.append(','); b.append('\n').append("    ".repeat(depth)).append(c); continue; } "#,
        r#"if (c == ',') { b.append(",\n").append("    ".repeat(depth)); if (i + 1 < s.length() && s.charAt(i + 1) == ' ') i++; continue; } "#,
        r#"b.append(c); } return b.toString(); }"#,
    );
    file_helper(cx, helper.to_string());
    Expr::static_call("debug_pretty", vec![compact])
}

// Debug of a generic value: the Rust text for the runtime class (strings quoted, floats with
// `.0`, collections and Options recursively, crate types through their `debug()`).
fn debug_any(cx: &mut Cx, v: Expr) -> Expr {
    debug_quote(cx, Expr::raw("\"\""), '"');
    debug_float(cx, Expr::raw("0.0"), false);
    // Simple mode's named tuple records print like the tuples they stand for.
    let records: String = crate::tuples::all()
        .iter()
        .map(|n| {
            let parts: Vec<String> = n.fields.iter().map(|f| format!("debug_any(t.{f}())")).collect();
            format!(
                "if (o instanceof {} t) return \"(\" + {} + \")\"; ",
                n.name,
                parts.join(" + \", \" + ")
            )
        })
        .collect();
    let helper = concat!(
        r#"static String debug_any(Object o) { if (o instanceof String s) return debug_str(s, '"'); "#,
        r#"if (o instanceof Character c) return debug_str(String.valueOf(c), '\''); if (o instanceof Double d) return debug_float(d); "#,
        r#"if (o instanceof Optional<?> p) return p.map(x -> "Some(" + debug_any(x) + ")").orElse("None"); "#,
        r#"if (o instanceof Map.Entry<?, ?> e) return "(" + debug_any(e.getKey()) + ", " + debug_any(e.getValue()) + ")"; "#,
        r#"if (o instanceof Map<?, ?> m) { StringJoiner j = new StringJoiner(", ", "{", "}"); m.forEach((k, x) -> j.add(debug_any(k) + ": " + debug_any(x))); return j.toString(); } "#,
        r#"if (o instanceof Collection<?> c) { StringJoiner j = o instanceof Set ? new StringJoiner(", ", "{", "}") : new StringJoiner(", ", "[", "]"); for (Object x : c) j.add(debug_any(x)); return j.toString(); } "#,
        r#"try { return (String) o.getClass().getMethod("debug").invoke(o); } catch (ReflectiveOperationException e) { return String.valueOf(o); } }"#,
    );
    let helper = helper.replacen("if (o instanceof Map<?, ?> m)", &format!("{records}if (o instanceof Map<?, ?> m)"), 1);
    file_helper(cx, helper);
    Expr::static_call("debug_any", vec![v])
}

// Rust's `{:e}`: shortest round-trip digits (or `{:.N}` digits, rounded half-even) as d.ddde±x.
fn sci_float(cx: &mut Cx, v: Expr, f32: bool, precision: Option<Expr>) -> Expr {
    cx.import("java.math.BigDecimal");
    cx.import("java.math.RoundingMode");
    let t = if f32 { "float" } else { "double" };
    file_helper(
        cx,
        format!(
            "static String sci_float({t} x, int p) {{ if (Double.isNaN(x)) return \"NaN\"; if (Double.isInfinite(x)) return x > 0 ? \"inf\" : \"-inf\"; \
             String sign = x < 0 || 1 / x < 0 ? \"-\" : \"\"; BigDecimal d = p < 0 ? new BigDecimal(String.valueOf(Math.abs(x))).stripTrailingZeros() : new BigDecimal(Math.abs(x)); \
             if (d.signum() == 0) return sign + (p > 0 ? \"0.\" + \"0\".repeat(p) : \"0\") + \"e0\"; int e = d.precision() - d.scale() - 1; \
             BigDecimal m = d.movePointLeft(e); if (p >= 0) {{ m = m.setScale(p, RoundingMode.HALF_EVEN); if (m.compareTo(BigDecimal.TEN) >= 0) {{ e++; m = d.movePointLeft(e).setScale(p, RoundingMode.HALF_EVEN); }} }} \
             return sign + m.toPlainString() + \"e\" + e; }}"
        ),
    );
    Expr::static_call(
        "sci_float",
        vec![v, precision.unwrap_or_else(|| Expr::raw("-1"))],
    )
}

// Rust's float Debug: Display plus a forced `.0`, and an exponent outside 1e-4..1e16.
fn debug_float(cx: &mut Cx, v: Expr, f32: bool) -> Expr {
    cx.import("java.math.BigDecimal");
    let t = if f32 { "float" } else { "double" };
    file_helper(
        cx,
        format!(
            "static String debug_float({t} x) {{ String s = String.valueOf(x); if (s.equals(\"NaN\") || s.equals(\"-0.0\")) return s; \
             if (s.endsWith(\"Infinity\")) return s.startsWith(\"-\") ? \"-inf\" : \"inf\"; double a = Math.abs(x); \
             if (a >= 1e16 || a > 0 && a < 1e-4) return s.replace(\"E\", \"e\").replace(\".0e\", \"e\"); \
             String p = new BigDecimal(s).stripTrailingZeros().toPlainString(); return p.contains(\".\") ? p : p + \".0\"; }}"
        ),
    );
    Expr::static_call("debug_float", vec![v])
}

// Debug text as the right operand of a String `+`, where Java's own toString suffices.
fn debug_operand(cx: &mut Cx, v: Expr, ty: &Ty) -> Expr {
    if debug_is_to_string(ty) {
        v
    } else {
        debug(cx, v, ty)
    }
}

// `{:?}`: Rust Debug as a Java expression.
pub fn debug(cx: &mut Cx, v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Unit => Expr::str_lit("()"),
        Ty::Str => debug_quote(cx, v, '"'),
        Ty::Char => debug_quote(cx, v, '\''),
        Ty::F32 | Ty::F64 => debug_float(cx, v, *ty.strip_refs() == Ty::F32),
        Ty::Vec(inner) | Ty::Deque(inner) => debug_sequence(cx, v, inner, "[", "]"),
        Ty::Set { t, .. } => debug_sequence(cx, v, t, "{", "}"),
        Ty::Map { k, v: value, .. } => {
            let name = cx.fresh_param("fmt_entry");
            let key = debug_operand(cx, Expr::call(Expr::name(&name), "getKey", vec![]), k);
            let val = debug_operand(cx, Expr::call(Expr::name(&name), "getValue", vec![]), value);
            let entry = concat(vec![key, Expr::str_lit(": "), val]);
            let stream = Expr::call(Expr::call(v, "entrySet", vec![]), "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![lambda(cx, name, entry)]);
            let joining = Expr::call(
                Expr::raw("Collectors"),
                "joining",
                vec![Expr::str_lit(", "), Expr::str_lit("{"), Expr::str_lit("}")],
            );
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Option(inner) => {
            let name = cx.fresh_param("fmt_some");
            let value = debug_operand(cx, Expr::name(&name), inner);
            let some = concat(vec![Expr::str_lit("Some("), value, Expr::str_lit(")")]);
            Expr::call(
                Expr::call(v, "map", vec![lambda(cx, name, some)]),
                "orElse",
                vec![Expr::str_lit("None")],
            )
        }
        Ty::Result(ok, err) => {
            // Arm bindings, scoped to their arm like lambda parameters.
            let ok_name = cx.fresh_param("fmt_ok");
            let err_name = cx.fresh_param("fmt_err");
            let ok_value = debug_operand(cx, Expr::name(&ok_name), ok);
            let err_value = debug_operand(cx, Expr::name(&err_name), err);
            cx.release_param(&ok_name);
            cx.release_param(&err_name);
            let wrap = |name: &str, value| {
                concat(vec![
                    Expr::str_lit(&format!("{name}(")),
                    value,
                    Expr::str_lit(")"),
                ])
            };
            Expr::Switch {
                scrut: Box::new(v),
                arms: vec![
                    Arm {
                        line: 1,
                        label: format!("Ok<{}, {}>(var {ok_name})", ok.boxed(), err.boxed()),
                        body: ArmBody::Expr(wrap("Ok", ok_value)),
                    },
                    Arm {
                        line: 1,
                        label: format!("Err<{}, {}>(var {err_name})", ok.boxed(), err.boxed()),
                        body: ArmBody::Expr(wrap("Err", err_value)),
                    },
                ],
            }
        }
        Ty::Tuple(ts) => {
            let mut parts = vec![Expr::str_lit("(")];
            for (i, t) in ts.iter().enumerate() {
                if i != 0 {
                    parts.push(Expr::str_lit(", "));
                }
                let field = expr::tuple_field(v.clone(), i, ts);
                parts.push(debug_operand(cx, field, t));
            }
            if ts.len() == 1 {
                parts.push(Expr::str_lit(","));
            }
            parts.push(Expr::str_lit(")"));
            concat(parts)
        }
        Ty::Int(i) if unsigned_display(*i) => unsigned_string(*i, v.clone()).unwrap_or(v),
        Ty::Adt { .. } if crate::lower::items::debug_via_to_string(cx, ty) => {
            Expr::call(Expr::raw("String"), "valueOf", vec![v])
        }
        Ty::Adt { .. } => Expr::call(v, "debug", vec![]),
        Ty::Param(_) => debug_any(cx, v),
        // `Ordering` is an int: comparing its sign is shorter than the switch, when repeatable.
        Ty::Ordering if is_pure(&v) => Expr::Ternary(
            Box::new(Expr::bin(v.clone(), "<", Expr::raw("0"))),
            Box::new(Expr::str_lit("Less")),
            Box::new(Expr::Ternary(
                Box::new(Expr::bin(v, "==", Expr::raw("0"))),
                Box::new(Expr::str_lit("Equal")),
                Box::new(Expr::str_lit("Greater")),
            )),
        ),
        Ty::Ordering => Expr::Switch {
            scrut: Box::new(signum(v)),
            arms: vec![
                Arm {
                    line: 1,
                    label: "-1".into(),
                    body: ArmBody::Expr(Expr::str_lit("Less")),
                },
                Arm {
                    line: 1,
                    label: "0".into(),
                    body: ArmBody::Expr(Expr::str_lit("Equal")),
                },
                Arm {
                    line: 1,
                    label: "default".into(),
                    body: ArmBody::Expr(Expr::str_lit("Greater")),
                },
            ],
        },
        Ty::Other(_) => match std_error_debug(cx, v.clone(), ty) {
            Some(e) => e,
            None => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
        },
        _ => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
    }
}

// Joins pieces with `+`, making sure Java treats the chain as string concatenation. Nested
// concatenations and adjacent literals are spliced so `"Some(" + ("(" + x + ")") + ")"` comes out
// as `"Some((" + x + "))"`.
// `f.debug_struct("P").field("x", &self.x).finish()` (and debug_tuple/list/set/map): the text
// `#[derive(Debug)]` would print, appended to the formatter in one go.
pub fn lower_debug_builder(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let finish = method_name_of(m)?;
    if !matches!(finish.as_str(), "finish" | "finish_non_exhaustive") {
        return None;
    }
    // Walk the chain back to `f.debug_*(..)`.
    let mut steps: Vec<(String, Vec<ast::Expr>)> = Vec::new();
    let mut cur = m.receiver()?;
    let (kind, title, formatter) = loop {
        let ast::Expr::MethodCallExpr(call) = super::strip_parens(&cur) else {
            return None;
        };
        let name = method_name_of(&call)?;
        let args: Vec<ast::Expr> = ast::HasArgList::arg_list(&call)
            .map(|a| a.args().collect())
            .unwrap_or_default();
        if name.starts_with("debug_") {
            break (name, args.into_iter().next(), call.receiver()?);
        }
        steps.push((name, args));
        cur = call.receiver()?;
    };
    steps.reverse();
    let strip_ref = |e: &ast::Expr| match e {
        ast::Expr::RefExpr(r) => r.expr().unwrap_or_else(|| e.clone()),
        _ => e.clone(),
    };
    let mut items: Vec<Expr> = Vec::new();
    let mut joined: Option<Expr> = None;
    for (name, args) in &steps {
        match (name.as_str(), args.as_slice()) {
            ("field", [n, v]) if kind == "debug_struct" => {
                let label = super::expr::lower_expr(cx, n, out);
                let v = strip_ref(v);
                let ty = cx.ty(&v);
                let val = super::expr::lower_expr(cx, &v, out);
                items.push(concat(vec![
                    label,
                    Expr::str_lit(": "),
                    debug(cx, val, &ty),
                ]));
            }
            ("field", [v]) | ("entry", [v]) if kind != "debug_struct" && kind != "debug_map" => {
                let v = strip_ref(v);
                let ty = cx.ty(&v);
                let val = super::expr::lower_expr(cx, &v, out);
                items.push(debug(cx, val, &ty));
            }
            ("entry", [k, v]) if kind == "debug_map" => {
                let (k, v) = (strip_ref(k), strip_ref(v));
                let (kt, vt) = (cx.ty(&k), cx.ty(&v));
                let (kv, vv) = (
                    super::expr::lower_expr(cx, &k, out),
                    super::expr::lower_expr(cx, &v, out),
                );
                items.push(concat(vec![
                    debug(cx, kv, &kt),
                    Expr::str_lit(": "),
                    debug(cx, vv, &vt),
                ]));
            }
            ("entries", [it]) if joined.is_none() && cx.lambda_depth == 0 => {
                let ty = cx.ty(it);
                let item_ty = match ty.strip_refs() {
                    Ty::Iter(t) | Ty::Vec(t) | Ty::Deque(t) | Ty::Set { t, .. } => (**t).clone(),
                    Ty::Map { k, v, .. } => Ty::Tuple(vec![(**k).clone(), (**v).clone()]),
                    _ => return None,
                };
                let src = super::expr::lower_expr(cx, it, out);
                let stream = match ty.strip_refs() {
                    Ty::Iter(_) => src,
                    Ty::Map { .. } => {
                        Expr::call(Expr::call(src, "entrySet", vec![]), "stream", vec![])
                    }
                    _ => Expr::call(src, "stream", vec![]),
                };
                let x = cx.fresh_param("item");
                let text = match (&kind[..], &item_ty) {
                    ("debug_map", Ty::Tuple(kv)) if kv.len() == 2 => {
                        let k = debug(cx, Expr::call(Expr::name(&x), "getKey", vec![]), &kv[0]);
                        let v = debug(cx, Expr::call(Expr::name(&x), "getValue", vec![]), &kv[1]);
                        concat(vec![k, Expr::str_lit(": "), v])
                    }
                    _ => debug(cx, Expr::name(&x), &item_ty),
                };
                cx.release_param(&x);
                let mapper = Expr::Lambda {
                    params: vec![x],
                    body: crate::jir::LambdaBody::Expr(Box::new(debug_string(text))),
                };
                let mapped = Expr::call(stream, "map", vec![mapper]);
                joined = Some(Expr::call(
                    mapped,
                    "collect",
                    vec![Expr::raw("Collectors.joining(\", \")")],
                ));
            }
            _ => return None,
        }
    }
    // A joined stream may be empty, so it can't share separators with single entries.
    if joined.is_some() && !items.is_empty() {
        return None;
    }
    let mut body: Vec<Expr> = Vec::new();
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            body.push(Expr::str_lit(", "));
        }
        body.push(item);
    }
    body.extend(joined);
    let more = finish == "finish_non_exhaustive";
    let title = match title {
        Some(t) => super::expr::lower_expr(cx, &t, out),
        None => Expr::str_lit(""),
    };
    let text = match kind.as_str() {
        "debug_struct" => {
            let mut parts = vec![title];
            match (body.is_empty(), more) {
                (true, false) => {}
                (true, true) => parts.push(Expr::str_lit(" { .. }")),
                (false, _) => {
                    parts.push(Expr::str_lit(" { "));
                    parts.extend(body);
                    parts.push(Expr::str_lit(if more { ", .. }" } else { " }" }));
                }
            }
            concat(parts)
        }
        "debug_tuple" if body.is_empty() => title,
        "debug_tuple" => concat(
            [
                vec![title, Expr::str_lit("(")],
                body,
                vec![Expr::str_lit(")")],
            ]
            .concat(),
        ),
        "debug_list" => concat([vec![Expr::str_lit("[")], body, vec![Expr::str_lit("]")]].concat()),
        "debug_set" | "debug_map" => {
            concat([vec![Expr::str_lit("{")], body, vec![Expr::str_lit("}")]].concat())
        }
        _ => return None,
    };
    let f = super::expr::lower_expr(cx, &formatter, out);
    out.push(Stmt::new(
        cx.line(m.syntax()),
        StmtKind::Expr(Expr::call(f, "append", vec![text])),
    ));
    Some(Expr::raw("null"))
}

fn method_name_of(m: &ast::MethodCallExpr) -> Option<String> {
    m.name_ref().map(|n| n.text().to_string())
}

// A lambda body must be a String even when the debug text is a bare number.
fn debug_string(e: Expr) -> Expr {
    match e {
        Expr::Raw(ref s) if s.starts_with('"') => e,
        Expr::Binary(ref l, "+", _) if matches!(l.as_ref(), Expr::Raw(s) if s.starts_with('"')) => {
            e
        }
        e => Expr::bin(Expr::str_lit(""), "+", e),
    }
}

fn concat(parts: Vec<Expr>) -> Expr {
    let mut flat = Vec::new();
    for p in parts {
        flatten_concat(p, &mut flat);
    }
    concat_typed(
        merge_literals(flat)
            .into_iter()
            .map(|p| {
                let s = is_string_lit(&p);
                (p, s)
            })
            .collect(),
    )
}

// Only a chain whose leftmost operand is a String can be spliced into a longer concatenation
// without changing a numeric `+`.
fn flatten_concat(e: Expr, out: &mut Vec<Expr>) {
    match e {
        Expr::Binary(l, "+", r) if starts_with_literal(&l) => {
            flatten_concat(*l, out);
            flatten_concat(*r, out);
        }
        other => out.push(other),
    }
}

fn starts_with_literal(e: &Expr) -> bool {
    match e {
        Expr::Binary(l, _, _) => starts_with_literal(l),
        _ => is_string_lit(e),
    }
}

fn merge_literals(parts: Vec<Expr>) -> Vec<Expr> {
    let mut out: Vec<Expr> = Vec::new();
    for p in parts {
        if let (Some(Expr::Raw(last)), Expr::Raw(next)) = (out.last_mut(), &p)
            && let (Some(a), Some(b)) = (literal_body(last), literal_body(next))
        {
            *last = format!("\"{a}{b}\"");
            continue;
        }
        out.push(p);
    }
    out
}

// The body of a plain Java string literal, e.g. `Some(` for `"Some("`.
fn literal_body(s: &str) -> Option<&str> {
    s.strip_prefix('"')?.strip_suffix('"')
}

// `a + b + ...` is String concatenation once either of the first two operands is a String.
fn concat_typed(parts: Vec<(Expr, bool)>) -> Expr {
    let mut it = parts.into_iter();
    let Some((first, first_str)) = it.next() else {
        return Expr::str_lit("");
    };
    let rest: Vec<(Expr, bool)> = it.collect();
    let needs_string = !first_str && rest.first().is_none_or(|(_, s)| !s);
    let mut acc = if needs_string {
        Expr::call(Expr::raw("String"), "valueOf", vec![first])
    } else {
        first
    };
    for (p, _) in rest {
        acc = Expr::bin(acc, "+", p);
    }
    acc
}

fn is_string_lit(e: &Expr) -> bool {
    matches!(e, Expr::Raw(s) if s.starts_with('"'))
}

#[allow(dead_code)]
fn literal_text(l: &ast::String) -> String {
    l.text().to_string()
}
