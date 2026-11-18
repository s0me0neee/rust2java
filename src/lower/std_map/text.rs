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
        // `char::from(u8)`: the argument may be a boxed `Integer` when iterating a list.
        "char::from" => {
            let args = expr::call_args(cx, c, out);
            let b = args.into_iter().next()?;
            Some(Expr::Cast(
                "char".into(),
                Box::new(Expr::Cast("int".into(), Box::new(b))),
            ))
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
    super::option::parse_result(cx, m, recv, ret, &target, line, out)
}
