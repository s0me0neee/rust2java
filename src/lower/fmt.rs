// Format strings (`format_args!` family) -> Java string concatenation / String.format.

use ra_ap_hir::ScopeDef;
use ra_ap_syntax::{AstNode, AstToken, SyntaxNode, ast, ast::HasName};

use super::std_map::{unsigned_display, unsigned_string};
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
    if !matches!(kind.as_str(), "" | "?" | "x" | "X" | "o" | "b") {
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
    for a in fa.args() {
        let Some(e) = a.expr() else { continue };
        let ty = if matches!(&e, ast::Expr::MacroExpr(m) if m.macro_call().is_some_and(|mc| matches!(super::macros::macro_name(&mc).as_str(), "format_args" | "format_args_nl")))
        {
            Ty::Str
        } else {
            cx.ty(&e)
        };
        let mut pre = Vec::new();
        let v = expr::lower_expr(cx, &e, &mut pre);
        args.push((a.name().map(|n| n.text().to_string()), v, ty, pre));
    }
    let temps = args_needing_temps(&pieces, &specs, &args);
    let mut positional: Vec<(Expr, Ty)> = Vec::new();
    let mut named: Vec<(String, Expr, Ty)> = Vec::new();
    for ((name, v, ty, pre), temp) in args.into_iter().zip(temps) {
        out.extend(pre);
        let v = if temp {
            let tmp = cx.fresh("fmt_arg");
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
    if spec.alternate && value {
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
        if spec.kind.is_empty() && matches!(ty.strip_refs(), Ty::Int(i) if unsigned_display(*i)) {
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
pub fn is_target_typed(e: &Expr) -> bool {
    matches!(e, Expr::Call { name, .. } if name == "collect")
}

// Java's `toString` already prints Rust's Debug text for these (`[1, 2, 3]`, `true`).
fn debug_is_to_string(ty: &Ty) -> bool {
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

fn is_pure(e: &Expr) -> bool {
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
    if matches!(ty.strip_refs(), Ty::Other(_)) && std_error_name(ty).is_none() {
        return cx.unsupported(node, &format!("format argument type {ty:?}"));
    }
    let numeric = matches!(ty.strip_refs(), Ty::Int(_) | Ty::F32 | Ty::F64);
    if spec.alternate && spec.kind == "?" {
        return cx.unsupported(node, "pretty Debug format");
    }
    if spec.kind == "?" {
        if !debug_supported(ty) {
            return cx.unsupported(node, &format!("Debug for {ty:?}"));
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
        let tmp = hoist_string(cx, node, text, out);
        let negative = Expr::call(tmp.clone(), "startsWith", vec![Expr::str_lit("-")]);
        text = Expr::Ternary(
            Box::new(negative),
            Box::new(tmp.clone()),
            Box::new(Expr::bin(Expr::str_lit("+"), "+", tmp)),
        );
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
        let raw = crate::emit::render_expr(&tmp);
        let prefix = if spec.alternate && !spec.kind.is_empty() {
            2
        } else {
            0
        };
        let sign = Expr::raw(format!(
            "({raw}.startsWith(\"-\") || {raw}.startsWith(\"+\") ? 1 : 0) + {prefix}"
        ));
        let left = Expr::call(tmp.clone(), "substring", vec![Expr::raw("0"), sign.clone()]);
        let zeros = Expr::call(Expr::str_lit("0"), "repeat", vec![pad]);
        let right = Expr::call(tmp, "substring", vec![sign]);
        return Expr::bin(Expr::bin(left, "+", zeros), "+", right);
    }
    let align = spec.align.unwrap_or(if numeric { '>' } else { '<' });
    let fill = Expr::str_lit(&spec.fill.to_string());
    match align {
        '<' => Expr::bin(tmp, "+", Expr::call(fill, "repeat", vec![pad])),
        '>' => Expr::bin(Expr::call(fill, "repeat", vec![pad]), "+", tmp),
        _ => {
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
pub fn display(cx: &mut Cx, node: &SyntaxNode, v: Expr, ty: &Ty, out: &mut Vec<Stmt>) -> Expr {
    if let Some(e) = std_error_display(cx, v.clone(), ty) {
        return e;
    }
    match ty.strip_refs() {
        Ty::F32 | Ty::F64 => display_float(cx, node, v, *ty.strip_refs() == Ty::F32, out),
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

pub fn std_error_display(cx: &mut Cx, v: Expr, ty: &Ty) -> Option<Expr> {
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

// Rust's float Display: no exponent, `-0`, `NaN`, `inf`; Java's own toString differs in all four.
fn display_float(cx: &mut Cx, node: &SyntaxNode, v: Expr, f32: bool, out: &mut Vec<Stmt>) -> Expr {
    cx.import("java.math.BigDecimal");
    let simple = matches!(&v, Expr::Name(_))
        || matches!(&v, Expr::Raw(s) if s.chars().all(|c| c.is_ascii_digit() || matches!(c, '.' | '-')));
    let v = if simple || cx.lambda_depth > 0 {
        v
    } else {
        let name = cx.fresh("fmt_f");
        out.push(Stmt::new(
            cx.line(node),
            StmtKind::Local {
                ty: if f32 { "float" } else { "double" }.into(),
                name: name.clone(),
                init: Some(v),
            },
        ));
        Expr::name(name)
    };
    let cls = if f32 { "Float" } else { "Double" };
    let r = crate::emit::render_expr(&v);
    let plain = format!("new BigDecimal(String.valueOf({r})).stripTrailingZeros().toPlainString()");
    Expr::raw(format!(
        "({cls}.isNaN({r}) ? \"NaN\" : {r} == {cls}.POSITIVE_INFINITY ? \"inf\" : \
         {r} == {cls}.NEGATIVE_INFINITY ? \"-inf\" : \
         String.valueOf({r}).equals(\"-0.0\") ? \"-0\" : {plain})"
    ))
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
        return lambda(name, body);
    }
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
    match ty.strip_refs() {
        Ty::Vec(inner) | Ty::Deque(inner) | Ty::Set { t: inner, .. } => {
            let name = cx.fresh("fmt_item");
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
            let name = cx.fresh("fmt_entry");
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
            let item = Expr::bin(Expr::bin(key, "+", Expr::str_lit(": ")), "+", val);
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
            let name = cx.fresh("fmt_some");
            let mut pre = Vec::new();
            let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
            let some = Expr::bin(
                Expr::bin(Expr::str_lit("Some("), "+", item),
                "+",
                Expr::str_lit(")"),
            );
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
                let field = expr::tuple_field(value.clone(), i, ts.len());
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
                let text = Expr::bin(
                    Expr::bin(Expr::str_lit(&format!("{variant}(")), "+", item),
                    "+",
                    Expr::str_lit(")"),
                );
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

fn lambda(name: String, body: Expr) -> Expr {
    Expr::Lambda {
        params: vec![name],
        body: LambdaBody::Expr(Box::new(body)),
    }
}

fn debug_quote(cx: &mut Cx, value: Expr, quote: char) -> Expr {
    let name = cx.fresh("fmt_char");
    let c = Expr::name(&name);
    let unicode = Expr::bin(
        Expr::bin(
            Expr::str_lit("\\u{"),
            "+",
            Expr::call(Expr::raw("Integer"), "toHexString", vec![c.clone()]),
        ),
        "+",
        Expr::str_lit("}"),
    );
    let character = Expr::New {
        ty: "String".into(),
        args: vec![Expr::call(
            Expr::raw("Character"),
            "toChars",
            vec![c.clone()],
        )],
    };
    let mut escaped = Expr::Ternary(
        Box::new(Expr::call(
            Expr::raw("Character"),
            "isISOControl",
            vec![c.clone()],
        )),
        Box::new(unicode),
        Box::new(character),
    );
    for (ch, replacement) in [
        ('\\', "\\\\"),
        ('\n', "\\n"),
        ('\r', "\\r"),
        ('\t', "\\t"),
        ('\0', "\\0"),
        (quote, if quote == '"' { "\\\"" } else { "\\'" }),
    ] {
        escaped = Expr::Ternary(
            Box::new(Expr::bin(
                c.clone(),
                "==",
                Expr::raw((ch as u32).to_string()),
            )),
            Box::new(Expr::str_lit(replacement)),
            Box::new(escaped),
        );
    }
    let stream = Expr::call(
        Expr::call(Expr::raw("String"), "valueOf", vec![value]),
        "codePoints",
        vec![],
    );
    let mapped = Expr::call(stream, "mapToObj", vec![lambda(name, escaped)]);
    let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![]);
    let escaped = Expr::call(mapped, "collect", vec![joining]);
    Expr::bin(
        Expr::bin(Expr::str_lit(&quote.to_string()), "+", escaped),
        "+",
        Expr::str_lit(&quote.to_string()),
    )
}

fn debug_sequence(cx: &mut Cx, v: Expr, inner: &Ty, open: &str, close: &str) -> Expr {
    let name = cx.fresh("fmt_item");
    let item = debug(cx, Expr::name(&name), inner);
    let stream = Expr::call(v, "stream", vec![]);
    let mapped = Expr::call(stream, "map", vec![lambda(name, item)]);
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

fn debug_float(cx: &mut Cx, v: Expr) -> Expr {
    cx.import("java.math.BigDecimal");
    let text = Expr::call(Expr::raw("String"), "valueOf", vec![v.clone()]);
    let plain = Expr::call(
        Expr::call(
            Expr::New {
                ty: "BigDecimal".into(),
                args: vec![text.clone()],
            },
            "stripTrailingZeros",
            vec![],
        ),
        "toPlainString",
        vec![],
    );
    let fraction = Expr::Ternary(
        Box::new(Expr::call(
            plain.clone(),
            "contains",
            vec![Expr::str_lit(".")],
        )),
        Box::new(plain.clone()),
        Box::new(Expr::bin(plain, "+", Expr::str_lit(".0"))),
    );
    let scientific = Expr::call(
        Expr::call(
            text.clone(),
            "replace",
            vec![Expr::str_lit("E"), Expr::str_lit("e")],
        ),
        "replace",
        vec![Expr::str_lit(".0e"), Expr::str_lit("e")],
    );
    let abs = Expr::call(Expr::raw("Math"), "abs", vec![v.clone()]);
    let scientific_range = Expr::bin(
        Expr::bin(abs.clone(), ">=", Expr::raw("1e16")),
        "||",
        Expr::bin(
            Expr::bin(abs.clone(), ">", Expr::raw("0.0")),
            "&&",
            Expr::bin(abs, "<", Expr::raw("1e-4")),
        ),
    );
    let finite = Expr::Ternary(
        Box::new(scientific_range),
        Box::new(scientific),
        Box::new(fraction),
    );
    let special = Expr::Ternary(
        Box::new(Expr::call(
            text.clone(),
            "equals",
            vec![Expr::str_lit("Infinity")],
        )),
        Box::new(Expr::str_lit("inf")),
        Box::new(Expr::Ternary(
            Box::new(Expr::call(
                text.clone(),
                "equals",
                vec![Expr::str_lit("-Infinity")],
            )),
            Box::new(Expr::str_lit("-inf")),
            Box::new(text.clone()),
        )),
    );
    let is_special = Expr::bin(
        Expr::call(Expr::raw("Double"), "isFinite", vec![v]),
        "==",
        Expr::raw("false"),
    );
    let nonzero = Expr::Ternary(Box::new(is_special), Box::new(special), Box::new(finite));
    Expr::Ternary(
        Box::new(Expr::call(text, "equals", vec![Expr::str_lit("-0.0")])),
        Box::new(Expr::str_lit("-0.0")),
        Box::new(nonzero),
    )
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
        Ty::F32 | Ty::F64 => debug_float(cx, v),
        Ty::Vec(inner) | Ty::Deque(inner) => debug_sequence(cx, v, inner, "[", "]"),
        Ty::Set { t, .. } => debug_sequence(cx, v, t, "{", "}"),
        Ty::Map { k, v: value, .. } => {
            let name = cx.fresh("fmt_entry");
            let key = debug_operand(cx, Expr::call(Expr::name(&name), "getKey", vec![]), k);
            let val = debug_operand(cx, Expr::call(Expr::name(&name), "getValue", vec![]), value);
            let entry = Expr::bin(Expr::bin(key, "+", Expr::str_lit(": ")), "+", val);
            let stream = Expr::call(Expr::call(v, "entrySet", vec![]), "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![lambda(name, entry)]);
            let joining = Expr::call(
                Expr::raw("Collectors"),
                "joining",
                vec![Expr::str_lit(", "), Expr::str_lit("{"), Expr::str_lit("}")],
            );
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Option(inner) => {
            let name = cx.fresh("fmt_some");
            let value = debug_operand(cx, Expr::name(&name), inner);
            let some = Expr::bin(
                Expr::bin(Expr::str_lit("Some("), "+", value),
                "+",
                Expr::str_lit(")"),
            );
            Expr::call(
                Expr::call(v, "map", vec![lambda(name, some)]),
                "orElse",
                vec![Expr::str_lit("None")],
            )
        }
        Ty::Result(ok, err) => {
            let ok_name = cx.fresh("fmt_ok");
            let err_name = cx.fresh("fmt_err");
            let ok_value = debug_operand(cx, Expr::name(&ok_name), ok);
            let err_value = debug_operand(cx, Expr::name(&err_name), err);
            let wrap = |name: &str, value| {
                Expr::bin(
                    Expr::bin(Expr::str_lit(&format!("{name}(")), "+", value),
                    "+",
                    Expr::str_lit(")"),
                )
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
                let field = expr::tuple_field(v.clone(), i, ts.len());
                parts.push(debug_operand(cx, field, t));
            }
            if ts.len() == 1 {
                parts.push(Expr::str_lit(","));
            }
            parts.push(Expr::str_lit(")"));
            concat(parts)
        }
        Ty::Ordering => {
            // Rust names the variants; the Java value only keeps its sign.
            let less = Expr::bin(v.clone(), "<", Expr::raw("0"));
            let greater = Expr::bin(v, ">", Expr::raw("0"));
            Expr::Ternary(
                Box::new(less),
                Box::new(Expr::str_lit("Less")),
                Box::new(Expr::Ternary(
                    Box::new(greater),
                    Box::new(Expr::str_lit("Greater")),
                    Box::new(Expr::str_lit("Equal")),
                )),
            )
        }
        Ty::Int(i) if unsigned_display(*i) => unsigned_string(*i, v.clone()).unwrap_or(v),
        Ty::Adt { .. } => Expr::call(v, "debug", vec![]),
        // `Ordering` is an int; only its sign matters.
        Ty::Ordering => Expr::Switch {
            scrut: Box::new(Expr::call(Expr::raw("Integer"), "signum", vec![v])),
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

// Joins pieces with `+`, making sure Java treats the chain as string concatenation.
fn concat(parts: Vec<Expr>) -> Expr {
    concat_typed(
        parts
            .into_iter()
            .map(|p| {
                let s = is_string_lit(&p);
                (p, s)
            })
            .collect(),
    )
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
