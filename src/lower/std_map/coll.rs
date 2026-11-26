// Std Vec/slices/arrays, HashMap/BTreeMap, sets, VecDeque, BinaryHeap, entry API.

use super::*;
use crate::lower::strip_parens;
use ra_ap_syntax::ast::HasName;

// Hook for std free functions (`key` as built by `calls::std_fn_key`, e.g. `Rc::new`).
pub(super) fn fn_call(
    cx: &mut Cx,
    c: &ast::CallExpr,
    key: &str,
    ret: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    if matches!(key, "HashMap::from" | "BTreeMap::from") {
        let src = expr::call_args(cx, c, out).into_iter().next()?;
        return Some(ordered_from(cx, src, ret));
    }
    if matches!(
        key,
        "HashSet::from" | "BTreeSet::from" | "VecDeque::from" | "BinaryHeap::from_vec"
    ) {
        let src = expr::call_args(cx, c, out).into_iter().next()?;
        if ordered_new(cx, ret).is_some() {
            return Some(ordered_from(cx, src, ret));
        }
        return Some(clone_expr(src, ret));
    }
    if matches!(
        key,
        "BinaryHeap::new" | "BinaryHeap::with_capacity" | "BinaryHeap::from"
    ) && let Ty::Heap(elem) = ret.strip_refs()
    {
        let reversed = cx
            .sema
            .type_of_expr(&ast::Expr::CallExpr(c.clone()))
            .and_then(|info| info.original.type_arguments().next())
            .and_then(|ty| ty.as_adt())
            .is_some_and(|adt| {
                adt.name(cx.db).as_str() == "Reverse"
                    && !adt.module(cx.db).krate(cx.db).origin(cx.db).is_local()
            })
            || heap_is_reverse(c);
        let natural = ord_comparator(cx, elem);
        let cmp = if reversed {
            natural
        } else {
            Some(reversed_comparator(natural))
        };
        let heap = Expr::New {
            ty: "PriorityQueue<>".into(),
            args: cmp.into_iter().map(Expr::raw).collect(),
        };
        if key == "BinaryHeap::with_capacity" {
            if let Some(capacity) = expr::call_args(cx, c, out).into_iter().next() {
                super::super::stmt::push_discard(out, cx.line(c.syntax()), capacity);
            }
            return Some(heap);
        }
        if key == "BinaryHeap::new" {
            return Some(heap);
        }
        if cx.lambda_depth > 0 {
            return Some(cx.unsupported(c.syntax(), "tuple heap construction inside a closure"));
        }
        let src = expr::call_args(cx, c, out).into_iter().next()?;
        let name = push_local(cx, cx.line(c.syntax()), ret.java(), Some(heap), out, "heap");
        out.push(Stmt::new(
            cx.line(c.syntax()),
            StmtKind::Expr(Expr::call(Expr::name(name.clone()), "addAll", vec![src])),
        ));
        return Some(Expr::name(name));
    }
    None
}

// Java comparator for Rust's `Ord` on `ty`; `None` when Java's natural order already agrees.
pub(crate) fn ord_comparator(cx: &mut Cx, ty: &Ty) -> Option<String> {
    let boxed = ty.boxed();
    match ty.strip_refs() {
        Ty::Int(_) if crate::mode::simple() => None,
        Ty::Int(Int::U16) => {
            let x = cx.fresh("x");
            Some(format!(
                "Comparator.comparingInt(({boxed} {x}) -> {x} & 0xFFFF)"
            ))
        }
        Ty::Int(Int::U32) => Some("Integer::compareUnsigned".into()),
        Ty::Int(Int::U64 | Int::U128) => Some("Long::compareUnsigned".into()),
        Ty::Tuple(fields) if fields.len() >= 2 => {
            let cmps = fields.iter().map(|f| ord_comparator(cx, f)).collect();
            Some(tuple_comparator(cx, ty, cmps))
        }
        Ty::Option(inner) => {
            let o = cx.fresh("o");
            let c = ord_comparator(cx, inner).unwrap_or_else(|| "Comparator.naturalOrder()".into());
            // Rust orders None before Some.
            Some(format!(
                "Comparator.comparing(({boxed} {o}) -> {o}.orElse(null), Comparator.nullsFirst({c}))"
            ))
        }
        Ty::Vec(elem) | Ty::Deque(elem) => {
            let (a, b) = (cx.fresh("a"), cx.fresh("b"));
            let elem_boxed = elem.boxed();
            let c = ord_comparator(cx, elem);
            // Lexicographic like Rust; generic element types can't form a Java array.
            if !elem_boxed.contains('<') {
                let arr = |l: &str| format!("{l}.toArray({elem_boxed}[]::new)");
                let extra = c.map(|c| format!(", {c}")).unwrap_or_default();
                return Some(format!(
                    "({boxed} {a}, {boxed} {b}) -> Arrays.compare({}, {}{extra})",
                    arr(&a),
                    arr(&b)
                ));
            }
            let (i, d) = (cx.fresh("i"), cx.fresh("d"));
            let cmp = call_comparator(c, elem, &format!("{a}.get({i})"), &format!("{b}.get({i})"));
            Some(format!(
                "({boxed} {a}, {boxed} {b}) -> IntStream.range(0, Math.min({a}.size(), {b}.size())).map({i} -> {cmp}).filter({d} -> {d} != 0).findFirst().orElse(Integer.compare({a}.size(), {b}.size()))"
            ))
        }
        _ => None,
    }
}

// Java int expression comparing `a` with `b` the way Rust's `Ord` on `ty` does.
pub(crate) fn compare_call(cx: &mut Cx, ty: &Ty, a: &str, b: &str) -> String {
    // Lists of array-able elements compare directly instead of through a cast lambda.
    if let Ty::Vec(elem) | Ty::Deque(elem) = ty.strip_refs()
        && !elem.boxed().contains('<')
    {
        let e = elem.boxed();
        let extra = ord_comparator(cx, elem)
            .map(|c| format!(", {c}"))
            .unwrap_or_default();
        return format!("Arrays.compare({a}.toArray({e}[]::new), {b}.toArray({e}[]::new){extra})");
    }
    let cmp = ord_comparator(cx, ty);
    call_comparator(cmp, ty, a, b)
}

fn call_comparator(cmp: Option<String>, ty: &Ty, a: &str, b: &str) -> String {
    match cmp {
        None => format!("{a}.compareTo({b})"),
        // A lambda has no `compare` method until it has a type.
        Some(c) if c.starts_with('(') => {
            format!("((Comparator<{}>) {c}).compare({a}, {b})", ty.boxed())
        }
        Some(c) if !c.starts_with("Comparator.") => format!("{}({a}, {b})", c.replace("::", ".")),
        Some(c) => format!("{c}.compare({a}, {b})"),
    }
}

// `a.cmp(&b)` as a Java int, for any type (`compareTo` only where Java's order agrees).
pub(crate) fn ord_compare(cx: &mut Cx, ty: &Ty, a: Expr, b: Expr) -> Expr {
    // `a` may become a receiver (`a.compareTo(b)`), so a cast or operator needs parentheses.
    let receiver = |e: &Expr| match e {
        Expr::Cast(..) | Expr::Binary(..) | Expr::Unary(..) | Expr::Ternary(..) => {
            format!("({})", render(e))
        }
        _ => render(e),
    };
    let (a, b) = (receiver(&a), render(&b));
    Expr::raw(compare_call(cx, ty, &a, &b))
}

// `a.max(b)` / `a.min(b)`: Rust's max keeps the second of equal values, min the first.
pub(crate) fn ord_pick(
    cx: &mut Cx,
    max: bool,
    ty: &Ty,
    a: Expr,
    b: Expr,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let (a, b) = (simple(cx, a, ty, line, out), simple(cx, b, ty, line, out));
    let cmp = ord_compare(cx, ty, a.clone(), b.clone());
    let cond = Expr::bin(cmp, if max { ">" } else { "<=" }, Expr::raw("0"));
    Expr::Ternary(Box::new(cond), Box::new(a), Box::new(b))
}

// `cmp` reversed, for max-heaps.
// Field by field, each with its own comparator (`None`: Java's natural order).
fn tuple_comparator(cx: &mut Cx, ty: &Ty, cmps: Vec<Option<String>>) -> String {
    let boxed = ty.boxed();
    let p = cx.fresh("p");
    let n_fields = cmps.len();
    let mut chain = String::new();
    for (n, cmp) in cmps.into_iter().enumerate() {
        let get = match ty.strip_refs() {
            Ty::Tuple(ts) => format!("{}()", crate::tuples::accessor(ts, n)),
            _ if n_fields == 2 => ["getKey()", "getValue()"][n].to_string(),
            _ => format!("_{n}()"),
        };
        chain += &match n {
            0 => format!("Comparator.comparing(({boxed} {p}) -> {p}.{get}"),
            _ => format!(".thenComparing({p} -> {p}.{get}"),
        };
        if let Some(c) = cmp {
            chain += &format!(", {c}");
        }
        chain += ")";
    }
    chain
}

fn is_reverse_call(e: &ast::Expr) -> bool {
    matches!(strip_parens(e), ast::Expr::CallExpr(c)
        if matches!(c.expr(), Some(ast::Expr::PathExpr(p))
            if p.path().and_then(|p| p.segment()).is_some_and(|s| s.syntax().text() == "Reverse")))
}

// Comparator for the key a closure body builds. Types erase `Reverse(..)`, so its order comes
// from the expression's shape: `Reverse(k)`, `(Reverse(a), b)`.
pub(crate) fn key_comparator(cx: &mut Cx, body: &ast::Expr, ty: &Ty) -> Option<String> {
    match strip_parens(body) {
        ast::Expr::CallExpr(c) if is_reverse_call(body) => {
            let inner = c.arg_list()?.args().next()?;
            Some(reversed_comparator(key_comparator(cx, &inner, ty)))
        }
        ast::Expr::BlockExpr(b) if b.stmt_list()?.statements().next().is_none() => {
            key_comparator(cx, &b.tail_expr()?, ty)
        }
        ast::Expr::TupleExpr(t) => match ty.strip_refs() {
            Ty::Tuple(fields) if t.fields().count() == fields.len() && fields.len() >= 2 => {
                let cmps = t
                    .fields()
                    .zip(fields.iter())
                    .map(|(e, f)| key_comparator(cx, &e, f))
                    .collect();
                Some(tuple_comparator(cx, ty, cmps))
            }
            _ => ord_comparator(cx, ty),
        },
        _ => ord_comparator(cx, ty),
    }
}

pub(crate) fn reversed_comparator(cmp: Option<String>) -> String {
    match cmp {
        None => "Comparator.reverseOrder()".into(),
        Some(c) if c.starts_with("Comparator.") => format!("{c}.reversed()"),
        Some(c) => format!("Collections.reverseOrder({c})"),
    }
}

pub(crate) fn clone_list(cx: &mut Cx, value: Expr, elem: &Ty) -> Expr {
    if let Ty::Vec(inner) = elem.strip_refs() {
        let item = cx.fresh("item");
        let copied = clone_list(cx, Expr::name(item.clone()), inner);
        return Expr::call(
            Expr::call(
                Expr::call(value, "stream", vec![]),
                "map",
                vec![Expr::Lambda {
                    params: vec![item],
                    body: LambdaBody::Expr(Box::new(copied)),
                }],
            ),
            "collect",
            vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
        );
    }
    Expr::New {
        ty: "ArrayList<>".into(),
        args: vec![value],
    }
}

// `v.clone()` / `v.to_vec()`: struct elements are mutable Java objects, so each is cloned too.
fn clone_elems(cx: &mut Cx, m: &ast::MethodCallExpr, recv: Expr, elem: &Ty) -> Expr {
    let struct_elems = m
        .receiver()
        .and_then(|r| cx.sema.type_of_expr(&r))
        .map(|t| t.original.strip_references())
        .and_then(|t| match t.as_array(cx.db) {
            Some((e, _)) => Some(e),
            None => t.as_slice().or_else(|| t.type_arguments().next()),
        })
        .and_then(|e| match e.as_adt() {
            Some(ra_ap_hir::Adt::Struct(st)) => Some(st),
            _ => None,
        })
        .is_some_and(|st| st.module(cx.db).krate(cx.db).origin(cx.db).is_local());
    if !struct_elems {
        return clone_list(cx, recv, elem);
    }
    let item = cx.fresh_param("item");
    cx.release_param(&item);
    let copied = Expr::call(
        Expr::call(recv, "stream", vec![]),
        "map",
        vec![Expr::Lambda {
            params: vec![item.clone()],
            body: LambdaBody::Expr(Box::new(Expr::call(Expr::name(item), "clone", vec![]))),
        }],
    );
    Expr::call(
        copied,
        "collect",
        vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
    )
}

// Hook for std methods, before the receiver is lowered (`key` like `RefCell::borrow`).
pub(super) fn method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let name = key.rsplit("::").next().unwrap_or(key);
    // `cmp`/`partial_cmp`/`max`/`min` on lists, tuples and options, whose Java types aren't Comparable.
    if matches!(name, "cmp" | "partial_cmp" | "max" | "min")
        && matches!(
            recv_ty.strip_refs(),
            Ty::Vec(_) | Ty::Deque(_) | Ty::Tuple(_) | Ty::Option(_)
        )
        && let Some(arg) = arg_asts(m).into_iter().next()
    {
        let ty = recv_ty.strip_refs().clone();
        let line = cx.line(m.syntax());
        let a = expr::lower_expr(cx, &m.receiver()?, out);
        let b = expr::lower_expr(cx, &arg, out);
        return Some(match name {
            "cmp" => ord_compare(cx, &ty, a, b),
            "partial_cmp" => Expr::call(
                Expr::raw("Optional"),
                "of",
                vec![ord_compare(cx, &ty, a, b)],
            ),
            _ => ord_pick(cx, name == "max", &ty, a, b, line, out),
        });
    }
    if name == "and_modify" || matches!(name, "or_insert" | "or_insert_with" | "or_default") {
        if let Some(e) = and_modify(cx, m, name, out) {
            return Some(e);
        }
    }
    if let Ty::Vec(elem) = recv_ty.strip_refs()
        && let Some(ast::Expr::MethodCallExpr(contiguous)) = m.receiver()
        && method_name(&contiguous) == "make_contiguous"
        && let Some(deque_ast) = contiguous.receiver()
        && matches!(cx.ty(&deque_ast).strip_refs(), Ty::Deque(_))
    {
        let name = method_name(m);
        if !matches!(name.as_str(), "sort" | "sort_unstable" | "reverse") {
            return Some(cx.unsupported(m.syntax(), "operation on a contiguous deque slice"));
        }
        if cx.lambda_depth > 0 {
            return Some(cx.unsupported(m.syntax(), "contiguous deque slice inside a closure"));
        }
        let line = cx.line(m.syntax());
        let deque_ty = cx.ty(&deque_ast);
        let deque = expr::lower_expr(cx, &deque_ast, out);
        let deque = push_local(cx, line, deque_ty.java(), Some(deque), out, "deque");
        let list = push_local(
            cx,
            line,
            recv_ty.java(),
            Some(Expr::New {
                ty: "ArrayList<>".into(),
                args: vec![Expr::name(deque.clone())],
            }),
            out,
            "contiguous",
        );
        let result = lower_vec_method(
            cx,
            m,
            &ast::Expr::MethodCallExpr(contiguous),
            Expr::name(list.clone()),
            elem,
            vec![],
            &[],
            &name,
            &Ty::Unit,
            line,
            out,
        )?;
        super::super::stmt::push_discard(out, line, result);
        out.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::call(Expr::name(deque.clone()), "clear", vec![])),
        ));
        out.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::call(
                Expr::name(deque),
                "addAll",
                vec![Expr::name(list)],
            )),
        ));
        return Some(Expr::raw("null"));
    }
    if let Ty::Vec(elem) = recv_ty.strip_refs()
        && matches!(key.rsplit("::").next()?, "retain_mut" | "dedup_by_key")
    {
        return Some(mutable_vec_predicate(cx, m, recv_ty, elem, key, out));
    }
    let Ty::Map { k, v, .. } = recv_ty.strip_refs() else {
        return None;
    };
    if key.rsplit("::").next()? == "extend" {
        if cx.lambda_depth > 0 {
            return Some(cx.unsupported(m.syntax(), "map extend inside a closure"));
        }
        let source_ast = m.arg_list()?.args().next()?;
        let source_ty = cx.ty(&source_ast);
        let recv = expr::lower_expr(cx, &m.receiver()?, out);
        let map = push_local(
            cx,
            cx.line(m.syntax()),
            recv_ty.java(),
            Some(recv),
            out,
            "map",
        );
        let source = expr::lower_expr(cx, &source_ast, out);
        let stream = match source_ty.strip_refs() {
            Ty::Iter(_) => source,
            Ty::Map { .. } => Expr::call(Expr::call(source, "entrySet", vec![]), "stream", vec![]),
            Ty::Vec(_) | Ty::Set { .. } | Ty::Deque(_) => Expr::call(source, "stream", vec![]),
            _ => return Some(cx.unsupported(m.syntax(), "map extend source")),
        };
        let entry = cx.fresh("entry");
        return Some(Expr::call(
            stream,
            "forEach",
            vec![Expr::Lambda {
                params: vec![entry.clone()],
                body: LambdaBody::Expr(Box::new(Expr::call(
                    Expr::name(map),
                    "put",
                    vec![
                        Expr::call(Expr::name(entry.clone()), "getKey", vec![]),
                        Expr::call(Expr::name(entry), "getValue", vec![]),
                    ],
                ))),
            }],
        ));
    }
    if key.rsplit("::").next()? != "retain" {
        return None;
    }
    let ast::Expr::ClosureExpr(closure) = m.arg_list()?.args().next()? else {
        return Some(cx.unsupported(m.syntax(), "map retain predicate without a closure"));
    };
    let local = match closure.param_list()?.params().nth(1)?.pat()? {
        ast::Pat::IdentPat(binding) => cx.sema.to_def(&binding),
        ast::Pat::WildcardPat(_) => None,
        _ => return Some(cx.unsupported(closure.syntax(), "map retain value pattern")),
    };
    let recv = expr::lower_expr(cx, &m.receiver()?, out);
    let entry = cx.fresh("entry");
    let saved = local.and_then(|local| {
        cx.elem_aliases.insert(
            local,
            (
                entry.clone(),
                String::new(),
                super::super::ElemAliasKind::Entry,
            ),
        )
    });
    let predicate = expr::lower_closure(cx, &closure, out);
    if let Some(local) = local {
        if let Some(previous) = saved {
            cx.elem_aliases.insert(local, previous);
        } else {
            cx.elem_aliases.remove(&local);
        }
    }
    let predicate = Expr::Cast(
        format!("BiPredicate<{}, {}>", k.boxed(), v.boxed()),
        Box::new(predicate),
    );
    let test = Expr::call(
        predicate,
        "test",
        vec![
            Expr::call(Expr::name(entry.clone()), "getKey", vec![]),
            Expr::call(Expr::name(entry.clone()), "getValue", vec![]),
        ],
    );
    Some(Expr::call(
        Expr::call(recv, "entrySet", vec![]),
        "removeIf",
        vec![Expr::Lambda {
            params: vec![entry],
            body: LambdaBody::Expr(Box::new(Expr::Unary("!", Box::new(test)))),
        }],
    ))
}

fn mutable_vec_predicate(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    elem: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Expr {
    if cx.lambda_depth > 0 {
        return cx.unsupported(m.syntax(), "mutable vector predicate inside a closure");
    }
    let Some(ast::Expr::ClosureExpr(closure)) = m.arg_list().and_then(|a| a.args().next()) else {
        return cx.unsupported(m.syntax(), "mutable vector predicate without a closure");
    };
    let Some(ast::Pat::IdentPat(binding)) = closure
        .param_list()
        .and_then(|p| p.params().next())
        .and_then(|p| p.pat())
    else {
        return cx.unsupported(closure.syntax(), "mutable vector predicate pattern");
    };
    let Some(local) = cx.sema.to_def(&binding) else {
        return cx.unsupported(closure.syntax(), "mutable vector predicate binding");
    };
    let Some(receiver) = m.receiver() else {
        return cx.unsupported(m.syntax(), "mutable vector predicate receiver");
    };
    let line = cx.line(m.syntax());
    let source = expr::lower_expr(cx, &receiver, out);
    let list = push_local(cx, line, recv_ty.java(), Some(source), out, "list");
    let index = cx.fresh("index");
    let current = cx.fresh("current");
    let previous = cx.fresh("previous");
    let is_dedup = key.ends_with("dedup_by_key");
    let result_ty = closure
        .body()
        .map(|body| cx.ty(&body))
        .unwrap_or(Ty::Other("?".into()));
    if is_dedup && !result_ty.is_nameable() {
        return cx.unsupported(closure.syntax(), "dedup key type");
    }
    let saved = cx.elem_aliases.insert(
        local,
        (
            list.clone(),
            current.clone(),
            super::super::ElemAliasKind::List,
        ),
    );
    let current_fn = expr::lower_closure(cx, &closure, out);
    let previous_fn = if is_dedup {
        cx.elem_aliases.insert(
            local,
            (
                list.clone(),
                previous.clone(),
                super::super::ElemAliasKind::List,
            ),
        );
        Some(expr::lower_closure(cx, &closure, out))
    } else {
        None
    };
    if let Some(old) = saved {
        cx.elem_aliases.insert(local, old);
    } else {
        cx.elem_aliases.remove(&local);
    }
    let mut body = vec![Stmt::new(
        line,
        StmtKind::Local {
            ty: "int".into(),
            name: current.clone(),
            init: Some(Expr::name(index.clone())),
        },
    )];
    let condition = if let Some(previous_fn) = previous_fn {
        body.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "int".into(),
                name: previous.clone(),
                init: Some(Expr::bin(Expr::name(index.clone()), "-", Expr::raw("1"))),
            },
        ));
        let ty = format!("Function<{}, {}>", elem.boxed(), result_ty.boxed());
        let call = |func: Expr, slot: &str| {
            Expr::call(
                Expr::Cast(ty.clone(), Box::new(func)),
                "apply",
                vec![Expr::call(
                    Expr::name(list.clone()),
                    "get",
                    vec![Expr::name(slot)],
                )],
            )
        };
        Expr::call(
            Expr::raw("Objects"),
            "equals",
            vec![call(current_fn, &current), call(previous_fn, &previous)],
        )
    } else {
        let ty = format!("Predicate<{}>", elem.boxed());
        Expr::Unary(
            "!",
            Box::new(Expr::call(
                Expr::Cast(ty, Box::new(current_fn)),
                "test",
                vec![Expr::call(
                    Expr::name(list.clone()),
                    "get",
                    vec![Expr::name(current)],
                )],
            )),
        )
    };
    let start = if is_dedup { 1 } else { 0 };
    let close_line = cx.end_line(m.syntax());
    body.push(Stmt::new(
        line,
        StmtKind::If {
            cond: condition,
            then: crate::jir::Block {
                stmts: vec![Stmt::new(
                    line,
                    StmtKind::Expr(Expr::call(
                        Expr::name(list.clone()),
                        "remove",
                        vec![Expr::name(index.clone())],
                    )),
                )],
                close_line,
            },
            els: Some(Box::new(Stmt::new(
                line,
                StmtKind::Block(crate::jir::Block {
                    stmts: vec![Stmt::new(line, StmtKind::Raw(format!("{index}++;")))],
                    close_line,
                }),
            ))),
        },
    ));
    out.push(Stmt::new(
        line,
        StmtKind::For {
            label: None,
            header: format!("int {index} = {start}; {index} < {list}.size();"),
            body: crate::jir::Block {
                stmts: body,
                close_line,
            },
        },
    ));
    Expr::raw("null")
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
        _ => default_value_cx(cx, &v_ty),
    };
    let ignored = cx.fresh("ignored");
    let f = Expr::Lambda {
        params: vec![ignored],
        body: LambdaBody::Expr(Box::new(value)),
    };
    Some(Expr::call(map, "computeIfAbsent", vec![key, f]))
}

// `m.entry(k).and_modify(f)` -> `computeIfPresent`; with `.or_insert*(v)` after it -> `compute`.
// `f` must be `|x| *x op= e` / `|x| *x = e` (a new value) or mutate an object value in place.
fn and_modify(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let modify = if name == "and_modify" {
        m.clone()
    } else {
        match m.receiver()? {
            ast::Expr::MethodCallExpr(r) if method_name(&r) == "and_modify" => r,
            _ => return None,
        }
    };
    let ast::Expr::MethodCallExpr(entry) = modify.receiver()? else {
        return None;
    };
    if method_name(&entry) != "entry" {
        return None;
    }
    let ast::Expr::ClosureExpr(f) = modify.arg_list()?.args().next()? else {
        return Some(cx.unsupported(modify.syntax(), "`and_modify` without a closure literal"));
    };
    let map_ast = entry.receiver()?;
    let v_ty = match cx.ty(&map_ast).strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        _ => return None,
    };
    let map = expr::lower_expr(cx, &map_ast, out);
    let key_ast = entry.arg_list()?.args().next()?;
    let key = expr::lower_expr(cx, &key_ast, out);
    if !matches!(
        v_ty.strip_refs(),
        Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Bool | Ty::Char | Ty::Str
    ) && m
        .syntax()
        .parent()
        .is_some_and(|p| ast::ExprStmt::can_cast(p.kind()))
    {
        return in_place_modify(cx, m, name, &f, map, key, &cx.ty(&key_ast), &v_ty, out);
    }
    let (k, x) = (cx.fresh("key"), cx.fresh("value"));
    let Some(updated) = modified_value(cx, &f, &x, &v_ty) else {
        return Some(cx.unsupported(f.syntax(), "`and_modify` closure shape"));
    };
    if name == "and_modify" {
        let lambda = Expr::Lambda {
            params: vec![k, x],
            body: LambdaBody::Expr(Box::new(updated)),
        };
        return Some(Expr::call(map, "computeIfPresent", vec![key, lambda]));
    }
    let fresh = match name {
        "or_insert" => expr::lower_expr(cx, &m.arg_list()?.args().next()?, out),
        "or_insert_with" => ctor_value(cx, &m.arg_list()?.args().next()?, &v_ty, out)?,
        _ => default_value_cx(cx, &v_ty),
    };
    let body = conditional(
        Expr::bin(Expr::name(&x), "==", Expr::raw("null")),
        fresh,
        updated,
    );
    let lambda = Expr::Lambda {
        params: vec![k, x],
        body: LambdaBody::Expr(Box::new(body)),
    };
    Some(Expr::call(map, "compute", vec![key, lambda]))
}

// An object value is mutated where it sits: `V v = m.get(k); if (v != null) { f } else m.put(k, fresh);`.
#[allow(clippy::too_many_arguments)]
fn in_place_modify(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    name: &str,
    f: &ast::ClosureExpr,
    map: Expr,
    key: Expr,
    key_ty: &Ty,
    v_ty: &Ty,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let line = cx.line(m.syntax());
    let ast::Pat::IdentPat(p) = f.param_list()?.params().next()?.pat()? else {
        return None;
    };
    let rust = ast::HasName::name(&p)?.text().to_string();
    let fresh = match name {
        "and_modify" => None,
        "or_insert" => Some(expr::lower_expr(cx, &m.arg_list()?.args().next()?, out)),
        "or_insert_with" => Some(ctor_value(cx, &m.arg_list()?.args().next()?, v_ty, out)?),
        _ => Some(default_value_cx(cx, v_ty)),
    };
    let map = simple(cx, map, &Ty::Unit, line, out);
    let key = if fresh.is_some() {
        simple(cx, key, key_ty, line, out)
    } else {
        key
    };
    cx.push_scope();
    let x = cx.declare(&rust);
    let mut body = Vec::new();
    crate::lower::stmt::lower_to_dest(
        cx,
        &f.body()?,
        &crate::lower::stmt::Dest::Discard,
        &mut body,
    );
    cx.pop_scope();
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: crate::lower::stmt::decl_type(v_ty),
            name: x.clone(),
            init: Some(Expr::call(map.clone(), "get", vec![key.clone()])),
        },
    ));
    let els = fresh.map(|v| {
        Box::new(Stmt::new(
            line,
            StmtKind::Expr(Expr::call(map, "put", vec![key, v])),
        ))
    });
    out.push(Stmt::new(
        line,
        StmtKind::If {
            cond: Expr::bin(Expr::name(&x), "!=", Expr::raw("null")),
            then: crate::jir::Block {
                stmts: body,
                close_line: line,
            },
            els,
        },
    ));
    Some(Expr::raw("null"))
}

// The value after `f` ran on the current value `x` (a Java name).
fn modified_value(cx: &mut Cx, f: &ast::ClosureExpr, x: &str, v_ty: &Ty) -> Option<Expr> {
    let ast::Pat::IdentPat(p) = f.param_list()?.params().next()?.pat()? else {
        return None;
    };
    let p_name = ast::HasName::name(&p)?.text().to_string();
    let mut body = f.body()?;
    // `{ *x += 1 }` / `{ *x += 1; }`
    if let ast::Expr::BlockExpr(b) = &body {
        let list = b.stmt_list()?;
        let stmts: Vec<ast::Stmt> = list.statements().collect();
        body = match (stmts.as_slice(), list.tail_expr()) {
            ([], Some(t)) => t,
            ([ast::Stmt::ExprStmt(s)], None) => s.expr()?,
            _ => return None,
        };
    }
    cx.push_scope();
    cx.declare_as(&p_name, x);
    let value = (|| {
        let ast::Expr::BinExpr(b) = &body else {
            return None;
        };
        let Some(ast::BinaryOp::Assignment { op }) = b.op_kind() else {
            return None;
        };
        let ast::Expr::PrefixExpr(d) = b.lhs()? else {
            return None;
        };
        if d.op_kind() != Some(ast::UnaryOp::Deref) {
            return None;
        }
        let mut no_out = Vec::new();
        let rhs = expr::lower_expr_as(cx, &b.rhs()?, v_ty, &mut no_out);
        if !no_out.is_empty() {
            return None;
        }
        Some(match op {
            None => rhs,
            Some(op) => {
                let sym = expr::arith_symbol(op, v_ty);
                expr::narrow_if_needed(Expr::bin(Expr::name(x), sym, rhs), v_ty)
            }
        })
    })();
    cx.pop_scope();
    value
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
        "default" => default_value_cx(cx, ty),
        _ => return None,
    })
}

// `*m.get_mut(&k).unwrap() op= r` / `*v.last_mut().unwrap() op= r` on a number or String: Java
// can't write through the element, so the collection gets the new value (`put`/`set`).
pub fn lower_slot_assign(
    cx: &mut Cx,
    inner: &ast::Expr,
    sym: &'static str,
    r: Expr,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let ast::Expr::MethodCallExpr(outer) = strip_parens(inner) else {
        return None;
    };
    // `*cell.get_mut() += 1` on a one-element-array cell writes its element.
    if method_name(&outer) == "get_mut"
        && let Some(recv) = outer.receiver()
        && super::ptr::is_array(cx, &recv).is_some()
    {
        let v = expr::lower_expr(cx, &recv, out);
        let place = super::ptr::contents(cx, &recv, v);
        return Some(Expr::Assign(Box::new(place), sym, Box::new(r)));
    }
    if !matches!(method_name(&outer).as_str(), "unwrap" | "expect") {
        return None;
    }
    let ast::Expr::MethodCallExpr(call) = strip_parens(&outer.receiver()?) else {
        return None;
    };
    let base_ast = call.receiver()?;
    let base_ty = cx.ty(&base_ast);
    let elem_ty = match base_ty.strip_refs() {
        Ty::Map { v, .. } => (**v).clone(),
        Ty::Vec(t) => (**t).clone(),
        _ => return None,
    };
    if !crate::lower::is_mut_ref_cell_ty(&elem_ty) {
        return None;
    }
    let is_map = matches!(base_ty.strip_refs(), Ty::Map { .. });
    let line = cx.line(inner.syntax());
    let pure_or_local = |cx: &mut Cx, e: Expr, ty: String, out: &mut Vec<Stmt>, base: &str| {
        if crate::lower::fmt::is_pure(&e) {
            e
        } else {
            Expr::name(push_local(cx, line, ty, Some(e), out, base))
        }
    };
    let base = expr::lower_expr(cx, &base_ast, out);
    let base = pure_or_local(cx, base, base_ty.strip_refs().java(), out, "slots");
    let (index, cur) = match (method_name(&call).as_str(), is_map) {
        ("get_mut", _) => {
            let arg = call.arg_list()?.args().next()?;
            let key_ty = cx.ty(&arg);
            let key = expr::lower_expr(cx, &arg, out);
            let key = pure_or_local(cx, key, key_ty.strip_refs().java(), out, "key");
            let cur = Expr::call(base.clone(), "get", vec![key.clone()]);
            (key, cur)
        }
        ("last_mut", false) => (
            Expr::bin(
                Expr::call(base.clone(), "size", vec![]),
                "-",
                Expr::raw("1"),
            ),
            Expr::call(base.clone(), "getLast", vec![]),
        ),
        ("first_mut", false) => (Expr::raw("0"), Expr::call(base.clone(), "getFirst", vec![])),
        _ => return None,
    };
    // A plain `=` on a missing map key inserts where Rust would panic; accepted.
    let value = if sym == "=" {
        r
    } else {
        expr::narrow_if_needed(Expr::bin(cur, sym.trim_end_matches('='), r), &elem_ty)
    };
    let setter = if is_map { "put" } else { "set" };
    Some(Expr::call(base, setter, vec![index, value]))
}

// `*m.entry(k).or_insert(v) op= r` updates the entry after creating it.
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
    let (k_ty, v_ty) = match map_ty.strip_refs() {
        Ty::Map { k, v, .. } => ((**k).clone(), (**v).clone()),
        _ => return None,
    };
    let symbol = match op {
        None => None,
        Some(ast::ArithOp::Add) => Some("+"),
        Some(ast::ArithOp::Sub) => Some("-"),
        Some(ast::ArithOp::Mul) => Some("*"),
        Some(ast::ArithOp::Div) => Some("/"),
        Some(ast::ArithOp::Rem) => Some("%"),
        Some(ast::ArithOp::BitOr) => Some("|"),
        Some(ast::ArithOp::BitAnd) => Some("&"),
        Some(ast::ArithOp::BitXor) => Some("^"),
        _ => return None,
    };
    let line = cx.line(lhs.syntax());
    // `*m.entry(k).or_insert(0) += x` (the counting idiom) is `m.merge(k, x, Integer::sum)`.
    // Rust reads `x` before `k`; with either side pure the order can't show.
    let zero_default = match method_name(mc).as_str() {
        "or_default" => true,
        "or_insert" => mc
            .arg_list()
            .and_then(|a| a.args().next())
            .is_some_and(|a| {
                matches!(&a, ast::Expr::Literal(l) if {
                    let t = l.syntax().text().to_string();
                    let digits = t.trim_end_matches(|c: char| c.is_ascii_alphabetic());
                    digits.trim_end_matches(['0', '.', '_']).is_empty() && !digits.is_empty()
                })
            }),
        _ => false,
    };
    let sum = match v_ty.strip_refs() {
        Ty::Int(i) if !matches!(i.java(), "byte" | "short") => Some(if i.is_long() {
            "Long::sum"
        } else {
            "Integer::sum"
        }),
        Ty::F64 => Some("Double::sum"),
        _ => None,
    };
    if zero_default
        && let Some(sum) = sum
        && matches!(symbol, Some("+" | "-"))
        && (crate::lower::pat::is_pure(rhs) || crate::lower::pat::is_pure(&key_ast))
    {
        let map = expr::lower_expr(cx, &map_ast, out);
        let key = expr::lower_expr(cx, &key_ast, out);
        let mut value = expr::lower_expr_as(cx, rhs, &v_ty, out);
        if symbol == Some("-") {
            value = Expr::Unary("-", Box::new(value));
        }
        return Some(Expr::call(map, "merge", vec![key, value, Expr::raw(sum)]));
    }
    let rhs = expr::lower_expr_as(cx, rhs, &v_ty, out);
    let rhs = push_local(cx, line, v_ty.java(), Some(rhs), out, "value");
    let map = expr::lower_expr(cx, &map_ast, out);
    let map = push_local(cx, line, map_ty.java(), Some(map), out, "map");
    let key = expr::lower_expr(cx, &key_ast, out);
    let key = push_local(cx, line, k_ty.java(), Some(key), out, "key");
    let default = match method_name(mc).as_str() {
        "or_insert" => {
            let arg = mc.arg_list()?.args().next()?;
            let value = expr::lower_expr_as(cx, &arg, &v_ty, out);
            let value = push_local(cx, line, v_ty.java(), Some(value), out, "default");
            Expr::name(value)
        }
        "or_insert_with" => {
            let arg = mc.arg_list()?.args().next()?;
            if let ast::Expr::ClosureExpr(closure) = arg {
                Expr::call(
                    Expr::Cast(
                        format!("Supplier<{}>", v_ty.boxed()),
                        Box::new(expr::lower_closure(cx, &closure, out)),
                    ),
                    "get",
                    vec![],
                )
            } else {
                ctor_value(cx, &arg, &v_ty, out)?
            }
        }
        _ => default_value_cx(cx, &v_ty),
    };
    let ignored = cx.fresh("ignored");
    out.push(Stmt::new(
        line,
        StmtKind::Expr(Expr::call(
            Expr::name(map.clone()),
            "computeIfAbsent",
            vec![
                Expr::name(key.clone()),
                Expr::Lambda {
                    params: vec![ignored],
                    body: LambdaBody::Expr(Box::new(default)),
                },
            ],
        )),
    ));
    let value = if let Some(symbol) = symbol {
        let old = Expr::call(
            Expr::name(map.clone()),
            "get",
            vec![Expr::name(key.clone())],
        );
        let value = Expr::bin(old, symbol, Expr::name(rhs));
        if let Ty::Int(i) = v_ty.strip_refs() {
            narrow(value, *i)
        } else {
            value
        }
    } else {
        Expr::name(rhs)
    };
    Some(Expr::call(
        Expr::name(map),
        "put",
        vec![Expr::name(key), value],
    ))
}

// ---- Vec / slices ----------------------------------------------------------------------------

// Slice views are `subList`s, so writes through them reach the list like Rust's `&mut [T]`.
pub(crate) const SLICE_VIEW_HELPERS: &str = "static <T> List<List<T>> slice_split(List<T> v, Predicate<T> p, int n, boolean rev, boolean inclusive) { \
List<List<T>> out = new ArrayList<>(); if (n == 0) return out; \
if (inclusive) { int start = 0; for (int i = 0; i < v.size(); i++) if (p.test(v.get(i))) { out.add(v.subList(start, i + 1)); start = i + 1; } \
if (start < v.size()) out.add(v.subList(start, v.size())); return out; } \
if (!rev) { int start = 0; for (int i = 0; i < v.size() && (n < 0 || out.size() < n - 1); i++) if (p.test(v.get(i))) { out.add(v.subList(start, i)); start = i + 1; } \
out.add(v.subList(start, v.size())); return out; } \
int end = v.size(); for (int i = v.size() - 1; i >= 0 && (n < 0 || out.size() < n - 1); i--) if (p.test(v.get(i))) { out.add(v.subList(i + 1, end)); end = i; } \
out.add(v.subList(0, end)); return out; } \
static <T> List<List<T>> rchunks(List<T> v, int k, boolean exact) { \
List<List<T>> out = new ArrayList<>(); int end = v.size(); for (; end >= k; end -= k) out.add(v.subList(end - k, end)); \
if (!exact && end > 0) out.add(v.subList(0, end)); return out; } \
static <T> List<List<T>> chunks_of(List<T> v, int from, int k) { \
List<List<T>> out = new ArrayList<>(); for (int i = from; i + k <= v.size(); i += k) out.add(v.subList(i, i + k)); return out; } \
static <T> List<List<T>> chunk_by(List<T> v, BiPredicate<T, T> same) { \
List<List<T>> out = new ArrayList<>(); int start = 0; \
for (int i = 1; i <= v.size(); i++) if (i == v.size() || !same.test(v.get(i - 1), v.get(i))) { if (i > start) out.add(v.subList(start, i)); start = i; } return out; } \
static <T> List<T> flat_view(List<List<T>> v) { int k = v.isEmpty() ? 0 : v.get(0).size(); return new AbstractList<T>() { \
public T get(int i) { return v.get(i / k).get(i % k); } public T set(int i, T x) { return v.get(i / k).set(i % k, x); } \
public int size() { return v.size() * k; } }; }";

pub(crate) const SLICE_EDIT_HELPERS: &str = "static <T> void fill_with(List<T> v, Supplier<T> f) { for (int i = 0; i < v.size(); i++) v.set(i, f.get()); } \
static <T> void copy_within(List<T> v, int from, int to, int dest) { List<T> part = new ArrayList<>(v.subList(from, to)); for (int k = 0; k < part.size(); k++) v.set(dest + k, part.get(k)); } \
static <T> void swap_with_slice(List<T> a, List<T> b) { for (int i = 0; i < a.size(); i++) { T t = a.get(i); a.set(i, b.get(i)); b.set(i, t); } } \
@SuppressWarnings(\"unchecked\") static <T> List<T> slice_join(List<List<T>> v, Object sep) { \
List<T> out = new ArrayList<>(); for (int i = 0; i < v.size(); i++) { \
if (i > 0) { if (sep instanceof List<?> s) out.addAll((List<T>) s); else out.add((T) sep); } out.addAll(v.get(i)); } return out; } \
static <T> Optional<List<T>> slice_strip(List<T> v, List<T> pre, List<T> suf) { \
int p = pre.size(), s = suf.size(); \
if (v.size() < p + s || !v.subList(0, p).equals(pre) || !v.subList(v.size() - s, v.size()).equals(suf)) return Optional.empty(); \
return Optional.of(v.subList(p, v.size() - s)); }";

pub(crate) const BYTES_HELPERS: &str = "static List<Integer> trim_ascii_bytes(List<Integer> b, boolean start, boolean end) { \
int a = 0, z = b.size(); while (start && a < z && \" \\t\\n\\f\\r\".indexOf(b.get(a)) >= 0) a++; \
while (end && z > a && \" \\t\\n\\f\\r\".indexOf(b.get(z - 1)) >= 0) z--; return b.subList(a, z); } \
static String esc_ascii(List<Integer> b) { StringBuilder s = new StringBuilder(); \
for (int x : b) { int c = x & 0xFF; switch (c) { \
case '\\t' -> s.append(\"\\\\t\"); case '\\r' -> s.append(\"\\\\r\"); case '\\n' -> s.append(\"\\\\n\"); \
case '\\\\' -> s.append(\"\\\\\\\\\"); case '\\'' -> s.append(\"\\\\'\"); case '\"' -> s.append(\"\\\\\\\"\"); \
default -> { if (c >= 0x20 && c < 0x7f) s.append((char) c); else s.append(String.format(\"\\\\x%02x\", c)); } } } return s.toString(); } \
static List<Map.Entry<String, List<Integer>>> utf8_chunks(List<Integer> b) { \
List<Map.Entry<String, List<Integer>>> out = new ArrayList<>(); byte[] a = new byte[b.size()]; for (int k = 0; k < a.length; k++) a[k] = (byte) (int) b.get(k); \
int i = 0, n = a.length; while (i < n) { int start = i, bad = 0; \
while (i < n) { int c = a[i] & 0xFF; int len = c < 0x80 ? 1 : c >= 0xC2 && c <= 0xDF ? 2 : c >= 0xE0 && c <= 0xEF ? 3 : c >= 0xF0 && c <= 0xF4 ? 4 : 0; \
int j = 1; boolean ok = len > 0; \
for (; ok && j < len; j++) { if (i + j >= n) { ok = false; break; } int cc = a[i + j] & 0xFF, lo = 0x80, hi = 0xBF; \
if (j == 1) { if (c == 0xE0) lo = 0xA0; if (c == 0xED) hi = 0x9F; if (c == 0xF0) lo = 0x90; if (c == 0xF4) hi = 0x8F; } \
if (cc < lo || cc > hi) { ok = false; break; } } \
if (ok) { i += len; } else { bad = Math.max(1, j); break; } } \
String valid = new String(a, start, i - start, java.nio.charset.StandardCharsets.UTF_8); \
out.add(Map.entry(valid, new ArrayList<>(b.subList(i, Math.min(n, i + bad))))); i = Math.min(n, i + bad); } return out; }";

// `a..b` / `a..=b` / `..` over list `v`, as Java `[from, to)` bounds.
fn range_ends(cx: &mut Cx, r: &ast::RangeExpr, v: &Expr, out: &mut Vec<Stmt>) -> (Expr, Expr) {
    use ast::RangeItem;
    let from = r.start().map(|s| expr::lower_expr(cx, &s, out)).unwrap_or_else(|| Expr::raw("0"));
    let to = match r.end() {
        Some(e) => {
            let e = expr::lower_expr(cx, &e, out);
            if r.op_kind() == Some(ast::RangeOp::Inclusive) { Expr::bin(e, "+", Expr::raw("1")) } else { e }
        }
        None => Expr::call(v.clone(), "size", vec![]),
    };
    (from, to)
}

// `N` of a const-generic slice method: `v.first_chunk::<2>()`.
pub(crate) fn const_arg(m: &ast::MethodCallExpr) -> Option<String> {
    use ra_ap_syntax::ast::HasGenericArgs;
    let arg = m.generic_arg_list()?.generic_args().next()?;
    Some(arg.syntax().text().to_string().trim_matches(|c| c == '{' || c == '}' || c == ' ').to_string())
}

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
        "get" | "get_mut" => {
            if let Some(ast::Expr::RangeExpr(r)) = arg_asts.first() {
                let sub = lower_slice_range(cx, recv, &Ty::Vec(Box::new(elem.clone())), r, out);
                return Some(opt_of(sub));
            }
            // Not `stream().skip(i).findFirst()`: that walks i elements, making index loops quadratic.
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let mut cond = Expr::bin(i.clone(), "<", Expr::call(recv.clone(), "size", vec![]));
            // `v.get(i - 1)` / `v.get(x as usize)` wrap to a huge usize (None); in Java they go negative.
            if !arg_asts.first().is_some_and(|a| non_negative(cx, a, 3)) {
                cond = Expr::bin(Expr::bin(i.clone(), ">=", Expr::raw("0")), "&&", cond);
            }
            Some(conditional(
                cond,
                opt_of(Expr::call(recv, "get", vec![i])),
                opt_empty(witness.as_ref()),
            ))
        }
        "first" | "first_mut" => Some(Expr::call(
            Expr::call(recv, "stream", vec![]),
            "findFirst",
            vec![],
        )),
        // `reversed()` is a view, so this neither copies nor throws on an empty list.
        "last" | "last_mut" => Some(Expr::call(
            Expr::call(Expr::call(recv, "reversed", vec![]), "stream", vec![]),
            "findFirst",
            vec![],
        )),
        "insert" => Some(Expr::call(recv, "add", vec![b0, b1])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "swap" => Some(Expr::call(
            Expr::raw("Collections"),
            "swap",
            vec![recv, b0, b1],
        )),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "split_off" if !matches!(arg_asts.first().map(strip_parens), Some(ast::Expr::RangeExpr(_))) => {
            let p = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (ps, ns) = (render(&p), render(&n));
            let tail = push_local(
                cx,
                line,
                format!("List<{}>", elem.boxed()),
                Some(Expr::raw(format!(
                    "new ArrayList<>({ps}.subList({ns}, {ps}.size()))"
                ))),
                out,
                "tail",
            );
            push_raw(
                out,
                line,
                format!("{ps}.subList({ns}, {ps}.size()).clear();"),
            );
            Some(Expr::name(tail))
        }
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
        "sort" | "sort_unstable" => {
            let cmp = ord_comparator(cx, elem).unwrap_or_else(|| "null".into());
            Some(Expr::call(recv, "sort", vec![Expr::raw(cmp)]))
        }
        "sort_by" | "sort_unstable_by" => Some(Expr::call(recv, "sort", vec![b0])),
        // Caching the keys only saves work; the order is `sort_by_key`'s.
        "sort_by_key" | "sort_unstable_by_key" | "sort_by_cached_key" => {
            let key_ty = match arg_asts.first() {
                Some(ast::Expr::ClosureExpr(c)) => c.body().map(|b| cx.ty(&b)),
                _ => None,
            };
            let body = match arg_asts.first() {
                Some(ast::Expr::ClosureExpr(c)) => c.body(),
                _ => None,
            };
            if let (Some(body), Some(t)) = (&body, &key_ty)
                && let Some(order) = key_comparator(cx, body, t)
            {
                let f = Expr::call(
                    Expr::raw("Comparator"),
                    "comparing",
                    vec![typed_lambda(b0, elem), Expr::raw(order)],
                );
                return Some(Expr::call(recv, "sort", vec![f]));
            }
            // A key function value (`let key = |s| (s.len(), ..)`): order its tuple keys too.
            let key_ty = key_ty.or_else(|| match arg_asts.first().map(|a| cx.ty(a)) {
                Some(Ty::Fn { ret, .. }) => Some(*ret),
                _ => None,
            });
            if let Some(t) = &key_ty
                && matches!(t.strip_refs(), Ty::Tuple(_))
                && let Some(order) = ord_comparator(cx, t)
            {
                let f = Expr::call(
                    Expr::raw("Comparator"),
                    "comparing",
                    vec![typed_lambda(b0, elem), Expr::raw(order)],
                );
                return Some(Expr::call(recv, "sort", vec![f]));
            }
            let cmp = match key_ty.as_ref().map(|t| t.strip_refs().clone()) {
                Some(Ty::Int(i)) if i.is_long() => "comparingLong",
                Some(Ty::Int(_)) => "comparingInt",
                Some(Ty::F32 | Ty::F64) => "comparingDouble",
                _ => "comparing",
            };
            let f = Expr::call(Expr::raw("Comparator"), cmp, vec![typed_lambda(b0, elem)]);
            Some(Expr::call(recv, "sort", vec![f]))
        }
        "reverse" => Some(Expr::call(Expr::raw("Collections"), "reverse", vec![recv])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        // Slice prefix/suffix: the same-length sublist compared with `equals`.
        "starts_with" | "ends_with" => {
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let w = simple(cx, b0, &Ty::Vec(Box::new(elem.clone())), line, out);
            let (vs, ws) = (render(&v), render(&w));
            let part = if name == "starts_with" {
                format!("{vs}.subList(0, {ws}.size())")
            } else {
                format!("{vs}.subList({vs}.size() - {ws}.size(), {vs}.size())")
            };
            Some(Expr::raw(format!(
                "({vs}.size() >= {ws}.size() && {part}.equals({ws}))"
            )))
        }
        "binary_search_by" | "binary_search_by_key" | "partition_point" => {
            if cx.lambda_depth > 0 {
                return Some(cx.unsupported(m.syntax(), &format!("{name} inside a closure")));
            }
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let (f, key) = match name {
                "binary_search_by_key" => {
                    let key_ty = arg_asts.first().map(|a| cx.ty(a)).unwrap_or(Ty::Unit);
                    let key = simple(cx, b0, &key_ty, line, out);
                    (b1, Some((key, key_ty)))
                }
                _ => (b0, None),
            };
            let partition = name == "partition_point";
            // `let pos = v.partition_point(..)`: the search index is `pos` itself.
            let target = if partition {
                cx.take_let_target(m.syntax())
            } else {
                None
            };
            Some(binary_search_loop(
                cx, partition, target, recv, elem, f, key, line, out,
            ))
        }
        "binary_search" => {
            if cx.lambda_depth > 0 {
                return Some(cx.unsupported(m.syntax(), "binary_search inside a closure"));
            }
            cx.needs_result = true;
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let key = simple(cx, b0, elem, line, out);
            let n = push_local(
                cx,
                line,
                "int".into(),
                Some(Expr::call(
                    Expr::raw("Collections"),
                    "binarySearch",
                    vec![recv.clone(), key.clone()],
                )),
                out,
                "pos",
            );
            push_raw(
                out,
                line,
                format!(
                    "while ({n} >= 0 && {n} + 1 < {}.size() && {}.get({n} + 1).compareTo({}) == 0) {n}++;",
                    render(&recv),
                    render(&recv),
                    render(&key)
                ),
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
        "extend_from_slice" => Some(Expr::call(recv, "addAll", vec![b0])),
        // `dst[a..b].copy_from_slice(src)`: the receiver is already a write-through subList view.
        "copy_from_slice" | "clone_from_slice" => {
            Some(Expr::call(Expr::raw("Collections"), "copy", vec![recv, b0]))
        }
        // Shared copies are only safe for immutable elements; Rust clones each one.
        "resize"
            if matches!(
                elem.strip_refs(),
                Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Bool | Ty::Char | Ty::Str
            ) && m
                .syntax()
                .parent()
                .is_some_and(|p| ast::ExprStmt::can_cast(p.kind())) =>
        {
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            // Simple mode: grow and shrink loops; `v.size()` / `s.length()` can be re-read.
            let rereadable = matches!(&b0, Expr::Call { recv: Some(r), name, args }
                if args.is_empty() && matches!(name.as_str(), "size" | "length") && is_simple(r));
            if crate::mode::simple() {
                let n = if rereadable {
                    b0
                } else {
                    let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
                    match n {
                        Expr::Name(_) | Expr::Raw(_) => n,
                        n => Expr::name(push_local(cx, line, "int".into(), Some(n), out, "len")),
                    }
                };
                let size = Expr::call(recv.clone(), "size", vec![]);
                let grow = Stmt::new(
                    line,
                    StmtKind::While {
                        label: None,
                        cond: Expr::bin(size.clone(), "<", n.clone()),
                        body: crate::jir::Block {
                            stmts: vec![Stmt::new(
                                line,
                                StmtKind::Expr(Expr::call(recv.clone(), "add", vec![b1])),
                            )],
                            close_line: line,
                        },
                    },
                );
                let shrink = Stmt::new(
                    line,
                    StmtKind::While {
                        label: None,
                        cond: Expr::bin(size.clone(), ">", n),
                        body: crate::jir::Block {
                            stmts: vec![Stmt::new(
                                line,
                                StmtKind::Expr(Expr::call(
                                    recv,
                                    "remove",
                                    vec![Expr::bin(size, "-", Expr::raw("1"))],
                                )),
                            )],
                            close_line: line,
                        },
                    },
                );
                out.push(grow);
                out.push(shrink);
                return Some(Expr::raw("null"));
            }
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (v, n) = (render(&recv), render(&n));
            push_raw(
                out,
                line,
                format!(
                    "if ({n} < {v}.size()) {v}.subList({n}, {v}.size()).clear(); else {v}.addAll(Collections.nCopies({n} - {v}.size(), {}));",
                    render(&b1)
                ),
            );
            Some(Expr::raw("null"))
        }
        // Rust leaves the other elements in an unspecified order; sorting them all is one choice.
        "select_nth_unstable"
            if m.syntax()
                .parent()
                .is_some_and(|p| ast::ExprStmt::can_cast(p.kind())) =>
        {
            Some(Expr::call(recv, "sort", vec![Expr::raw("null")]))
        }
        // Sorted in full, then `(left, nth, right)` with the halves as views.
        "select_nth_unstable" | "select_nth_unstable_by" | "select_nth_unstable_by_key" => {
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let cmp = match name {
                "select_nth_unstable" => Expr::raw("null"),
                "select_nth_unstable_by" => Expr::Cast(format!("Comparator<{}>", elem.boxed()), Box::new(b1.clone())),
                _ => {
                    let key = arg_asts.get(1).and_then(|a| match strip_parens(a) {
                        ast::Expr::ClosureExpr(c) => c.body().map(|b| cx.ty(&b)),
                        _ => None,
                    });
                    let key = key.unwrap_or(Ty::Other("?".into()));
                    let f = Expr::Cast(format!("Function<{}, {}>", elem.boxed(), key.boxed()), Box::new(b1.clone()));
                    Expr::raw(format!("Comparator.comparing({})", render(&f)))
                }
            };
            push_raw(out, line, format!("{}.sort({});", render(&v), render(&cmp)));
            let (vs, is) = (render(&v), render(&i));
            let parts = vec![
                Expr::raw(format!("{vs}.subList(0, {is})")),
                Expr::raw(format!("{vs}.get({is})")),
                Expr::raw(format!("{vs}.subList({is} + 1, {vs}.size())")),
            ];
            Some(expr::tuple_expr(cx, ret, parts))
        }
        // `subList` views write through, like the two `&mut` halves.
        "split_at" | "split_at_mut" | "split_at_checked" | "split_at_mut_checked" => {
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let i = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let a = Expr::call(recv.clone(), "subList", vec![Expr::raw("0"), i.clone()]);
            let b = Expr::call(
                recv.clone(),
                "subList",
                vec![i.clone(), Expr::call(recv.clone(), "size", vec![])],
            );
            let pair_ty = match ret.strip_refs() {
                Ty::Option(t) => (**t).clone(),
                t => t.clone(),
            };
            let pair = expr::tuple_expr(cx, &pair_ty, vec![a, b]);
            if !name.ends_with("_checked") {
                return Some(pair);
            }
            let ok = Expr::bin(i, "<=", Expr::call(recv, "size", vec![]));
            Some(conditional(ok, opt_of(pair), opt_empty(Some(&pair_ty))))
        }
        // Const-generic views: `first_chunk::<N>()`, `as_array::<N>()`, `as_chunks::<N>()`.
        "first_chunk" | "first_chunk_mut" | "last_chunk" | "last_chunk_mut" | "split_first_chunk"
        | "split_first_chunk_mut" | "split_last_chunk" | "split_last_chunk_mut" | "as_array" | "as_mut_array"
        | "as_chunks" | "as_chunks_mut" | "as_rchunks" | "as_rchunks_mut" => {
            let Some(n) = const_arg(m) else {
                return Some(cx.unsupported(m.syntax(), &format!("`{name}` without a `::<N>` length")));
            };
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let vs = render(&v);
            let sub = |a: String, b: String| Expr::raw(format!("{vs}.subList({a}, {b})"));
            let len = format!("{vs}.size()");
            let inner = match ret.strip_refs() {
                Ty::Option(t) => (**t).clone(),
                t => t.clone(),
            };
            let (cond, value) = match name.trim_end_matches("_mut") {
                "first_chunk" => (format!("{len} >= {n}"), sub("0".into(), n.clone())),
                "last_chunk" => (format!("{len} >= {n}"), sub(format!("{len} - {n}"), len.clone())),
                "split_first_chunk" => (
                    format!("{len} >= {n}"),
                    expr::tuple_expr(cx, &inner, vec![sub("0".into(), n.clone()), sub(n.clone(), len.clone())]),
                ),
                "split_last_chunk" => (
                    format!("{len} >= {n}"),
                    expr::tuple_expr(cx, &inner, vec![sub("0".into(), format!("{len} - {n}")), sub(format!("{len} - {n}"), len.clone())]),
                ),
                "as_array" | "as_mut_array" => (format!("{len} == {n}"), v.clone()),
                "as_chunks" => {
                    super::super::fmt::file_helper(cx, SLICE_VIEW_HELPERS.into());
                    let chunks = Expr::static_call("chunks_of", vec![v.clone(), Expr::raw("0"), Expr::raw(n.clone())]);
                    let rest = sub(format!("{len} - {len} % {n}"), len.clone());
                    return Some(expr::tuple_expr(cx, ret, vec![chunks, rest]));
                }
                _ => {
                    super::super::fmt::file_helper(cx, SLICE_VIEW_HELPERS.into());
                    let head = sub("0".into(), format!("{len} % {n}"));
                    let chunks = Expr::static_call("chunks_of", vec![v.clone(), Expr::raw(format!("{len} % {n}")), Expr::raw(n.clone())]);
                    return Some(expr::tuple_expr(cx, ret, vec![head, chunks]));
                }
            };
            Some(conditional(Expr::raw(cond), opt_of(value), opt_empty(Some(&inner))))
        }
        // `s.split_off_first()` on a `&[T]` binding: the item, and `s` shrinks past it.
        "split_off_first" | "split_off_first_mut" | "split_off_last" | "split_off_last_mut" => {
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let vs = render(&v);
            let first = name.starts_with("split_off_first");
            let item = if first { format!("{vs}.getFirst()") } else { format!("{vs}.getLast()") };
            let taken = push_local(
                cx,
                line,
                format!("Optional<{}>", elem.boxed()),
                Some(Expr::raw(format!("{vs}.isEmpty() ? Optional.empty() : Optional.of({item})"))),
                out,
                "taken",
            );
            let rest = if first { format!("{vs}.subList(1, {vs}.size())") } else { format!("{vs}.subList(0, {vs}.size() - 1)") };
            let mut shrink = Vec::new();
            let assign = expr::place_assign(cx, _recv_ast, "=", Expr::raw(rest), &mut shrink);
            out.extend(shrink);
            push_raw(out, line, format!("if (!{vs}.isEmpty()) {};", render(&assign)));
            Some(Expr::name(taken))
        }
        // `s.split_off(..k)` / `s.split_off(k..)` on a slice binding (a `Vec`'s takes an index).
        "split_off" | "split_off_mut" if matches!(arg_asts.first().map(strip_parens), Some(ast::Expr::RangeExpr(_))) => {
            use ast::RangeItem;
            let Some(ast::Expr::RangeExpr(r)) = arg_asts.first().map(strip_parens) else { return None };
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let vs = render(&v);
            let (bound, front) = match (r.start(), r.end()) {
                (None, Some(e)) => (expr::lower_expr(cx, &e, out), true),
                (Some(s), None) => (expr::lower_expr(cx, &s, out), false),
                _ => return Some(cx.unsupported(m.syntax(), "split_off with a two-sided range")),
            };
            let k = render(&simple(cx, bound, &Ty::Int(Int::Usize), line, out));
            let (removed, kept) = if front {
                (format!("{vs}.subList(0, {k})"), format!("{vs}.subList({k}, {vs}.size())"))
            } else {
                (format!("{vs}.subList({k}, {vs}.size())"), format!("{vs}.subList(0, {k})"))
            };
            let taken = push_local(
                cx,
                line,
                format!("Optional<List<{}>>", elem.boxed()),
                Some(Expr::raw(format!("{k} <= {vs}.size() ? Optional.of({removed}) : Optional.empty()"))),
                out,
                "taken",
            );
            let mut shrink = Vec::new();
            let assign = expr::place_assign(cx, _recv_ast, "=", Expr::raw(kept), &mut shrink);
            out.extend(shrink);
            push_raw(out, line, format!("if ({taken}.isPresent()) {};", render(&assign)));
            Some(Expr::name(taken))
        }
        // `[i, j]` distinct and in range: the items (shared objects, so writes reach the list).
        "get_disjoint_mut" => {
            cx.needs_result = true;
            let v = render(&simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out));
            let idx = render(&simple(cx, b0, &Ty::Vec(Box::new(Ty::Int(Int::Usize))), line, out));
            let i = cx.fresh_param("i");
            cx.release_param(&i);
            let ok_ty = match ret.strip_refs() {
                Ty::Result(ok, _) => ok.boxed(),
                _ => "Object".into(),
            };
            Some(Expr::raw(format!(
                "({idx}.stream().allMatch({i} -> {i} < {v}.size()) && {idx}.stream().distinct().count() == {idx}.size() ? new Ok<{ok_ty}, String>({idx}.stream().map({v}::get).collect(Collectors.toCollection(ArrayList::new))) : new Err<{ok_ty}, String>(\"GetDisjointMutError\"))"
            )))
        }
        // A writable view of `[[T; N]]` as `[T]`.
        "as_flattened" | "as_flattened_mut" => {
            cx.import("java.util.AbstractList");
            super::super::fmt::file_helper(cx, SLICE_VIEW_HELPERS.into());
            Some(Expr::static_call("flat_view", vec![recv]))
        }
        "strip_prefix" | "strip_suffix" | "strip_circumfix" | "trim_prefix" | "trim_suffix" => {
            super::super::fmt::file_helper(cx, SLICE_EDIT_HELPERS.into());
            let none = Expr::raw("List.of()");
            let (pre, suf) = match name {
                "strip_prefix" | "trim_prefix" => (b0.clone(), none),
                "strip_suffix" | "trim_suffix" => (none, b0.clone()),
                _ => (b0.clone(), b1.clone()),
            };
            if name.starts_with("trim_") {
                let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
                let stripped = Expr::static_call("slice_strip", vec![v.clone(), pre, suf]);
                return Some(Expr::call(stripped, "orElse", vec![v]));
            }
            Some(Expr::static_call("slice_strip", vec![recv, pre, suf]))
        }
        "is_sorted" | "is_sorted_by" | "is_sorted_by_key" => {
            super::super::fmt::file_helper(cx, crate::lower::iter::IS_SORTED.into());
            let (x, y) = (cx.fresh_param("x"), cx.fresh_param("y"));
            cx.release_param(&x);
            cx.release_param(&y);
            let le = |cx: &mut Cx, t: &Ty| {
                let b = t.boxed();
                match t.strip_refs() {
                    t if t.is_primitive() => format!("({b} {x}, {b} {y}) -> {x} <= {y}"),
                    t => format!("({b} {x}, {b} {y}) -> {} <= 0", compare_call(cx, t, &x, &y)),
                }
            };
            Some(match name {
                "is_sorted" => Expr::static_call("is_sorted", vec![recv, Expr::raw(le(cx, elem))]),
                "is_sorted_by" => Expr::static_call("is_sorted", vec![recv, b0]),
                _ => {
                    let key = arg_asts.first().and_then(|a| match strip_parens(a) {
                        ast::Expr::ClosureExpr(c) => c.body().map(|b| cx.ty(&b)),
                        _ => None,
                    });
                    let key = key.unwrap_or(Ty::Other("?".into()));
                    let keys = Expr::call(
                        Expr::call(Expr::call(recv, "stream", vec![]), &format!("<{}>map", key.boxed()), vec![b0]),
                        "toList",
                        vec![],
                    );
                    Expr::static_call("is_sorted", vec![keys, Expr::raw(le(cx, &key))])
                }
            })
        }
        "fill_with" | "swap_with_slice" => {
            super::super::fmt::file_helper(cx, SLICE_EDIT_HELPERS.into());
            Some(Expr::static_call(name, vec![recv, b0]))
        }
        "copy_within" => {
            super::super::fmt::file_helper(cx, SLICE_EDIT_HELPERS.into());
            let Some(ast::Expr::RangeExpr(r)) = arg_asts.first().map(strip_parens) else {
                return Some(cx.unsupported(m.syntax(), "copy_within without a range literal"));
            };
            let v = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let (from, to) = range_ends(cx, &r, &v, out);
            Some(Expr::static_call("copy_within", vec![v, from, to, b1.clone()]))
        }
        "repeat" => {
            let copies = Expr::call(Expr::raw("Collections"), "nCopies", vec![b0, recv]);
            Some(Expr::call(
                Expr::call(Expr::call(copies, "stream", vec![]), "flatMap", vec![Expr::raw("List::stream")]),
                "collect",
                vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
            ))
        }
        // `[[1], [2, 3]].connect(&0)`: the lists with a separator item (or items) between them.
        "connect" | "join" if matches!(elem.strip_refs(), Ty::Vec(_)) => {
            super::super::fmt::file_helper(cx, SLICE_EDIT_HELPERS.into());
            Some(Expr::static_call("slice_join", vec![recv, b0]))
        }
        "connect" => lower_vec_method(cx, m, _recv_ast, recv, elem, args, arg_asts, "join", ret, line, out),
        "into_vec" => Some(recv),
        // `[u8]` as ASCII text.
        "is_ascii" => {
            let b = cx.fresh_param("b");
            cx.release_param(&b);
            Some(Expr::call(Expr::call(recv, "stream", vec![]), "allMatch", vec![Expr::raw(format!("{b} -> {b} < 128"))]))
        }
        "eq_ignore_ascii_case" | "to_ascii_uppercase" | "to_ascii_lowercase" | "make_ascii_uppercase" | "make_ascii_lowercase" => {
            super::super::fmt::file_helper(cx, super::text::ASCII_CHAR_HELPERS.into());
            let b = cx.fresh_param("b");
            cx.release_param(&b);
            let f = if name.ends_with("uppercase") { "ascii_upper" } else { "ascii_lower" };
            Some(match name {
                "eq_ignore_ascii_case" => {
                    let lower = |e: Expr| {
                        Expr::call(Expr::call(e, "stream", vec![]), "map", vec![Expr::raw(format!("{b} -> ascii_lower({b})"))])
                    };
                    Expr::call(Expr::call(lower(recv), "toList", vec![]), "equals", vec![Expr::call(lower(b0), "toList", vec![])])
                }
                n if n.starts_with("make_") => Expr::call(recv, "replaceAll", vec![Expr::raw(format!("{b} -> {f}({b})"))]),
                _ => Expr::call(
                    Expr::call(Expr::call(recv, "stream", vec![]), "map", vec![Expr::raw(format!("{b} -> {f}({b})"))]),
                    "collect",
                    vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
                ),
            })
        }
        "trim_ascii" | "trim_ascii_start" | "trim_ascii_end" => {
            super::super::fmt::file_helper(cx, BYTES_HELPERS.into());
            let (s, e) = match name {
                "trim_ascii" => ("true", "true"),
                "trim_ascii_start" => ("true", "false"),
                _ => ("false", "true"),
            };
            Some(Expr::static_call("trim_ascii_bytes", vec![recv, Expr::raw(s), Expr::raw(e)]))
        }
        "split_first" | "split_last" | "split_first_mut" | "split_last_mut" => {
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let last = name.starts_with("split_last");
            let size = Expr::call(recv.clone(), "size", vec![]);
            let item = Expr::call(
                recv.clone(),
                if last { "getLast" } else { "getFirst" },
                vec![],
            );
            let start = if last { Expr::raw("0") } else { Expr::raw("1") };
            let end = if last {
                Expr::bin(size, "-", Expr::raw("1"))
            } else {
                size
            };
            let rest = Expr::call(recv.clone(), "subList", vec![start, end]);
            let pair_ty = match ret.strip_refs() {
                Ty::Option(t) => (**t).clone(),
                t => t.clone(),
            };
            let pair = expr::tuple_expr(cx, &pair_ty, vec![item, rest]);
            Some(optional(
                Expr::Unary("!", Box::new(Expr::call(recv, "isEmpty", vec![]))),
                pair,
                ret,
            ))
        }
        "to_vec" | "to_owned" => Some(clone_elems(cx, m, recv, elem)),
        "concat" if matches!(elem.strip_refs(), Ty::Vec(t) if super::super::is_cell_value_ty(t)) => {
            let flat = Expr::call(
                Expr::call(recv, "stream", vec![]),
                "flatMap",
                vec![Expr::raw("Collection::stream")],
            );
            Some(Expr::call(
                flat,
                "collect",
                vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
            ))
        }
        "rotate_left" | "rotate_right" => {
            if cx.lambda_depth > 0 {
                return Some(cx.unsupported(m.syntax(), "slice rotation inside a closure"));
            }
            let recv = simple(cx, recv, &Ty::Vec(Box::new(elem.clone())), line, out);
            let n = simple(cx, b0, &Ty::Int(Int::Usize), line, out);
            let (v, n) = (render(&recv), render(&n));
            let distance = if name == "rotate_left" {
                format!("-{n}")
            } else {
                n.clone()
            };
            push_raw(
                out,
                line,
                format!(
                    "if ({n} < 0 || {n} > {v}.size()) throw new IndexOutOfBoundsException(); Collections.rotate({v}, {distance});"
                ),
            );
            Some(Expr::raw("null"))
        }
        "fill" if super::super::is_cell_value_ty(elem) => {
            Some(Expr::call(Expr::raw("Collections"), "fill", vec![recv, b0]))
        }
        "fill" if matches!(elem.strip_refs(), Ty::Vec(inner) if super::super::is_cell_value_ty(inner)) =>
        {
            if cx.lambda_depth > 0 {
                return Some(cx.unsupported(m.syntax(), "slice fill inside a closure"));
            }
            let list = push_local(
                cx,
                line,
                Ty::Vec(Box::new(elem.clone())).java(),
                Some(recv),
                out,
                "list",
            );
            let value = push_local(cx, line, elem.java(), Some(b0), out, "fill");
            let index = cx.fresh("index");
            push_raw(
                out,
                line,
                format!(
                    "for (int {index} = 0; {index} < {list}.size(); {index}++) {list}.set({index}, new ArrayList<>({value}));"
                ),
            );
            Some(Expr::raw("null"))
        }
        "clone" => Some(clone_elems(cx, m, recv, elem)),
        "as_slice" | "as_mut" | "as_mut_slice" | "into_boxed_slice" | "each_ref" | "each_mut" => Some(recv),
        // `[T; N]::map`: a new array of the results.
        "map" => {
            let f = args.first().cloned()?;
            Some(Expr::call(
                Expr::call(Expr::call(recv, "stream", vec![]), "map", vec![f]),
                "collect",
                vec![Expr::raw("Collectors.toCollection(ArrayList::new)")],
            ))
        }
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
        "first_key_value" | "last_key_value" | "pop_first" | "pop_last" if ordered => {
            let method = match name {
                "first_key_value" => "firstEntry",
                "last_key_value" => "lastEntry",
                "pop_first" => "pollFirstEntry",
                _ => "pollLastEntry",
            };
            Some(Expr::call(
                Expr::raw("Optional"),
                "ofNullable",
                vec![Expr::call(recv, method, vec![])],
            ))
        }
        "retain" => {
            let e = cx.fresh("entry");
            let predicate = Expr::Cast(
                format!("BiPredicate<{}, {}>", _k.boxed(), _v.boxed()),
                Box::new(b0),
            );
            let test = Expr::call(
                predicate,
                "test",
                vec![
                    Expr::call(Expr::name(e.clone()), "getKey", vec![]),
                    Expr::call(Expr::name(e.clone()), "getValue", vec![]),
                ],
            );
            Some(Expr::call(
                Expr::call(recv, "entrySet", vec![]),
                "removeIf",
                vec![Expr::Lambda {
                    params: vec![e],
                    body: LambdaBody::Expr(Box::new(Expr::Unary("!", Box::new(test)))),
                }],
            ))
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
    m: &ast::MethodCallExpr,
    _recv_ast: &ast::Expr,
    recv: Expr,
    ordered: bool,
    elem: &Ty,
    args: Vec<Expr>,
    arg_asts: &[ast::Expr],
    name: &str,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let mut b0 = args.first().cloned().unwrap_or_else(|| Expr::raw("null"));
    let mut recv = recv;
    if matches!(
        name,
        "union"
            | "intersection"
            | "difference"
            | "symmetric_difference"
            | "is_subset"
            | "is_superset"
    ) {
        if cx.lambda_depth > 0 && (!matches!(recv, Expr::Name(_)) || !matches!(b0, Expr::Name(_))) {
            return Some(cx.unsupported(
                m.syntax(),
                "set algebra with temporary operands inside a closure",
            ));
        }
        let ty = Ty::Set {
            ordered,
            t: Box::new(elem.clone()),
        };
        recv = simple(cx, recv, &ty, line, out);
        b0 = simple(cx, b0, &ty, line, out);
    }
    match name {
        "insert" => Some(Expr::call(recv, "add", vec![b0])),
        "contains" => Some(Expr::call(recv, "contains", vec![b0])),
        "remove" => Some(Expr::call(recv, "remove", vec![b0])),
        "len" => Some(Expr::call(recv, "size", vec![])),
        "is_empty" => Some(Expr::call(recv, "isEmpty", vec![])),
        "clear" => Some(Expr::call(recv, "clear", vec![])),
        "retain" => Some(Expr::call(recv, "removeIf", vec![negate_lambda(b0)])),
        "first" | "last" if ordered => {
            let source = if name == "last" {
                Expr::call(recv, "descendingSet", vec![])
            } else {
                recv
            };
            Some(Expr::call(
                Expr::call(source, "stream", vec![]),
                "findFirst",
                vec![],
            ))
        }
        "pop_first" | "pop_last" if ordered => Some(Expr::call(
            Expr::raw("Optional"),
            "ofNullable",
            vec![Expr::call(
                recv,
                if name == "pop_first" {
                    "pollFirst"
                } else {
                    "pollLast"
                },
                vec![],
            )],
        )),
        "extend" => {
            let ty = arg_asts.first().map(|a| cx.ty(a));
            let items = if matches!(ty.as_ref().map(Ty::strip_refs), Some(Ty::Iter(_))) {
                Expr::call(b0, "toList", vec![])
            } else {
                b0
            };
            Some(Expr::call(recv, "addAll", vec![items]))
        }
        "union" => {
            let s = render(&recv);
            let o = render(&b0);
            let sorted = if ordered { ".sorted()" } else { "" };
            Some(Expr::raw(format!(
                "Stream.concat({s}.stream(), {o}.stream()).distinct(){sorted}"
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
            let sorted = if ordered { ".sorted()" } else { "" };
            Some(Expr::raw(format!(
                "Stream.concat({s}.stream().filter({x} -> !{o}.contains({x})), {o}.stream().filter({x} -> !{s}.contains({x}))){sorted}"
            )))
        }
        "is_disjoint" => Some(Expr::call(
            Expr::raw("Collections"),
            "disjoint",
            vec![recv, b0],
        )),
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
            let _ = (elem, line, out);
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
        "get"
            if m.arg_list()
                .and_then(|a| a.args().next())
                .is_some_and(|a| a.syntax().text() == "0") =>
        {
            Some(opt_nullable("peekFirst"))
        }
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
    recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = ret;
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
        "into_sorted_vec" => {
            let recv_ty = cx.ty(recv_ast);
            let recv = simple(cx, recv, &recv_ty, line, out);
            let heap = render(&recv);
            let item = cx.fresh("item");
            Some(Expr::New {
                ty: "ArrayList<>".into(),
                args: vec![Expr::raw(format!(
                    "Stream.generate(() -> {heap}.poll()).takeWhile({item} -> {item} != null).toList().reversed()"
                ))],
            })
        }
        "into_vec" => Some(Expr::New {
            ty: "ArrayList<>".into(),
            args: vec![recv],
        }),
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
    let read_only = std::mem::take(&mut cx.read_only_literal);
    let exprs: Vec<ast::Expr> = a.exprs().collect();
    if a.semicolon_token().is_some() && exprs.len() == 2 {
        return lower_repeat(cx, a.syntax(), &exprs[0], &exprs[1], out);
    }
    let mut items: Vec<Expr> = exprs.iter().map(|e| expr::lower_expr(cx, e, out)).collect();
    // `List.of((short) 1, 2)` mixes Short and Integer.
    if let Some(Ty::Int(i)) = exprs.first().map(|e| cx.ty(e).strip_refs().clone())
        && matches!(i.java(), "short" | "byte")
    {
        items = items
            .into_iter()
            .map(|e| match e {
                Expr::Cast(..) => e,
                e => Expr::Cast(i.java().into(), Box::new(e)),
            })
            .collect();
    }
    if items.is_empty() {
        // `f::<f64>(&[])`: an argument position gives the diamond nothing to infer from.
        let ty = match cx.ty(&ast::Expr::ArrayExpr(a.clone())).strip_refs() {
            Ty::Vec(t)
                if t.is_nameable()
                    && matches!(a.syntax().parent(), Some(p) if ast::ArgList::can_cast(p.kind()) || ast::RefExpr::can_cast(p.kind())) =>
            {
                format!("ArrayList<{}>", t.boxed())
            }
            _ => "ArrayList<>".into(),
        };
        return Expr::New { ty, args: vec![] };
    }
    // `()` elements are `null` in Java, which `List.of` rejects.
    let factory = if exprs.iter().any(|e| cx.ty(e).is_unit()) {
        ("Arrays", "asList".to_string())
    } else {
        ("List", "of".to_string())
    };
    // `List.of(Optional.of(1), Optional.empty())` infers `List<Optional<? extends Object>>`.
    let elem_ty = cx.ty(&exprs[0]);
    let untyped_empty = |e: &Expr| match e {
        Expr::Raw(s) => s == "Optional.empty()",
        Expr::New { ty, args } => ty.ends_with("<>") && args.is_empty(),
        _ => false,
    };
    let factory = if elem_ty.boxed().contains('<') && items.iter().any(untyped_empty) {
        (factory.0, format!("<{}>{}", elem_ty.boxed(), factory.1))
    } else {
        factory
    };
    let list = Expr::call(Expr::raw(factory.0), &factory.1, items);
    if read_only {
        return list;
    }
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
    let n = if super::is_simple(&n) || cx.lambda_depth > 0 {
        n
    } else {
        Expr::name(push_local(
            cx,
            cx.line(node),
            "int".into(),
            Some(n),
            out,
            "len",
        ))
    };
    if crate::lower::is_cell_value_ty(&elem_ty) || matches!(elem_ty.strip_refs(), Ty::Option(_)) {
        let x = expr::lower_expr(cx, elem, out);
        // `[0u16; 2]`: an int literal would make the list `List<Integer>`.
        let x = match elem_ty.strip_refs() {
            Ty::Int(i) if matches!(i.java(), "short" | "byte") && !matches!(x, Expr::Cast(..)) => {
                Expr::Cast(i.java().into(), Box::new(x))
            }
            _ => x,
        };
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
    // `mapToObj(i -> new ArrayList<>())` has no target type: spell out the type arguments.
    let x = match x {
        Expr::New { ty, args }
            if ty.ends_with("<>") && args.is_empty() && elem_ty.is_nameable() =>
        {
            let java = elem_ty.strip_refs().java();
            match java.find('<') {
                Some(at) => Expr::New {
                    ty: format!("{}{}", ty.trim_end_matches("<>"), &java[at..]),
                    args,
                },
                None => Expr::New { ty, args },
            }
        }
        x => x,
    };
    // Simple mode: fill a list in a loop, as people write `vec![vec![0; m]; n]` in Java.
    if crate::mode::simple() && cx.lambda_depth == 0 {
        let line = cx.line(node);
        let target = cx
            .take_let_target(node)
            .or_else(|| node.parent().and_then(|p| cx.take_let_target(&p)));
        let list = target.clone().unwrap_or_else(|| cx.fresh("rows"));
        let i = cx.fresh("i");
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: Ty::Vec(Box::new(elem_ty.clone())).java(),
                name: list.clone(),
                init: Some(Expr::New {
                    ty: "ArrayList<>".into(),
                    args: vec![],
                }),
            },
        ));
        out.push(Stmt::new(
            line,
            StmtKind::For {
                label: None,
                header: format!("int {i} = 0; {i} < {}; {i}++", render(&n)),
                body: crate::jir::Block {
                    stmts: vec![Stmt::new(
                        line,
                        StmtKind::Expr(Expr::call(Expr::name(&list), "add", vec![x])),
                    )],
                    close_line: line,
                },
            },
        ));
        return Expr::name(list);
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

// An index expression that can't have wrapped below zero.
pub(crate) fn non_negative(cx: &Cx, e: &ast::Expr, depth: u32) -> bool {
    match e {
        ast::Expr::Literal(_) | ast::Expr::FieldExpr(_) => true,
        ast::Expr::PathExpr(p) => {
            let Some(ra_ap_hir::PathResolution::Local(local)) =
                p.path().and_then(|p| cx.sema.resolve_path(&p))
            else {
                return true;
            };
            local_non_negative(cx, local, depth)
        }
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|i| non_negative(cx, &i, depth)),
        ast::Expr::RefExpr(r) => r.expr().is_some_and(|i| non_negative(cx, &i, depth)),
        // Values from calls are in range unless they wrap on purpose.
        ast::Expr::MethodCallExpr(m) => !m.name_ref().is_some_and(|n| {
            let n = n.text().to_string();
            n.starts_with("wrapping_") || n.starts_with("overflowing_")
        }),
        ast::Expr::CallExpr(_) | ast::Expr::TryExpr(_) => true,
        ast::Expr::BinExpr(b) => {
            matches!(
                b.op_kind(),
                Some(ast::BinaryOp::ArithOp(
                    ast::ArithOp::Add | ast::ArithOp::Mul | ast::ArithOp::Div | ast::ArithOp::Rem
                ))
            ) && b.lhs().is_some_and(|l| non_negative(cx, &l, depth))
                && b.rhs().is_some_and(|r| non_negative(cx, &r, depth))
        }
        _ => false,
    }
}

// Params, loop variables and pattern bindings count as in range; a `let` is judged by what it's set to.
fn local_non_negative(cx: &Cx, local: ra_ap_hir::Local, depth: u32) -> bool {
    let Some(ip) = local.primary_source(cx.db).into_ident_pat() else {
        return true;
    };
    let Some(l) = ip.syntax().parent().and_then(ast::LetStmt::cast) else {
        return true;
    };
    if depth == 0
        || !l
            .initializer()
            .is_some_and(|i| non_negative(cx, &i, depth - 1))
    {
        return false;
    }
    if ip.mut_token().is_none() {
        return true;
    }
    // A `-=` (or a reassignment from a possibly wrapped value) can take it below zero.
    let name = ast::HasName::name(&ip)
        .map(|n| n.text().to_string())
        .unwrap_or_default();
    let Some(body) = l.syntax().ancestors().find(|a| ast::Fn::can_cast(a.kind())) else {
        return false;
    };
    !body.descendants().filter_map(ast::BinExpr::cast).any(|b| {
        let Some(ast::BinaryOp::Assignment { op }) = b.op_kind() else {
            return false;
        };
        let target = b
            .lhs()
            .is_some_and(|t| t.syntax().text().to_string() == name);
        target
            && match op {
                Some(ast::ArithOp::Sub) => true,
                None => !b.rhs().is_some_and(|r| non_negative(cx, &r, depth - 1)),
                _ => false,
            }
    })
}

// Rust's binary search: the first index where `f` says Greater (or the predicate fails); an
// Equal match reports the last equal element, like std.
#[allow(clippy::too_many_arguments)]
fn binary_search_loop(
    cx: &mut Cx,
    partition: bool,
    target: Option<String>,
    recv: Expr,
    elem: &Ty,
    f: Expr,
    key: Option<(Expr, Ty)>,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Expr {
    let lo = target.unwrap_or_else(|| cx.fresh("lo"));
    let (hi, mid) = (cx.fresh("hi"), cx.fresh("mid"));
    let probe = Expr::call(recv.clone(), "get", vec![Expr::name(&mid)]);
    // `|x| body` is inlined with `x` bound to the probed element.
    let (bind, applied) = match f {
        Expr::Lambda {
            params,
            body: LambdaBody::Expr(body),
        } if params.len() == 1 => (
            format!("{} {} = {}; ", elem.java(), params[0], render(&probe)),
            *body,
        ),
        f => {
            let ret = if partition {
                "Boolean"
            } else if key.is_some() {
                "Object"
            } else {
                "Integer"
            };
            let call = if partition { "test" } else { "apply" };
            let iface = if partition {
                format!("Predicate<{}>", elem.boxed())
            } else {
                format!("Function<{}, {ret}>", elem.boxed())
            };
            (
                String::new(),
                Expr::call(Expr::Cast(iface, Box::new(f)), call, vec![probe]),
            )
        }
    };
    let cmp = match key {
        // Simple mode has no comparator for ints: a primitive key compares statically.
        Some((k, kty)) if crate::mode::simple() && kty.strip_refs().is_primitive() => {
            Expr::call(Expr::raw(kty.strip_refs().boxed()), "compare", vec![applied, k])
        }
        Some((k, kty)) => ord_compare(cx, &kty, applied, k),
        None => applied,
    };
    let n = render(&recv);
    if partition {
        push_raw(
            out,
            line,
            format!(
                "int {lo} = 0, {hi} = {n}.size(); while ({lo} < {hi}) {{ int {mid} = ({lo} + {hi}) >>> 1; {bind}if ({}) {lo} = {mid} + 1; else {hi} = {mid}; }}",
                render(&cmp)
            ),
        );
        return Expr::name(lo);
    }
    cx.needs_result = true;
    let (c, eq) = (cx.fresh("c"), cx.fresh("found"));
    push_raw(
        out,
        line,
        format!(
            "int {lo} = 0, {hi} = {n}.size(); boolean {eq} = false; while ({lo} < {hi}) {{ int {mid} = ({lo} + {hi}) >>> 1; {bind}int {c} = {}; if ({c} <= 0) {{ {lo} = {mid} + 1; {eq} = {c} == 0; }} else {hi} = {mid}; }}",
            render(&cmp)
        ),
    );
    conditional(
        Expr::name(&eq),
        Expr::New {
            ty: "Ok<Integer, Integer>".into(),
            args: vec![Expr::bin(Expr::name(&lo), "-", Expr::raw("1"))],
        },
        Expr::New {
            ty: "Err<Integer, Integer>".into(),
            args: vec![Expr::name(lo)],
        },
    )
}

// ---- simple mode: `binary_search` for its position only ----

// `v.binary_search(&x)` on a list, as a syntax node.
fn search_call(cx: &Cx, e: &ast::Expr) -> Option<ast::MethodCallExpr> {
    let ast::Expr::MethodCallExpr(m) = strip_parens(e) else { return None };
    let recv = m.receiver()?;
    (super::method_name(&m) == "binary_search" && matches!(cx.ty(&recv).strip_refs(), Ty::Vec(_)))
        .then_some(m)
}

// The arms of `match v.binary_search(&x)`: each `Ok(p)` / `Err(p)` / `_`, `p` a name or `_`.
fn search_arms(m: &ast::MatchExpr) -> Option<Vec<(Option<bool>, Option<String>, ast::MatchArm)>> {
    let mut out = Vec::new();
    for a in m.match_arm_list()?.arms() {
        let (ok, bind) = match a.pat()? {
            ast::Pat::WildcardPat(_) => (None, None),
            ast::Pat::TupleStructPat(tp) => {
                let ok = match tp.path()?.segment()?.name_ref()?.text().to_string().as_ref() {
                    "Ok" => true,
                    "Err" => false,
                    _ => return None,
                };
                let mut fields = tp.fields();
                let bind = match fields.next()? {
                    ast::Pat::WildcardPat(_) => None,
                    ast::Pat::IdentPat(ip) if ip.pat().is_none() => Some(ip.name()?.text().to_string()),
                    _ => return None,
                };
                if fields.next().is_some() {
                    return None;
                }
                (Some(ok), bind)
            }
            _ => return None,
        };
        out.push((ok, bind, a));
    }
    Some(out)
}

// Whether this `binary_search` is used only for its position, which simple mode reads from
// `Collections.binarySearch` directly instead of building a Result.
pub(crate) fn plain_search_use(cx: &Cx, e: &ast::Expr) -> bool {
    if !crate::mode::simple() || search_call(cx, e).is_none() {
        return false;
    }
    let Some(parent) = e.syntax().parent() else { return false };
    if let Some(m) = ast::MethodCallExpr::cast(parent.clone()) {
        return match super::method_name(&m).as_str() {
            "is_ok" | "is_err" => true,
            "unwrap_or_else" => insertion_closure(&m),
            _ => false,
        };
    }
    ast::MatchExpr::cast(parent).is_some_and(|m| search_arms(&m).is_some())
}

// `|e| e`: the insertion point on a miss.
fn insertion_closure(m: &ast::MethodCallExpr) -> bool {
    let Some(ast::Expr::ClosureExpr(c)) = m.arg_list().and_then(|a| a.args().next()) else {
        return false;
    };
    let param = c.param_list().and_then(|p| p.params().next()).and_then(|p| p.pat());
    let body = c.body().map(|b| b.syntax().text().to_string());
    matches!((param, body), (Some(ast::Pat::IdentPat(ip)), Some(b)) if ip.name().is_some_and(|n| n.text() == b.as_str()))
}

// `Collections.binarySearch(v, x)`.
fn search_expr(cx: &mut Cx, call: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let v = expr::lower_expr(cx, &call.receiver()?, out);
    let key = match strip_parens(&call.arg_list()?.args().next()?) {
        ast::Expr::RefExpr(r) => r.expr()?,
        k => k,
    };
    let key = expr::lower_expr(cx, &key, out);
    Some(Expr::call(Expr::raw("Collections"), "binarySearch", vec![v, key]))
}

// `v.binary_search(&x).is_ok()` → `Collections.binarySearch(v, x) >= 0`; `.unwrap_or_else(|i| i)`
// → `pos >= 0 ? pos : -pos - 1`.
pub(crate) fn lower_search_method(cx: &mut Cx, m: &ast::MethodCallExpr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let recv = m.receiver()?;
    if !plain_search_use(cx, &recv) {
        return None;
    }
    let call = search_call(cx, &recv)?;
    let line = cx.line(m.syntax());
    let search = search_expr(cx, &call, out)?;
    Some(match super::method_name(m).as_str() {
        "is_ok" => Expr::bin(search, ">=", Expr::raw("0")),
        "is_err" => Expr::bin(search, "<", Expr::raw("0")),
        _ => {
            let pos = push_local(cx, line, "int".into(), Some(search), out, "pos");
            let p = Expr::name(&pos);
            conditional(
                Expr::bin(p.clone(), ">=", Expr::raw("0")),
                p.clone(),
                Expr::bin(Expr::Unary("-", Box::new(p)), "-", Expr::raw("1")),
            )
        }
    })
}

// `match v.binary_search(&x) { Ok(i) => .., Err(i) if .. => .., Err(i) => .. }` as an if-chain
// on `int pos = Collections.binarySearch(v, x);`, where a found `i` is `pos` and a missing one
// `-pos - 1`.
pub(crate) fn lower_search_match(cx: &mut Cx, m: &ast::MatchExpr, dest: &super::super::stmt::Dest, out: &mut Vec<Stmt>) -> bool {
    let Some(scrut) = m.expr() else { return false };
    if !plain_search_use(cx, &scrut) {
        return false;
    }
    let (Some(call), Some(arms)) = (search_call(cx, &scrut), search_arms(m)) else { return false };
    let line = cx.line(m.syntax());
    let Some(search) = search_expr(cx, &call, out) else { return false };
    let base = if arms.iter().any(|(_, b, _)| b.as_deref() == Some("pos")) { "index" } else { "pos" };
    let pos = push_local(cx, line, "int".into(), Some(search), out, base);
    let missing = format!("-{pos} - 1");
    // `Ok(_) => {}` does nothing: drop it, and keep the Ok/Err test on the arms after it.
    let is_empty = |a: &ast::MatchArm| {
        a.guard().is_none()
            && matches!(a.expr(), Some(ast::Expr::BlockExpr(b)) if b.stmt_list().is_some_and(|l| l.statements().next().is_none() && l.tail_expr().is_none()))
    };
    let dropped = matches!(dest, super::super::stmt::Dest::Discard) && arms.iter().any(|(_, _, a)| is_empty(a));
    let mut kept = Vec::new();
    // Whether a kept unguarded arm before this one already took every found / missing position.
    let (mut found_done, mut missing_done) = (false, false);
    for (ok, bind, arm) in &arms {
        if dropped && is_empty(arm) {
            continue;
        }
        let side = match ok {
            Some(true) if !missing_done => Some(true),
            Some(false) if !found_done => Some(false),
            _ => None,
        };
        if arm.guard().is_none() {
            match ok {
                Some(true) => found_done = true,
                Some(false) => missing_done = true,
                None => {}
            }
        }
        kept.push((side, *ok, bind.clone(), arm.clone()));
    }
    let mut els: Option<Box<Stmt>> = None;
    let n = kept.len();
    for (i, (side, ok, bind, arm)) in kept.iter().enumerate().rev() {
        let aline = cx.line(arm.syntax());
        cx.push_scope();
        let mut stmts = Vec::new();
        if let Some(b) = bind {
            match ok {
                Some(true) => cx.declare_as(b, &pos),
                // With a guard the guard reads it too: spell it out; otherwise name it.
                _ if arm.guard().is_some() => cx.declare_as(b, &format!("({missing})")),
                _ => {
                    let java = cx.declare(b);
                    stmts.push(Stmt::new(
                        aline,
                        StmtKind::Local { ty: "int".into(), name: java, init: Some(Expr::raw(missing.clone())) },
                    ));
                }
            }
        }
        let mut conds = Vec::new();
        // rustc checked exhaustiveness: the last arm needs no test, unless an arm was dropped.
        if i + 1 < n || arm.guard().is_some() || dropped {
            if let Some(found) = side {
                conds.push(Expr::bin(Expr::name(&pos), if *found { ">=" } else { "<" }, Expr::raw("0")));
            }
            if let Some(g) = arm.guard().and_then(|g| g.condition()) {
                conds.push(expr::lower_expr(cx, &g, &mut Vec::new()));
            }
        }
        let close_line = match arm.expr() {
            Some(b) => {
                super::super::stmt::lower_to_dest(cx, &b, dest, &mut stmts);
                // A block arm is the branch's own block.
                if let Some(Stmt { kind: StmtKind::Block(_), .. }) = stmts.last()
                    && let Some(Stmt { kind: StmtKind::Block(inner), .. }) = stmts.pop()
                {
                    stmts.extend(inner.stmts);
                }
                cx.end_line(b.syntax())
            }
            None => aline,
        };
        cx.pop_scope();
        let cond = conds.into_iter().reduce(|a, b| Expr::bin(a, "&&", b));
        let empty = stmts.iter().all(|s| matches!(&s.kind, StmtKind::Block(b) if b.stmts.is_empty()));
        els = Some(Box::new(match cond {
            Some(cond) => Stmt::new(
                aline,
                StmtKind::If { cond, then: crate::jir::Block { stmts, close_line }, els: els.take() },
            ),
            None if empty && els.is_none() => continue,
            None => Stmt::new(aline, StmtKind::Block(crate::jir::Block { stmts, close_line })),
        }));
    }
    if let Some(e) = els {
        out.push(*e);
    }
    true
}
