// Format strings (`format_args!` family) -> Java string concatenation / String.format.

use ra_ap_hir::ScopeDef;
use ra_ap_syntax::{AstNode, AstToken, SyntaxNode, ast, ast::HasName};

use super::{Cx, expr};
use crate::jir::{Expr, Stmt};
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
    if strip_nl {
        if let Some(Piece::Lit(s)) = pieces.last_mut() {
            s.pop();
            if s.is_empty() {
                pieces.pop();
            }
        }
    }
    // Positional and named arguments, lowered once each in source order.
    let mut positional: Vec<(Expr, Ty)> = Vec::new();
    let mut named: Vec<(String, Expr, Ty)> = Vec::new();
    for a in fa.args() {
        let Some(e) = a.expr() else { continue };
        let ty = cx.ty(&e);
        let v = expr::lower_expr(cx, &e, out);
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
                let (v, ty) = match pos {
                    ArgRef::Next => {
                        next += 1;
                        match positional.get(next - 1) {
                            Some(x) => x.clone(),
                            None => return cx.unsupported(fa.syntax(), "format argument count"),
                        }
                    }
                    ArgRef::Index(i) => match positional.get(i) {
                        Some(x) => x.clone(),
                        None => return cx.unsupported(fa.syntax(), "format argument index"),
                    },
                    ArgRef::Name(n) => match named.iter().find(|(m, _, _)| *m == n) {
                        Some((_, v, t)) => (v.clone(), t.clone()),
                        None => captured(cx, fa.syntax(), &n),
                    },
                };
                parts.push(format_one(cx, fa.syntax(), v, &ty, &spec));
            }
        }
    }
    concat(parts)
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
            let e = if is_cell { Expr::raw(format!("{java}[0]")) } else { Expr::name(java) };
            (e, cx.convert(&t))
        }
        None => (Expr::name(java), Ty::Other("?".into())),
    }
}

fn format_one(cx: &mut Cx, node: &SyntaxNode, v: Expr, ty: &Ty, spec: &str) -> Expr {
    match spec {
        "" => display(cx, v, ty),
        "?" => debug(cx, v, ty),
        s if s.starts_with('.') && ty.is_float() && s[1..].chars().all(|c| c.is_ascii_digit()) => {
            Expr::call(Expr::raw("String"), "format", vec![Expr::str_lit(&format!("%{s}f")), v])
        }
        _ => cx.unsupported(node, &format!("format spec `{{:{spec}}}`")),
    }
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

// `{:?}`: Rust Debug as a Java expression.
pub fn debug(cx: &mut Cx, v: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Str => Expr::bin(Expr::bin(Expr::str_lit("\""), "+", v), "+", Expr::str_lit("\"")),
        Ty::Char => Expr::bin(Expr::bin(Expr::str_lit("'"), "+", v), "+", Expr::str_lit("'")),
        Ty::F32 | Ty::F64 => Expr::call(Expr::raw("String"), "valueOf", vec![v]),
        Ty::Vec(inner) if inner.is_primitive() && !inner.is_float() && **inner != Ty::Char => v,
        Ty::Adt { .. } | Ty::Result(..) => Expr::call(v, "debug", vec![]),
        Ty::Vec(inner) | Ty::Deque(inner) => {
            let x = cx.fresh("x");
            let inner_dbg = debug(cx, Expr::name(x.clone()), inner);
            let mapped = Expr::call(
                Expr::call(v, "stream", vec![]),
                "map",
                vec![Expr::Lambda { params: vec![x], body: crate::jir::LambdaBody::Expr(Box::new(inner_dbg)) }],
            );
            Expr::call(mapped, "collect", vec![Expr::raw("Collectors.joining(\", \", \"[\", \"]\")")])
        }
        Ty::Option(inner) => {
            let x = cx.fresh("x");
            let inner_dbg = debug(cx, Expr::name(x.clone()), inner);
            let some = Expr::bin(Expr::bin(Expr::str_lit("Some("), "+", inner_dbg), "+", Expr::str_lit(")"));
            let mapped = Expr::call(v, "map", vec![Expr::Lambda { params: vec![x], body: crate::jir::LambdaBody::Expr(Box::new(some)) }]);
            Expr::call(mapped, "orElse", vec![Expr::str_lit("None")])
        }
        _ => {
            let _ = cx;
            v
        }
    }
}

// Joins pieces with `+`, making sure Java treats the chain as string concatenation.
fn concat(parts: Vec<Expr>) -> Expr {
    let mut it = parts.into_iter();
    let Some(first) = it.next() else { return Expr::str_lit("") };
    let rest: Vec<Expr> = it.collect();
    let first = if rest.is_empty() || is_string_lit(&first) || rest.first().is_some_and(is_string_lit) {
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
