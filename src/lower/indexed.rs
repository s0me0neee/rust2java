// Iterators over positions of lists and strings (`enumerate`, `zip`, `windows`, `chunks`), lowered
// to an index: counted `for` loops, and `IntStream.range(0, n).mapToObj(i -> ..)` chains whose
// closure reads `v.get(i)` directly instead of unpacking a `Map.Entry`.

use ra_ap_syntax::{AstNode, ast, ast::HasArgList, ast::HasName};

use super::iter::{ensure_simple, list_receiver_of_iter, method_name};
use super::{Cx, expr, strip_parens};
use crate::emit::render_expr as render;
use crate::jir::{Expr, Stmt};
use crate::ty::{Int, Ty};

pub(super) struct Side {
    list: ast::Expr,
    skip: Option<ast::Expr>,
}

pub(super) enum Shape {
    // `v.iter().enumerate()` / `s.chars().enumerate()`: (index, element).
    Enumerate { seq: ast::Expr, text: bool },
    // `s.lines().enumerate()` / `s.split(..).enumerate()`: the pieces collected into a list first.
    Pieces { src: ast::Expr, name: &'static str },
    // `a.iter().zip(b.iter().skip(1))`: (a[i], b[i + 1]).
    Zip(Side, Side),
    // `v.windows(k)` / `v.chunks(k)`: one sub-list per step.
    Windows { list: ast::Expr, k: ast::Expr },
    Chunks { list: ast::Expr, k: ast::Expr },
}

impl Shape {
    // How many values each item has (a pair, or one sub-list).
    pub(super) fn arity(&self) -> usize {
        match self {
            Shape::Enumerate { .. } | Shape::Pieces { .. } | Shape::Zip(..) => 2,
            Shape::Windows { .. } | Shape::Chunks { .. } => 1,
        }
    }
}

#[derive(Clone)]
pub(crate) enum Item {
    Index,
    // `seq.get(i + offset)`, or `seq.charAt(i)` for text.
    Elem {
        seq: String,
        offset: Option<String>,
        text: bool,
    },
    // `list.subList(i, i + k)`, clamped to the end for chunks.
    Sub {
        list: String,
        k: String,
        chunk: bool,
    },
}

impl Item {
    pub(crate) fn at(&self, idx: &str) -> String {
        match self {
            Item::Index => idx.to_string(),
            Item::Elem {
                offset: Some(k), ..
            } => format!("{idx} + {k}"),
            Item::Elem { .. } | Item::Sub { .. } => idx.to_string(),
        }
    }

    fn expr(&self, idx: &str) -> Expr {
        match self {
            Item::Index => Expr::name(idx),
            Item::Elem { seq, text, .. } => Expr::call(
                Expr::name(seq.clone()),
                if *text { "charAt" } else { "get" },
                vec![Expr::name(self.at(idx))],
            ),
            Item::Sub { list, k, chunk } => {
                let end = if *chunk {
                    format!("Math.min({idx} + {k}, {list}.size())")
                } else {
                    format!("{idx} + {k}")
                };
                Expr::call(
                    Expr::name(list.clone()),
                    "subList",
                    vec![Expr::name(idx), Expr::raw(end)],
                )
            }
        }
    }
}

pub(super) struct Indexed {
    cond: String,
    step: String,
    // Exclusive bound for `IntStream.range(0, ..)`; `None` when the step isn't 1.
    bound: Option<String>,
    pub(super) items: Vec<Item>,
}

impl Indexed {
    fn sub(s: &str, idx: &str) -> String {
        s.replace("{i}", idx)
    }

    pub(super) fn header(&self, idx: &str) -> String {
        format!(
            "int {idx} = 0; {}; {}",
            Self::sub(&self.cond, idx),
            Self::sub(&self.step, idx)
        )
    }
}

pub(super) fn shape(cx: &Cx, e: &ast::Expr) -> Option<Shape> {
    let ast::Expr::MethodCallExpr(m) = strip_parens(e) else {
        return None;
    };
    let recv = m.receiver()?;
    let arg = || m.arg_list()?.args().next();
    match method_name(&m).as_str() {
        "enumerate" => {
            if let Some(list) = list_receiver_of_iter(cx, &recv) {
                return Some(Shape::Enumerate {
                    seq: list,
                    text: false,
                });
            }
            let ast::Expr::MethodCallExpr(c) = strip_parens(&recv) else {
                return None;
            };
            let s = c.receiver()?;
            if !matches!(cx.ty(&s).strip_refs(), Ty::Str) {
                return None;
            }
            let name = match method_name(&c).as_str() {
                "chars" => return Some(Shape::Enumerate { seq: s, text: true }),
                "lines" => "lines",
                "split_whitespace" => "words",
                "split" | "split_terminator" | "splitn" | "rsplit" => "parts",
                _ => return None,
            };
            // Collecting is a statement before the loop, which a lambda body can't have.
            (cx.lambda_depth == 0).then_some(Shape::Pieces { src: recv, name })
        }
        "zip" => Some(Shape::Zip(side(cx, &recv)?, side(cx, &arg()?)?)),
        "windows" if matches!(cx.ty(&recv).strip_refs(), Ty::Vec(_)) => Some(Shape::Windows {
            list: recv,
            k: arg()?,
        }),
        "chunks" if matches!(cx.ty(&recv).strip_refs(), Ty::Vec(_)) => Some(Shape::Chunks {
            list: recv,
            k: arg()?,
        }),
        _ => None,
    }
}

// One side of a `zip`: a list's iterator, possibly `.skip(k)`.
fn side(cx: &Cx, e: &ast::Expr) -> Option<Side> {
    if let ast::Expr::MethodCallExpr(m) = strip_parens(e)
        && method_name(&m) == "skip"
    {
        let list = list_receiver_of_iter(cx, &m.receiver()?)?;
        return Some(Side {
            list,
            skip: Some(m.arg_list()?.args().next()?),
        });
    }
    Some(Side {
        list: list_receiver_of_iter(cx, e)?,
        skip: None,
    })
}

pub(super) fn lower(cx: &mut Cx, shape: &Shape, line: u32, out: &mut Vec<Stmt>) -> Indexed {
    let seq = |cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>| {
        let ty = cx.ty(e);
        let v = expr::lower_expr(cx, e, out);
        render(&ensure_simple(cx, v, &ty, line, out))
    };
    let count = |cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>| {
        let v = expr::lower_expr(cx, e, out);
        render(&ensure_simple(cx, v, &Ty::Int(Int::Usize), line, out))
    };
    match shape {
        Shape::Pieces { src, name } => {
            let stream = expr::lower_expr(cx, src, out);
            let list = super::std_map::push_local(
                cx,
                line,
                "List<String>".into(),
                Some(Expr::call(stream, "toList", vec![])),
                out,
                name,
            );
            let size = format!("{list}.size()");
            Indexed {
                cond: format!("{{i}} < {size}"),
                step: "{i}++".into(),
                bound: Some(size),
                items: vec![
                    Item::Index,
                    Item::Elem {
                        seq: list,
                        offset: None,
                        text: false,
                    },
                ],
            }
        }
        Shape::Enumerate { seq: s, text } => {
            let s = seq(cx, s, out);
            let size = if *text {
                format!("{s}.length()")
            } else {
                format!("{s}.size()")
            };
            Indexed {
                cond: format!("{{i}} < {size}"),
                step: "{i}++".into(),
                bound: Some(size),
                items: vec![
                    Item::Index,
                    Item::Elem {
                        seq: s,
                        offset: None,
                        text: *text,
                    },
                ],
            }
        }
        Shape::Zip(a, b) => {
            let (la, lb) = (seq(cx, &a.list, out), seq(cx, &b.list, out));
            let ka = a.skip.as_ref().map(|k| count(cx, k, out));
            let kb = b.skip.as_ref().map(|k| count(cx, k, out));
            let len = |l: &str, k: &Option<String>| match k {
                Some(k) => format!("{l}.size() - {k}"),
                None => format!("{l}.size()"),
            };
            // Neighbors of one list: `i + 1 < v.size()`.
            let (cond, bound) = match (&ka, &kb) {
                (None, Some(k)) | (Some(k), None) if la == lb => (
                    format!("{{i}} + {k} < {la}.size()"),
                    len(&la, &Some(k.clone())),
                ),
                _ if la == lb && ka == kb => (format!("{{i}} < {}", len(&la, &ka)), len(&la, &ka)),
                _ => {
                    let min = format!("Math.min({}, {})", len(&la, &ka), len(&lb, &kb));
                    (format!("{{i}} < {min}"), min)
                }
            };
            Indexed {
                cond,
                step: "{i}++".into(),
                bound: Some(bound),
                items: vec![
                    Item::Elem {
                        seq: la,
                        offset: ka,
                        text: false,
                    },
                    Item::Elem {
                        seq: lb,
                        offset: kb,
                        text: false,
                    },
                ],
            }
        }
        Shape::Windows { list, k } => {
            let l = seq(cx, list, out);
            let k = count(cx, k, out);
            let (cond, bound) = match k.parse::<u64>() {
                Ok(1) => (format!("{{i}} < {l}.size()"), format!("{l}.size()")),
                Ok(n) => (
                    format!("{{i}} + {} < {l}.size()", n - 1),
                    format!("{l}.size() - {}", n - 1),
                ),
                Err(_) => (
                    format!("{{i}} + {k} <= {l}.size()"),
                    format!("{l}.size() - {k} + 1"),
                ),
            };
            Indexed {
                cond,
                step: "{i}++".into(),
                bound: Some(bound),
                items: vec![Item::Sub {
                    list: l,
                    k,
                    chunk: false,
                }],
            }
        }
        Shape::Chunks { list, k } => {
            let l = seq(cx, list, out);
            let k = count(cx, k, out);
            Indexed {
                cond: format!("{{i}} < {l}.size()"),
                step: format!("{{i}} += {k}"),
                bound: None,
                items: vec![Item::Sub {
                    list: l,
                    k,
                    chunk: true,
                }],
            }
        }
    }
}

// The per-item patterns: a pair's two fields, or the one pattern for a sub-list.
pub(super) fn pattern_fields(p: &ast::Pat, shape: &Shape) -> Option<Vec<ast::Pat>> {
    if shape.arity() == 1 {
        return Some(vec![p.clone()]);
    }
    let ast::Pat::TuplePat(tp) = p else {
        return None;
    };
    let fields: Vec<ast::Pat> = tp.fields().collect();
    (fields.len() == 2).then_some(fields)
}

// The value of each item field for index `idx`.
pub(super) fn item_exprs(ix: &Indexed, idx: &str) -> Vec<Expr> {
    ix.items.iter().map(|i| i.expr(idx)).collect()
}

// `.map(|(a, b)| ..)` / `.any(..)` / `.all(..)` straight on a pair source:
// `IntStream.range(0, n).mapToObj(i -> f(a.get(i), b.get(i)))`.
pub(super) fn fused(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    recv: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let jname = match name {
        "map" => "mapToObj",
        "any" => "anyMatch",
        "all" => "allMatch",
        _ => return None,
    };
    let closure_arg =
        |m: &ast::MethodCallExpr| match m.arg_list()?.args().next().map(|a| strip_parens(&a))? {
            ast::Expr::ClosureExpr(c) => closure_fields(&c).map(|_| c),
            _ => None,
        };
    let closure = closure_arg(m)?;
    // `.filter(|(_, line)| ..)` stages before it filter the indices (outermost last).
    let mut filters = Vec::new();
    let mut src = strip_parens(recv);
    while let ast::Expr::MethodCallExpr(f) = &src
        && method_name(f) == "filter"
    {
        filters.push(closure_arg(f)?);
        src = strip_parens(&f.receiver()?);
    }
    let shape = shape(cx, &src).filter(|s| s.arity() == 2)?;
    let line = cx.line(m.syntax());
    let ix = lower(cx, &shape, line, out);
    let bound = ix.bound.clone()?;
    let mut stream = Expr::raw(format!("IntStream.range(0, {bound})"));
    let stages: Vec<_> = filters
        .into_iter()
        .rev()
        .map(|c| ("filter", c))
        .chain([(jname, closure)])
        .collect();
    // Every stage names the index alike, after the last stage that names it at all.
    let rust_index = stages
        .iter()
        .rev()
        .find_map(|(_, c)| index_name(&closure_fields(c)?, &ix.items));
    let saved = std::mem::replace(&mut cx.indexed_index, (rust_index, None));
    for (name, c) in stages {
        cx.indexed_params = Some(ix.items.clone());
        let f = expr::lower_expr(cx, &ast::Expr::ClosureExpr(c), out);
        cx.indexed_params = None;
        // `.map(|(i, _)| i)` keeps the indices themselves.
        let text = render(&f);
        if name == "mapToObj" && text.split_once(" -> ").is_some_and(|(p, b)| p == b) {
            stream = Expr::call(stream, "boxed", vec![]);
            continue;
        }
        if name == "mapToObj"
            && let Ty::Iter(item) = cx.ty(&ast::Expr::MethodCallExpr(m.clone())).strip_refs()
            && let Some(w) = super::iter::map_witness(item, &f)
        {
            stream = Expr::call(stream, &format!("<{w}>mapToObj"), vec![f]);
            continue;
        }
        stream = Expr::call(stream, name, vec![f]);
    }
    cx.indexed_index = saved;
    Some(stream)
}

// The fields of a closure's one `(a, b)` parameter, each a plain binding or `_`.
fn closure_fields(c: &ast::ClosureExpr) -> Option<Vec<Option<ast::IdentPat>>> {
    let params: Vec<ast::Param> = c.param_list()?.params().collect();
    let [param] = params.as_slice() else {
        return None;
    };
    let ast::Pat::TuplePat(tp) = param.pat()? else {
        return None;
    };
    tp.fields()
        .map(|f| match strip_ref(f) {
            ast::Pat::WildcardPat(_) => Some(None),
            ast::Pat::IdentPat(ip) if ip.pat().is_none() && ip.mut_token().is_none() => {
                Some(Some(ip))
            }
            _ => None,
        })
        .collect()
}

// The Rust name a closure gives the index field, if any.
fn index_name(fields: &[Option<ast::IdentPat>], items: &[Item]) -> Option<String> {
    fields
        .iter()
        .zip(items)
        .find(|(_, i)| matches!(i, Item::Index))
        .and_then(|(f, _)| Some(f.as_ref()?.name()?.text().to_string()))
}

fn strip_ref(p: ast::Pat) -> ast::Pat {
    match &p {
        ast::Pat::RefPat(r) => r.pat().map(strip_ref).unwrap_or(p),
        _ => p,
    }
}

// Inside the closure's scope: its parameter is the index, and each element field reads
// `seq.get(i)` where it's used. Returns the Java parameter name.
pub(super) fn closure_params(cx: &mut Cx, c: &ast::ClosureExpr, items: &[Item]) -> Option<String> {
    let fields = closure_fields(c)?;
    if fields.len() != items.len() {
        return None;
    }
    let index_name = index_name(&fields, items);
    let idx = if let Some(java) = cx.indexed_index.1.clone() {
        // A later stage of the chain: the same name as the first.
        if let Some(n) = &index_name {
            cx.declare_as(n, &java);
        }
        java
    } else {
        let idx = match index_name.or_else(|| cx.indexed_index.0.clone()) {
            Some(n) => cx.declare_param(&n),
            // The lambda's names are released after it, so `i` stays free for later Rust locals.
            None if crate::mode::simple() => cx
                .claim_first(&["i", "j", "k"])
                .unwrap_or_else(|| cx.fresh_param("i")),
            None => cx.fresh_param("i"),
        };
        cx.indexed_index.1 = Some(idx.clone());
        idx
    };
    for (f, item) in fields.iter().zip(items) {
        let (Some(ip), Item::Elem { seq, text, .. }) = (f, item) else {
            continue;
        };
        let Some(local) = cx.sema.to_def(ip) else {
            return None;
        };
        // Format strings' inline `{x}` find it by name.
        cx.declare_as(&ip.name()?.text().to_string(), &render(&item.expr(&idx)));
        let kind = if *text {
            super::ElemAliasKind::Char
        } else {
            super::ElemAliasKind::List
        };
        cx.elem_aliases
            .insert(local, (seq.clone(), item.at(&idx), kind));
    }
    Some(idx)
}
