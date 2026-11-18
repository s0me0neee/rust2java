// Std Vec/slices/arrays, HashMap/BTreeMap, sets, VecDeque, BinaryHeap, entry API.

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

// `entry()` idioms
pub(super) fn is_entry_recv(e: &ast::Expr) -> bool {
    matches!(e, ast::Expr::MethodCallExpr(mc) if method_name(mc) == "entry")
}

// `m.entry(k)` chains: or_insert / or_insert_with / or_default -> computeIfAbsent.
pub(super) fn entry_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    args: &[Expr],
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    if !matches!(name, "or_insert" | "or_insert_with" | "or_default") {
        return None;
    }
    let recv_ast = m.receiver()?;
    let ast::Expr::MethodCallExpr(entry) = &recv_ast else {
        return None;
    };
    if method_name(entry) != "entry" {
        return None;
    }
    let map_ast = entry.receiver()?;
    let key = entry.arg_list()?.args().next()?;
    let map = expr::lower_expr(cx, &map_ast, out);
    let key = expr::lower_expr(cx, &key, out);
    let map_ty = cx.ty(&map_ast);
    let v_ty = match map_ty.strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        _ => Ty::Other("?".into()),
    };
    let value = match name {
        "or_insert" => args.first().cloned().unwrap_or_else(|| Expr::raw("null")),
        "or_insert_with" => {
            let a = m.arg_list()?.args().next()?;
            ctor_value(cx, &a, &v_ty, out)?
        }
        _ => default_value(&v_ty),
    };
    let ignored = cx.fresh("ignored");
    let f = Expr::Lambda {
        params: vec![ignored],
        body: LambdaBody::Expr(Box::new(value)),
    };
    Some(Expr::call(map, "computeIfAbsent", vec![key, f]))
}

// Value produced by a `FnOnce() -> T` argument such as `Vec::new` or `|| expr`.
pub(super) fn ctor_value(cx: &mut Cx, a: &ast::Expr, ty: &Ty, out: &mut Vec<Stmt>) -> Option<Expr> {
    if let ast::Expr::ClosureExpr(c) = a {
        if c.param_list()
            .is_some_and(|pl| pl.params().next().is_some())
        {
            return None;
        }
        let body = c.body()?;
        return Some(expr::lower_expr(cx, &body, out));
    }
    let text = a.syntax().text().to_string();
    let last = text.rsplit("::").next().unwrap_or(&text);
    Some(match last {
        "new" if text.contains("Vec") => Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![],
        },
        "new" if text.contains("String") => Expr::str_lit(""),
        "new" if text.contains("HashMap") => Expr::New {
            ty: "HashMap<>".into(),
            args: vec![],
        },
        "new" if text.contains("HashSet") => Expr::New {
            ty: "HashSet<>".into(),
            args: vec![],
        },
        "default" => default_value(ty),
        _ => return None,
    })
}

// `*m.entry(k).or_insert(v) op= r` -> put / merge. Called from expr::lower_assign.
pub fn lower_entry_assign(
    cx: &mut Cx,
    lhs: &ast::Expr,
    op: Option<ast::ArithOp>,
    rhs: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let ast::Expr::PrefixExpr(p) = lhs else {
        return None;
    };
    if p.op_kind() != Some(ast::UnaryOp::Deref) {
        return None;
    }
    let inner = p.expr()?;
    let ast::Expr::MethodCallExpr(mc) = &inner else {
        return None;
    };
    if !matches!(
        method_name(mc).as_str(),
        "or_insert" | "or_insert_with" | "or_default"
    ) {
        return None;
    }
    let ast::Expr::MethodCallExpr(entry) = mc.receiver()? else {
        return None;
    };
    if method_name(&entry) != "entry" {
        return None;
    }
    let map_ast = entry.receiver()?;
    let key_ast = entry.arg_list()?.args().next()?;
    let map_ty = cx.ty(&map_ast);
    let v_ty = match map_ty.strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        _ => Ty::Other("?".into()),
    };
    let map = expr::lower_expr(cx, &map_ast, out);
    let key = expr::lower_expr(cx, &key_ast, out);
    let r = expr::lower_expr(cx, rhs, out);
    match op {
        None => Some(Expr::call(map, "put", vec![key, r])),
        Some(a) => {
            let (x, y) = (cx.fresh("acc"), cx.fresh("delta"));
            let body = match a {
                ast::ArithOp::Add => Expr::bin(Expr::name(x.clone()), "+", Expr::name(y.clone())),
                ast::ArithOp::Sub => Expr::bin(Expr::name(x.clone()), "-", Expr::name(y.clone())),
                ast::ArithOp::Mul => Expr::bin(Expr::name(x.clone()), "*", Expr::name(y.clone())),
                ast::ArithOp::Div => Expr::bin(Expr::name(x.clone()), "/", Expr::name(y.clone())),
                ast::ArithOp::Rem => Expr::bin(Expr::name(x.clone()), "%", Expr::name(y.clone())),
                ast::ArithOp::BitOr => Expr::bin(Expr::name(x.clone()), "|", Expr::name(y.clone())),
                ast::ArithOp::BitAnd => {
                    Expr::bin(Expr::name(x.clone()), "&", Expr::name(y.clone()))
                }
                ast::ArithOp::BitXor => {
                    Expr::bin(Expr::name(x.clone()), "^", Expr::name(y.clone()))
                }
                _ => return None,
            };
            let _ = v_ty;
            let f = Expr::Lambda {
                params: vec![x, y],
                body: LambdaBody::Expr(Box::new(body)),
            };
            Some(Expr::call(map, "merge", vec![key, r, f]))
        }
    }
}

// ---- Vec / slices ----------------------------------------------------------------------------

pub(super) fn lower_vec_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    elem: &Ty,
    args: Vec<Expr>,
    arg_asts: &[ast::Expr],
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("0"));
    let witness = opt_inner(ret);
    match name {
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "push" => Some(Expr::call(recv, "add", vec![b0])),
        "pop" => {
            let cond = Expr::call(recv.clone(), "isEmpty", vec![]);
            Some(conditional(
                cond,
                opt_empty(witness.as_ref()),
                opt_of(Expr::call(recv, "removeLast", vec![])),
            ))
        }
        "get" => {
            if let Some(ast::Expr::RangeExpr(r)) = arg_asts.first() {
                let sub = lower_slice_range(cx, recv, &Ty::Vec(Box::new(elem.clone())), r, out);
                return Some(opt_of(sub));
            }
            let cond = Expr::bin(b0.clone(), "<", Expr::call(recv.clone(), "size", vec![]));
            Some(conditional(
                cond,
                opt_of(Expr::call(recv, "get", vec![b0])),
                opt_empty(witness.as_ref()),
            ))
        }
        "first" => Some(Expr::call(
            Expr::call(recv, "stream", vec![]),
            "findFirst",
            vec![],
        )),
        "last" => {
            let cond = Expr::call(recv.clone(), "isEmpty", vec![]);
            Some(conditional(
                cond,
                opt_empty(witness.as_ref()),
                opt_of(Expr::call(recv, "getLast", vec![])),
            ))
        }
        "insert" => Some(Expr::call(recv, "add", vec![b0, b1])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "swap" => Some(Expr::call(
            Expr::raw("Collections"),
            "swap",
            vec![recv, b0, b1],
        )),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "truncate" => {
            let p = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (ps, ns) = (render(&p), render(&n));
            push_raw(
                out,
                line,
                format!("if ({ps}.size() > {ns}) {ps}.subList({ns}, {ps}.size()).clear();"),
            );
            Some(Expr::raw("null"))
        }
        "extend" => {
            let at = arg_asts
                .first()
                .map(|a| cx.ty(a))
                .unwrap_or_else(|| Ty::Other("?".into()));
            let arg = if matches!(at.strip_refs(), Ty::Iter(_)) {
                Expr::call(b0, "collect", vec![Expr::raw("Collectors.toList()")])
            } else {
                b0
            };
            Some(Expr::call(recv, "addAll", vec![arg]))
        }
        "retain" => {
            let f = negate_lambda(b0);
            Some(Expr::call(recv, "removeIf", vec![f]))
        }
        "dedup" => {
            let p = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let ps = render(&p);
            let i = cx.fresh("i");
            push_raw(
                out,
                line,
                format!(
                    "for (int {i} = {ps}.size() - 1; {i} > 0; {i}--) if ({ps}.get({i}).equals({ps}.get({i} - 1))) {ps}.remove({i});"
                ),
            );
            Some(Expr::raw("null"))
        }
        "sort" | "sort_unstable" => Some(Expr::call(recv, "sort", vec![Expr::raw("null")])),
        "sort_by" => Some(Expr::call(recv, "sort", vec![b0])),
        "sort_by_key" => {
            let key_ty = match arg_asts.first() {
                Some(ast::Expr::ClosureExpr(c)) => c.body().map(|b| cx.ty(&b)),
                _ => None,
            };
            let cmp = match key_ty.as_ref().map(|t| t.strip_refs().clone()) {
                Some(Ty::Int(i)) if i.is_long() => "comparingLong",
                Some(Ty::Int(_)) => "comparingInt",
                Some(Ty::F32 | Ty::F64) => "comparingDouble",
                _ => "comparing",
            };
            let f = Expr::call(Expr::raw("Comparator"), cmp, vec![typed_lambda(b0, elem)]);
            Some(Expr::call(recv, "sort", vec![f]))
        }
        "sort_by_cached_key" => Some(Expr::call(Expr::raw("Comparator"), "comparing", vec![b0])),
        "reverse" => Some(Expr::call(Expr::raw("Collections"), "reverse", vec![recv])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "binary_search" | "binary_search_by" => {
            cx.needs_result = true;
            let n = push_local(
                cx,
                line,
                "int".into(),
                Some(Expr::call(
                    Expr::raw("Collections"),
                    "binarySearch",
                    vec![recv, b0],
                )),
                out,
                "pos",
            );
            let idx = Expr::name(n);
            Some(conditional(
                Expr::bin(idx.clone(), ">=", Expr::raw("0")),
                Expr::New {
                    ty: "Ok<Integer, Integer>".into(),
                    args: vec![idx.clone()],
                },
                Expr::New {
                    ty: "Err<Integer, Integer>".into(),
                    args: vec![Expr::bin(
                        Expr::Unary("-", Box::new(idx)),
                        "-",
                        Expr::raw("1"),
                    )],
                },
            ))
        }
        "split_at" => {
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let a = Expr::call(recv.clone(), "subList", vec![Expr::raw("0"), i.clone()]);
            let b = Expr::call(
                recv.clone(),
                "subList",
                vec![i, Expr::call(recv, "size", vec![])],
            );
            Some(Expr::call(Expr::raw("Map"), "entry", vec![a, b]))
        }
        "clone" => Some(clone_expr(recv, &Ty::Vec(Box::new(elem.clone())))),
        "as_slice" | "as_mut" | "as_mut_slice" | "into_boxed_slice" => Some(recv),
        "reserve" | "shrink_to_fit" => Some(Expr::raw("null")),
        _ => {
            let _ = m;
            None
        }
    }
}

// Give the single lambda parameter its Java type where javac cannot infer it.
pub(super) fn typed_lambda(f: Expr, elem: &Ty) -> Expr {
    match f {
        Expr::Lambda { mut params, body } if params.len() == 1 && !params[0].contains(' ') => {
            params[0] = format!("{} {}", elem.boxed(), params[0]);
            Expr::Lambda { params, body }
        }
        other => other,
    }
}

pub(super) fn negate_lambda(f: Expr) -> Expr {
    match f {
        Expr::Lambda {
            params,
            body: LambdaBody::Expr(e),
        } => Expr::Lambda {
            params,
            body: LambdaBody::Expr(Box::new(Expr::Unary("!", e))),
        },
        other => other,
    }
}

// ---- maps ------------------------------------------------------------------------------------

pub(super) fn lower_map_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    ordered: bool,
    _k: &Ty,
    _v: &Ty,
    args: Vec<Expr>,
    _arg_asts: &[ast::Expr],
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let b1 = args.get(1).cloned().unwrap_or_else(|| Expr::raw("null"));
    match name {
        "insert" => {
            let put = Expr::call(recv, "put", vec![b0, b1]);
            if discarded(m) {
                Some(put)
            } else {
                Some(Expr::call(Expr::raw("Optional"), "ofNullable", vec![put]))
            }
        }
        "get" | "get_mut" => Some(Expr::call(
            Expr::raw("Optional"),
            "ofNullable",
            vec![Expr::call(recv, "get", vec![b0])],
        )),
        "remove" => {
            let call = Expr::call(recv, "remove", vec![b0]);
            if discarded(m) {
                Some(call)
            } else {
                Some(Expr::call(Expr::raw("Optional"), "ofNullable", vec![call]))
            }
        }
        "contains_key" => Some(Expr::call(recv, "containsKey", vec![b0])),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "extend" => Some(Expr::call(recv, "putAll", vec![b0])),
        "clone" => Some(Expr::New {
            ty: if ordered { "TreeMap<>" } else { "HashMap<>" }.into(),
            args: vec![recv],
        }),
        "entry" => {
            let _ = m;
            Some(cx.unsupported(m.syntax(), "entry API (only or_insert* is supported)"))
        }
        "get_or_insert_with" => {
            let _ = (recv_ast, ret, line, out);
            None
        }
        _ => None,
    }
}

// ---- sets ------------------------------------------------------------------------------------

pub(super) fn lower_set_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    ordered: bool,
    _elem: &Ty,
    args: Vec<Expr>,
    _arg_asts: &[ast::Expr],
    name: &str,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let _ = ordered;
    match name {
        "insert" => Some(Expr::call(recv, "add", vec![b0])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "extend" => Some(Expr::call(recv, "addAll", vec![b0])),
        "union" => {
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!(
                "Stream.concat({s}.stream(), {o}.stream()).distinct()"
            )))
        }
        "intersection" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!(
                "{s}.stream().filter({x} -> {o}.contains({x}))"
            )))
        }
        "difference" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!(
                "{s}.stream().filter({x} -> !{o}.contains({x}))"
            )))
        }
        "symmetric_difference" => {
            let x = cx.fresh("x");
            let s = render(&recv);
            let o = render(&b0);
            Some(Expr::raw(format!(
                "Stream.concat({s}.stream().filter({x} -> !{o}.contains({x})), {o}.stream().filter({x} -> !{s}.contains({x})))"
            )))
        }
        "is_subset" => {
            let x = cx.fresh("x");
            Some(Expr::raw(format!(
                "{}.stream().allMatch({x} -> {}.contains({x}))",
                render(&recv),
                render(&b0)
            )))
        }
        "is_superset" => {
            let x = cx.fresh("x");
            Some(Expr::raw(format!(
                "{}.stream().allMatch({x} -> {}.contains({x}))",
                render(&b0),
                render(&recv)
            )))
        }
        "clone" => Some(Expr::New {
            ty: if ordered { "TreeSet<>" } else { "HashSet<>" }.into(),
            args: vec![recv],
        }),
        _ => {
            let _ = (line, out);
            None
        }
    }
}

// ---- VecDeque / BinaryHeap ------------------------------------------------------------------

pub(super) fn lower_deque_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    _elem: &Ty,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, ret, line, out);
    let b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("0"));
    let opt_nullable = |f: &str| {
        Expr::call(
            Expr::raw("Optional"),
            "ofNullable",
            vec![Expr::call(recv.clone(), f, vec![])],
        )
    };
    match name {
        "push_back" | "push" => Some(Expr::call(recv, "addLast", vec![b0])),
        "push_front" => Some(Expr::call(recv, "addFirst", vec![b0])),
        "pop_front" | "pop" => Some(opt_nullable("pollFirst")),
        "pop_back" => Some(opt_nullable("pollLast")),
        "front" => Some(opt_nullable("peekFirst")),
        "back" => Some(opt_nullable("peekLast")),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "extend" => Some(Expr::call(recv, "addAll", vec![b0])),
        "get" => Some(Expr::call(
            Expr::call(Expr::call(recv, "stream", vec![]), "skip", vec![b0]),
            "findFirst",
            vec![],
        )),
        "clone" => Some(Expr::New {
            ty: "ArrayDeque<>".into(),
            args: vec![recv],
        }),
        "iter" | "into_iter" => Some(Expr::call(recv, "stream", vec![])),
        _ => {
            let _ = m;
            None
        }
    }
}

pub(super) fn lower_heap_method(
    cx: &mut Cx,
    _m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, ret, line, out);
    let opt_nullable = |f: &str| {
        Expr::call(
            Expr::raw("Optional"),
            "ofNullable",
            vec![Expr::call(recv.clone(), f, vec![])],
        )
    };
    match name {
        "push" => Some(Expr::call(
            recv,
            "add",
            vec![args.into_iter().next().unwrap_or_else(|| Expr::raw("null"))],
        )),
        "pop" => Some(opt_nullable("poll")),
        "peek" => Some(opt_nullable("peek")),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "into_sorted_vec" => Some(Expr::call(
            Expr::call(recv, "stream", vec![]),
            "sorted",
            vec![],
        )),
        "clone" => Some(Expr::New {
            ty: "PriorityQueue<>".into(),
            args: vec![recv],
        }),
        _ => None,
    }
}

// ---- arrays / slicing ------------------------------------------------------------------------

// `[a, b, c]` -> mutable list; `[x; n]` -> n independent copies when `x` is a mutable object.
pub fn lower_array_expr(cx: &mut Cx, a: &ast::ArrayExpr, out: &mut Vec<Stmt>) -> Expr {
    let exprs: Vec<ast::Expr> = a.exprs().collect();
    if a.semicolon_token().is_some() && exprs.len() == 2 {
        return lower_repeat(cx, a.syntax(), &exprs[0], &exprs[1], out);
    }
    let items: Vec<Expr> = exprs.iter().map(|e| expr::lower_expr(cx, e, out)).collect();
    if items.is_empty() {
        return Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![],
        };
    }
    // `()` elements are `null` in Java, which `List.of` rejects.
    let factory = if exprs.iter().any(|e| cx.ty(e).is_unit()) {
        ("Arrays", "asList")
    } else {
        ("List", "of")
    };
    let list = Expr::call(Expr::raw(factory.0), factory.1, items);
    Expr::New {
        ty: "ArrayList<>".into(),
        args: vec![list],
    }
}

// `[x; n]` / `vec![x; n]`.
pub fn lower_repeat(
    cx: &mut Cx,
    node: &ra_ap_syntax::SyntaxNode,
    elem: &ast::Expr,
    count: &ast::Expr,
    out: &mut Vec<Stmt>,
) -> Expr {
    let elem_ty = cx.ty(elem);
    let n = expr::lower_expr(cx, count, out);
    let n = simple(cx, n, &Ty::Int(Int::Usize), cx.line(node), out);
    if crate::lower::is_cell_value_ty(&elem_ty) || matches!(elem_ty.strip_refs(), Ty::Option(_)) {
        let x = expr::lower_expr(cx, elem, out);
        let copies = Expr::call(Expr::raw("Collections"), "nCopies", vec![n, x]);
        return Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![copies],
        };
    }
    // Re-evaluate the element expression per slot: Rust clones it n times.
    let mut inner = Vec::new();
    cx.lambda_depth += 1;
    let x = expr::lower_expr(cx, elem, &mut inner);
    cx.lambda_depth -= 1;
    if !inner.is_empty() {
        return cx.unsupported(node, "repeat-array element needing statements");
    }
    let i = cx.fresh("i");
    let range = Expr::call(Expr::raw("IntStream"), "range", vec![Expr::raw("0"), n]);
    let mapped = Expr::call(
        range,
        "mapToObj",
        vec![Expr::Lambda {
            params: vec![i],
            body: LambdaBody::Expr(Box::new(x)),
        }],
    );
    Expr::call(
        mapped,
        "collect",
        vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
    )
}

// `&v[a..b]`, `&s[a..]` -> subList / substring (Rust slices by byte, we by Java char).
pub fn lower_slice_range(
    cx: &mut Cx,
    base: Expr,
    base_ty: &Ty,
    r: &ast::RangeExpr,
    out: &mut Vec<Stmt>,
) -> Expr {
    use ast::RangeItem;
    let line = cx.line(r.syntax());
    let start = r.start().map(|e| expr::lower_expr(cx, &e, out));
    let end = r.end().map(|e| expr::lower_expr(cx, &e, out));
    let inclusive = r.op_kind() == Some(ast::RangeOp::Inclusive);
    let end = end.map(|e| {
        if inclusive {
            Expr::bin(e, "+", Expr::raw("1"))
        } else {
            e
        }
    });
    let base = simple(cx, base, base_ty, line, out);
    let from = start.unwrap_or_else(|| Expr::raw("0"));
    match base_ty.strip_refs() {
        Ty::Str => match end {
            Some(to) => Expr::call(base, "substring", vec![from, to]),
            None => Expr::call(base, "substring", vec![from]),
        },
        Ty::Vec(_) => {
            let to = match end {
                Some(e) => e,
                None => Expr::call(base.clone(), "size", vec![]),
            };
            Expr::call(base, "subList", vec![from, to])
        }
        _ => cx.unsupported(r.syntax(), "slice by range on this type"),
    }
}
