// Format strings (`format_args!` family) -> Java string concatenation / String.format.

use ra_ap_hir::ScopeDef;
use ra_ap_syntax::{AstNode, AstToken, SyntaxNode, ast, ast::HasName};

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

enum ArgRef {
    Next,
    Index(usize),
    Name(String),
}

enum Count {
    Fixed(String),
    Argument(ArgRef),
    Star,
}

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
        while chars.get(*pos).is_some_and(char::is_ascii_digit) { *pos += 1; }
    } else {
        while chars.get(*pos).is_some_and(|c| c.is_alphanumeric() || *c == '_') { *pos += 1; }
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
    if plus { i += 1; }
    let alternate = chars.get(i) == Some(&'#');
    if alternate { i += 1; }
    let zero = chars.get(i) == Some(&'0');
    if zero { i += 1; }
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
    Some(Spec { fill, align, plus, alternate, zero, width, precision, kind })
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

fn lower_format_impl(cx: &mut Cx, fa: &ast::FormatArgsExpr, out: &mut Vec<Stmt>, strip_nl: bool) -> Expr {
    let template = match fa.template() {
        Some(ast::Expr::Literal(l)) => match l.kind() {
            ast::LiteralKind::String(s) => s.value().map(|v| v.to_string()).ok(),
            _ => None,
        },
        _ => None,
    };
    let Some(template) = template else { return cx.unsupported(fa.syntax(), "non-literal format string") };
    let Some(mut pieces) = parse_template(&template) else { return cx.unsupported(fa.syntax(), "format string") };
    if strip_nl && let Some(Piece::Lit(s)) = pieces.last_mut() {
        if s.ends_with('\n') { s.pop(); }
        if s.is_empty() {
            pieces.pop();
        }
    }
    // Positional and named arguments, lowered once each in source order.
    let mut positional: Vec<(Expr, Ty)> = Vec::new();
    let mut named: Vec<(String, Expr, Ty)> = Vec::new();
    for a in fa.args() {
        let Some(e) = a.expr() else { continue };
        let ty = if matches!(&e, ast::Expr::MacroExpr(m) if m.macro_call().is_some_and(|mc| matches!(super::macros::macro_name(&mc).as_str(), "format_args" | "format_args_nl"))) {
            Ty::Str
        } else {
            cx.ty(&e)
        };
        let v = expr::lower_expr(cx, &e, out);
        let tmp = cx.fresh("fmt_arg");
        out.push(Stmt::new(cx.line(fa.syntax()), StmtKind::Local {
            ty: if matches!(ty.strip_refs(), Ty::Unit) { "Void".into() } else { super::stmt::decl_type(&ty) },
            name: tmp.clone(), init: Some(v),
        }));
        let v = Expr::name(tmp);
        match a.name() {
            Some(n) => named.push((n.text().to_string(), v, ty)),
            None => positional.push((v, ty)),
        }
    }
    let mut next = 0;
    let mut parts: Vec<Expr> = Vec::new();
    for p in pieces {
        match p {
            Piece::Lit(s) => parts.push(Expr::str_lit(&s)),
            Piece::Arg { pos, spec } => {
                let Some(parsed) = parse_spec(&spec) else {
                    return cx.unsupported(fa.syntax(), &format!("format spec `{{:{spec}}}`"));
                };
                let precision = parsed.precision.as_ref().map(|c| count_expr(cx, fa.syntax(), c, &positional, &named, &mut next));
                let width = parsed.width.as_ref().map(|c| count_expr(cx, fa.syntax(), c, &positional, &named, &mut next));
                let (v, ty) = resolve_arg(cx, fa.syntax(), &pos, &positional, &named, &mut next);
                let precision = precision.map(|p| hoist_count(cx, fa.syntax(), p, out));
                let width = width.map(|w| hoist_count(cx, fa.syntax(), w, out));
                let counts = Counts { width, precision };
                parts.push(format_one(cx, fa.syntax(), v, &ty, &parsed, &counts, out));
            }
        }
    }
    concat(parts)
}

fn hoist_count(cx: &mut Cx, node: &SyntaxNode, value: Expr, out: &mut Vec<Stmt>) -> Expr {
    if matches!(&value, Expr::Raw(s) if s.chars().all(|c| c.is_ascii_digit())) {
        return value;
    }
    let name = cx.fresh("fmt_count");
    out.push(Stmt::new(cx.line(node), StmtKind::Local { ty: "int".into(), name: name.clone(), init: Some(value) }));
    Expr::name(name)
}

fn resolve_arg(
    cx: &mut Cx, node: &SyntaxNode, arg: &ArgRef, positional: &[(Expr, Ty)],
    named: &[(String, Expr, Ty)], next: &mut usize,
) -> (Expr, Ty) {
    match arg {
        ArgRef::Next => {
            let i = *next;
            *next += 1;
            positional.get(i).cloned().unwrap_or_else(|| (cx.unsupported(node, "format argument count"), Ty::Other("?".into())))
        }
        ArgRef::Index(i) => positional.get(*i).cloned().unwrap_or_else(|| (cx.unsupported(node, "format argument index"), Ty::Other("?".into()))),
        ArgRef::Name(n) => named.iter().find(|(m, _, _)| m == n)
            .map(|(_, v, t)| (v.clone(), t.clone())).unwrap_or_else(|| captured(cx, node, n)),
    }
}

fn count_expr(
    cx: &mut Cx, node: &SyntaxNode, count: &Count, positional: &[(Expr, Ty)],
    named: &[(String, Expr, Ty)], next: &mut usize,
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
                found = Some(l.ty(cx.db));
            }
        });
    }
    let ty = found.map(|t| cx.convert(&t)).unwrap_or(Ty::Other("?".into()));
    (Expr::name(cx.lookup(name)), ty)
}

fn format_one(
    cx: &mut Cx, node: &SyntaxNode, v: Expr, ty: &Ty, spec: &Spec,
    counts: &Counts, out: &mut Vec<Stmt>,
) -> Expr {
    let width = counts.width.clone();
    let precision = counts.precision.clone();
    if matches!(ty.strip_refs(), Ty::Other(_)) {
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
            Ty::Vec(_) | Ty::Deque(_) | Ty::Set { .. } | Ty::Map { .. } | Ty::Option(_) | Ty::Result(_, _) | Ty::Tuple(_) => {
                return debug_with_spec(cx, node, v, ty, spec, counts, out);
            }
            Ty::Adt { .. } if width.is_some() || precision.is_some() || spec.plus || spec.zero => {
                return cx.unsupported(node, "format flags for user Debug");
            }
            _ => {}
        }
    }
    let mut text = if spec.kind == "?" {
        if ty.is_float() && let Some(p) = precision {
            let fmt = Expr::bin(Expr::bin(Expr::str_lit("%."), "+", p), "+", Expr::str_lit("f"));
            Expr::call(Expr::raw("String"), "format", vec![Expr::raw("Locale.ROOT"), fmt, v])
        } else {
            debug(cx, v, ty)
        }
    } else if !spec.kind.is_empty() {
        let Ty::Int(i) = ty.strip_refs() else {
            return cx.unsupported(node, "non-integer radix format");
        };
        let conv = match spec.kind.as_str() {
            "x" => "toHexString", "X" => "toHexString", "o" => "toOctalString",
            "b" => "toBinaryString", _ => unreachable!(),
        };
        let mut number = v;
        if matches!(i, crate::ty::Int::I8 | crate::ty::Int::I16 | crate::ty::Int::U16) {
            let mask = if *i == crate::ty::Int::I8 { "255" } else { "65535" };
            number = Expr::bin(number, "&", Expr::raw(mask));
        }
        let class = if i.is_long() { "Long" } else { "Integer" };
        let mut digits = Expr::call(Expr::raw(class), conv, vec![number]);
        if spec.kind == "X" {
            digits = Expr::call(digits, "toUpperCase", vec![Expr::raw("Locale.ROOT")]);
        }
        if spec.alternate {
            let prefix = match spec.kind.as_str() { "x" => "0x", "X" => "0x", "o" => "0o", _ => "0b" };
            digits = Expr::bin(Expr::str_lit(prefix), "+", digits);
        }
        digits
    } else if let Some(p) = precision {
        if ty.is_float() {
            let fmt = Expr::bin(Expr::bin(Expr::str_lit("%."), "+", p), "+", Expr::str_lit("f"));
            Expr::call(Expr::raw("String"), "format", vec![Expr::raw("Locale.ROOT"), fmt, v])
        } else if ty.is_str() {
            let s = Expr::call(Expr::raw("String"), "valueOf", vec![v]);
            let tmp = hoist_string(cx, node, s, out);
            let len = Expr::call(tmp.clone(), "codePointCount", vec![Expr::raw("0"), Expr::call(tmp.clone(), "length", vec![])]);
            let count = Expr::call(Expr::raw("Math"), "min", vec![p, len]);
            let end = Expr::call(tmp.clone(), "offsetByCodePoints", vec![Expr::raw("0"), count]);
            Expr::call(tmp, "substring", vec![Expr::raw("0"), end])
        } else {
            return cx.unsupported(node, "precision on this format type");
        }
    } else {
        display(cx, v, ty)
    };
    text = Expr::call(Expr::raw("String"), "valueOf", vec![text]);
    if spec.plus && numeric {
        let tmp = hoist_string(cx, node, text, out);
        let negative = Expr::call(tmp.clone(), "startsWith", vec![Expr::str_lit("-")]);
        text = Expr::Ternary(Box::new(negative), Box::new(tmp.clone()), Box::new(Expr::bin(Expr::str_lit("+"), "+", tmp)));
    }
    let Some(width) = width else { return text };
    let tmp = hoist_string(cx, node, text, out);
    let len = Expr::call(tmp.clone(), "codePointCount", vec![Expr::raw("0"), Expr::call(tmp.clone(), "length", vec![])]);
    let pad = Expr::call(Expr::raw("Math"), "max", vec![Expr::raw("0"), Expr::bin(width, "-", len)]);
    if spec.zero && numeric {
        let raw = crate::emit::render_expr(&tmp);
        let prefix = if spec.alternate && !spec.kind.is_empty() { 2 } else { 0 };
        let sign = Expr::raw(format!("({raw}.startsWith(\"-\") || {raw}.startsWith(\"+\") ? 1 : 0) + {prefix}"));
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
            Expr::bin(Expr::bin(Expr::call(fill.clone(), "repeat", vec![left]), "+", tmp), "+", Expr::call(fill, "repeat", vec![right]))
        }
    }
}

fn hoist_string(cx: &mut Cx, node: &SyntaxNode, value: Expr, out: &mut Vec<Stmt>) -> Expr {
    let name = cx.fresh("fmt_text");
    out.push(Stmt::new(cx.line(node), StmtKind::Local { ty: "String".into(), name: name.clone(), init: Some(value) }));
    Expr::name(name)
}

// `{}`: Rust Display as a Java expression.
pub fn display(cx: &mut Cx, v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::F32 | Ty::F64 => {
            cx.import("java.math.BigDecimal");
            let bd = Expr::New { ty: "BigDecimal".into(), args: vec![Expr::call(Expr::raw("String"), "valueOf", vec![v])] };
            Expr::call(Expr::call(bd, "stripTrailingZeros", vec![]), "toPlainString", vec![])
        }
        _ => v,
    }
}

pub fn debug_supported(ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Unit | Ty::Bool | Ty::Char | Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Str | Ty::Adt { .. } => true,
        Ty::Vec(t) | Ty::Set { t, .. } | Ty::Deque(t) | Ty::Option(t) => debug_supported(t),
        Ty::Map { k, v, .. } | Ty::Result(k, v) => debug_supported(k) && debug_supported(v),
        Ty::Tuple(ts) => ts.iter().all(debug_supported),
        _ => false,
    }
}

fn debug_lambda(cx: &mut Cx, node: &SyntaxNode, name: String, body: Expr, mut pre: Vec<Stmt>) -> Expr {
    if pre.is_empty() {
        return lambda(name, body);
    }
    let line = cx.line(node);
    pre.push(Stmt::new(line, StmtKind::Return(Some(body))));
    Expr::Lambda { params: vec![name], body: LambdaBody::Block(crate::jir::Block { stmts: pre, close_line: line }) }
}

fn debug_with_spec(
    cx: &mut Cx, node: &SyntaxNode, value: Expr, ty: &Ty, spec: &Spec,
    counts: &Counts, out: &mut Vec<Stmt>,
) -> Expr {
    match ty.strip_refs() {
        Ty::Vec(inner) | Ty::Deque(inner) | Ty::Set { t: inner, .. } => {
            let name = cx.fresh("fmt_item");
            let mut pre = Vec::new();
            let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
            let mapper = debug_lambda(cx, node, name, item, pre);
            let stream = Expr::call(value, "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![mapper]);
            let (open, close) = if matches!(ty.strip_refs(), Ty::Set { .. }) { ("{", "}") } else { ("[", "]") };
            let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![Expr::str_lit(", "), Expr::str_lit(open), Expr::str_lit(close)]);
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Map { k, v, .. } => {
            let name = cx.fresh("fmt_entry");
            let mut pre = Vec::new();
            let key = format_one(cx, node, Expr::call(Expr::name(&name), "getKey", vec![]), k, spec, counts, &mut pre);
            let val = format_one(cx, node, Expr::call(Expr::name(&name), "getValue", vec![]), v, spec, counts, &mut pre);
            let item = Expr::bin(Expr::bin(key, "+", Expr::str_lit(": ")), "+", val);
            let mapper = debug_lambda(cx, node, name, item, pre);
            let stream = Expr::call(Expr::call(value, "entrySet", vec![]), "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![mapper]);
            let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![Expr::str_lit(", "), Expr::str_lit("{"), Expr::str_lit("}")]);
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Option(inner) => {
            let name = cx.fresh("fmt_some");
            let mut pre = Vec::new();
            let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
            let some = Expr::bin(Expr::bin(Expr::str_lit("Some("), "+", item), "+", Expr::str_lit(")"));
            let mapper = debug_lambda(cx, node, name, some, pre);
            Expr::call(Expr::call(value, "map", vec![mapper]), "orElse", vec![Expr::str_lit("None")])
        }
        Ty::Tuple(ts) => {
            let mut parts = vec![Expr::str_lit("(")];
            for (i, t) in ts.iter().enumerate() {
                if i != 0 { parts.push(Expr::str_lit(", ")); }
                let field = expr::tuple_field(value.clone(), i, ts.len());
                parts.push(format_one(cx, node, field, t, spec, counts, out));
            }
            if ts.len() == 1 { parts.push(Expr::str_lit(",")); }
            parts.push(Expr::str_lit(")"));
            concat(parts)
        }
        Ty::Result(ok, err) => {
            let mut arms = Vec::new();
            for (variant, inner) in [("Ok", ok.as_ref()), ("Err", err.as_ref())] {
                let name = cx.fresh("fmt_value");
                let mut pre = Vec::new();
                let item = format_one(cx, node, Expr::name(&name), inner, spec, counts, &mut pre);
                let text = Expr::bin(Expr::bin(Expr::str_lit(&format!("{variant}(")), "+", item), "+", Expr::str_lit(")"));
                let body = if pre.is_empty() {
                    ArmBody::Expr(text)
                } else {
                    let line = cx.line(node);
                    pre.push(Stmt::new(line, StmtKind::Raw(format!("yield {};", crate::emit::render_expr(&text)))));
                    ArmBody::Block(crate::jir::Block { stmts: pre, close_line: line })
                };
                arms.push(Arm { line: cx.line(node), label: format!("{variant}<{}, {}>(var {name})", ok.boxed(), err.boxed()), body });
            }
            Expr::Switch { scrut: Box::new(value), arms }
        }
        _ => unreachable!(),
    }
}

fn lambda(name: String, body: Expr) -> Expr {
    Expr::Lambda { params: vec![name], body: LambdaBody::Expr(Box::new(body)) }
}

fn debug_quote(cx: &mut Cx, value: Expr, quote: char) -> Expr {
    let name = cx.fresh("fmt_char");
    let c = Expr::name(&name);
    let unicode = Expr::bin(
        Expr::bin(Expr::str_lit("\\u{"), "+", Expr::call(Expr::raw("Integer"), "toHexString", vec![c.clone()])),
        "+", Expr::str_lit("}"),
    );
    let character = Expr::New { ty: "String".into(), args: vec![Expr::call(Expr::raw("Character"), "toChars", vec![c.clone()])] };
    let mut escaped = Expr::Ternary(
        Box::new(Expr::call(Expr::raw("Character"), "isISOControl", vec![c.clone()])),
        Box::new(unicode), Box::new(character),
    );
    for (ch, replacement) in [('\\', "\\\\"), ('\n', "\\n"), ('\r', "\\r"), ('\t', "\\t"), ('\0', "\\0"), (quote, if quote == '"' { "\\\"" } else { "\\'" })] {
        escaped = Expr::Ternary(
            Box::new(Expr::bin(c.clone(), "==", Expr::raw((ch as u32).to_string()))),
            Box::new(Expr::str_lit(replacement)), Box::new(escaped),
        );
    }
    let stream = Expr::call(Expr::call(Expr::raw("String"), "valueOf", vec![value]), "codePoints", vec![]);
    let mapped = Expr::call(stream, "mapToObj", vec![lambda(name, escaped)]);
    let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![]);
    let escaped = Expr::call(mapped, "collect", vec![joining]);
    Expr::bin(Expr::bin(Expr::str_lit(&quote.to_string()), "+", escaped), "+", Expr::str_lit(&quote.to_string()))
}

fn debug_sequence(cx: &mut Cx, v: Expr, inner: &Ty, open: &str, close: &str) -> Expr {
    let name = cx.fresh("fmt_item");
    let item = debug(cx, Expr::name(&name), inner);
    let stream = Expr::call(v, "stream", vec![]);
    let mapped = Expr::call(stream, "map", vec![lambda(name, item)]);
    let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![Expr::str_lit(", "), Expr::str_lit(open), Expr::str_lit(close)]);
    Expr::call(mapped, "collect", vec![joining])
}

fn debug_float(cx: &mut Cx, v: Expr) -> Expr {
    cx.import("java.math.BigDecimal");
    let text = Expr::call(Expr::raw("String"), "valueOf", vec![v.clone()]);
    let plain = Expr::call(
        Expr::call(Expr::New { ty: "BigDecimal".into(), args: vec![text.clone()] }, "stripTrailingZeros", vec![]),
        "toPlainString", vec![],
    );
    let fraction = Expr::Ternary(
        Box::new(Expr::call(plain.clone(), "contains", vec![Expr::str_lit(".")])),
        Box::new(plain.clone()), Box::new(Expr::bin(plain, "+", Expr::str_lit(".0"))),
    );
    let scientific = Expr::call(
        Expr::call(text.clone(), "replace", vec![Expr::str_lit("E"), Expr::str_lit("e")]),
        "replace", vec![Expr::str_lit(".0e"), Expr::str_lit("e")],
    );
    let abs = Expr::call(Expr::raw("Math"), "abs", vec![v.clone()]);
    let scientific_range = Expr::bin(
        Expr::bin(abs.clone(), ">=", Expr::raw("1e16")), "||",
        Expr::bin(Expr::bin(abs.clone(), ">", Expr::raw("0.0")), "&&", Expr::bin(abs, "<", Expr::raw("1e-4"))),
    );
    let finite = Expr::Ternary(Box::new(scientific_range), Box::new(scientific), Box::new(fraction));
    let special = Expr::Ternary(
        Box::new(Expr::call(text.clone(), "equals", vec![Expr::str_lit("Infinity")])),
        Box::new(Expr::str_lit("inf")),
        Box::new(Expr::Ternary(
            Box::new(Expr::call(text.clone(), "equals", vec![Expr::str_lit("-Infinity")])),
            Box::new(Expr::str_lit("-inf")), Box::new(text.clone()),
        )),
    );
    let is_special = Expr::bin(
        Expr::call(Expr::raw("Double"), "isFinite", vec![v]), "==", Expr::raw("false"),
    );
    let nonzero = Expr::Ternary(Box::new(is_special), Box::new(special), Box::new(finite));
    Expr::Ternary(
        Box::new(Expr::call(text, "equals", vec![Expr::str_lit("-0.0")])),
        Box::new(Expr::str_lit("-0.0")), Box::new(nonzero),
    )
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
            let key = debug(cx, Expr::call(Expr::name(&name), "getKey", vec![]), k);
            let val = debug(cx, Expr::call(Expr::name(&name), "getValue", vec![]), value);
            let entry = Expr::bin(Expr::bin(key, "+", Expr::str_lit(": ")), "+", val);
            let stream = Expr::call(Expr::call(v, "entrySet", vec![]), "stream", vec![]);
            let mapped = Expr::call(stream, "map", vec![lambda(name, entry)]);
            let joining = Expr::call(Expr::raw("Collectors"), "joining", vec![Expr::str_lit(", "), Expr::str_lit("{"), Expr::str_lit("}")]);
            Expr::call(mapped, "collect", vec![joining])
        }
        Ty::Option(inner) => {
            let name = cx.fresh("fmt_some");
            let value = debug(cx, Expr::name(&name), inner);
            let some = Expr::bin(Expr::bin(Expr::str_lit("Some("), "+", value), "+", Expr::str_lit(")"));
            Expr::call(Expr::call(v, "map", vec![lambda(name, some)]), "orElse", vec![Expr::str_lit("None")])
        }
        Ty::Result(ok, err) => {
            let ok_name = cx.fresh("fmt_ok");
            let err_name = cx.fresh("fmt_err");
            let ok_value = debug(cx, Expr::name(&ok_name), ok);
            let err_value = debug(cx, Expr::name(&err_name), err);
            let wrap = |name: &str, value| Expr::bin(Expr::bin(Expr::str_lit(&format!("{name}(")), "+", value), "+", Expr::str_lit(")"));
            Expr::Switch { scrut: Box::new(v), arms: vec![
                Arm { line: 1, label: format!("Ok<{}, {}>(var {ok_name})", ok.boxed(), err.boxed()), body: ArmBody::Expr(wrap("Ok", ok_value)) },
                Arm { line: 1, label: format!("Err<{}, {}>(var {err_name})", ok.boxed(), err.boxed()), body: ArmBody::Expr(wrap("Err", err_value)) },
            ] }
        }
        Ty::Tuple(ts) => {
            let mut parts = vec![Expr::str_lit("(")];
            for (i, t) in ts.iter().enumerate() {
                if i != 0 { parts.push(Expr::str_lit(", ")); }
                let field = expr::tuple_field(v.clone(), i, ts.len());
                parts.push(debug(cx, field, t));
            }
            if ts.len() == 1 { parts.push(Expr::str_lit(",")); }
            parts.push(Expr::str_lit(")"));
            concat(parts)
        }
        Ty::Adt { .. } => Expr::call(v, "debug", vec![]),
        _ => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
    }
}

// Joins pieces with `+`, making sure Java treats the chain as string concatenation.
fn concat(parts: Vec<Expr>) -> Expr {
    let mut it = parts.into_iter();
    let Some(first) = it.next() else { return Expr::str_lit("") };
    let rest: Vec<Expr> = it.collect();
    let first = if rest.is_empty() || is_string_lit(&first) {
        first
    } else {
        Expr::call(Expr::raw("String"), "valueOf", vec![first])
    };
    let mut acc = if rest.is_empty() && !is_string_lit(&first) {
        Expr::call(Expr::raw("String"), "valueOf", vec![first])
    } else {
        first
    };
    for p in rest {
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
