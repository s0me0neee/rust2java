// Statements, blocks and statement-context control flow (if / loops / labeled blocks).

use ra_ap_syntax::{AstNode, ast, ast::HasLoopBody, ast::HasName};

use super::{Cx, LoopFrame, expr, macros, pat};
use crate::jir::{Block, Expr, Stmt, StmtKind};
use crate::ty::Ty;

// Where the value of an expression in statement context goes.
#[derive(Clone, Debug)]
pub enum Dest {
    Discard,
    Return,
    Assign(String),
    // Block arm of a switch expression.
    Yield,
}

pub fn lower_block(cx: &mut Cx, b: &ast::BlockExpr, dest: &Dest) -> Block {
    cx.push_scope();
    // Everything declared here lives in this Java block, so a sibling block or a later
    // statement may use the same names again (`n` in two match arms instead of `n`, `n2`).
    let taken = cx.method_names.clone();
    let mut stmts = Vec::new();
    if let Some(list) = b.stmt_list() {
        lower_stmt_list(cx, &list, dest, &mut stmts);
    }
    cx.method_names = taken;
    cx.pop_scope();
    Block {
        stmts,
        close_line: cx.end_line(b.syntax()),
    }
}

pub fn lower_stmt_list(cx: &mut Cx, list: &ast::StmtList, dest: &Dest, out: &mut Vec<Stmt>) {
    for s in list.statements() {
        lower_stmt(cx, &s, out);
    }
    match list.tail_expr() {
        Some(tail) => lower_to_dest(cx, &tail, dest, out),
        None => {
            if let Dest::Assign(_) | Dest::Yield = dest {
                // Block ends in a diverging statement or has unit value; nothing to assign.
            }
        }
    }
}

pub fn lower_stmt(cx: &mut Cx, s: &ast::Stmt, out: &mut Vec<Stmt>) {
    // Lambda names of this statement stop mattering once it is done.
    let saved = cx.stmt_lambda_names.clone();
    lower_stmt_inner(cx, s, out);
    cx.stmt_lambda_names = saved;
}

fn lower_stmt_inner(cx: &mut Cx, s: &ast::Stmt, out: &mut Vec<Stmt>) {
    match s {
        ast::Stmt::LetStmt(l) => lower_let(cx, l, out),
        ast::Stmt::ExprStmt(es) => {
            if let Some(e) = es.expr() {
                lower_to_dest(cx, &e, &Dest::Discard, out);
            }
        }
        ast::Stmt::Item(item) => super::items::lower_local_item(cx, item, out),
    }
}

// A value computed by a call with write-backs: keep it in a local so they can run before it is used.
fn hold_for_write_backs(cx: &mut Cx, v: Expr, ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    if cx.write_backs.is_empty() {
        return v;
    }
    let name = cx.fresh("result");
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: decl_type(ty),
            name: name.clone(),
            init: Some(v),
        },
    ));
    out.append(&mut cx.write_backs);
    Expr::name(name)
}

// `var` for a long generic type, when the initializer alone determines exactly that type. Boxed
// Long/Short/Byte/Float are excluded: `Map.entry(i, 0)` would infer Integer from the literal.
fn var_infers(jty: &str, init: &Expr) -> bool {
    if !jty.contains('<')
        || ["Long", "Short", "Byte", "Float"]
            .iter()
            .any(|t| jty.contains(t))
    {
        return false;
    }
    // Only std types: a list of user values could infer an intersection type, not the interface.
    let std_only = jty
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| w.starts_with(|c: char| c.is_ascii_uppercase()))
        .all(|w| {
            matches!(
                w,
                "List"
                    | "ArrayList"
                    | "Map"
                    | "Entry"
                    | "HashMap"
                    | "TreeMap"
                    | "Set"
                    | "HashSet"
                    | "TreeSet"
                    | "ArrayDeque"
                    | "PriorityQueue"
                    | "Optional"
                    | "Stream"
                    | "Integer"
                    | "Character"
                    | "String"
                    | "Boolean"
                    | "Double"
            )
        });
    let text = crate::emit::render_expr(init);
    std_only
        && !matches!(init, Expr::Lambda { .. } | Expr::Ternary(..))
        && !text.contains("->")
        && !text.contains("<>")
        && !text.contains("empty")
        && !text.contains(" ? ")
        && !text.contains("Optional.empty()")
        && !text.contains("::")
        && !text.starts_with("null")
}

// `[a, b]` or `vec![a, b]` (not the `[x; n]` repeat form).
pub(crate) fn is_list_literal(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::ArrayExpr(a) => a.semicolon_token().is_none(),
        ast::Expr::MacroExpr(m) => m.macro_call().is_some_and(|mc| {
            super::macros::macro_name(&mc) == "vec"
                && mc
                    .token_tree()
                    .is_some_and(|t| !t.syntax().text().to_string().contains(';'))
        }),
        _ => false,
    }
}

// `let (a, b) = (x, y);` as two plain locals, when `x`/`y` don't read `a`/`b` (a swap would).
fn lower_tuple_literal_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    tp: &ast::TuplePat,
    out: &mut Vec<Stmt>,
) -> bool {
    let Some(ast::Expr::TupleExpr(te)) = l.initializer().map(|i| super::strip_parens(&i)) else {
        return false;
    };
    let pats: Vec<ast::Pat> = tp.fields().collect();
    let elems: Vec<ast::Expr> = te.fields().collect();
    if pats.len() != elems.len() || pats.len() < 2 {
        return false;
    }
    let mut names = Vec::new();
    for p in &pats {
        match p {
            ast::Pat::WildcardPat(_) => {}
            ast::Pat::IdentPat(ip)
                if ip.pat().is_none()
                    && ip.ref_token().is_none()
                    && !cx.binding_needs_cell(ip, &cx.pat_ty(p))
                    && !cx.binding_is_builder(ip) =>
            {
                match ip.name() {
                    Some(n) => names.push(n.text().to_string()),
                    None => return false,
                }
            }
            _ => return false,
        }
    }
    let reads_bound = elems.iter().any(|e| {
        e.syntax()
            .descendants()
            .filter_map(ast::PathExpr::cast)
            .any(|pe| names.iter().any(|n| pe.syntax().text() == n.as_str()))
    });
    if reads_bound || elems.iter().any(is_control_flow) {
        return false;
    }
    let line = cx.line(l.syntax());
    for (p, e) in pats.iter().zip(&elems) {
        let ast::Pat::IdentPat(ip) = p else {
            lower_to_dest(cx, e, &Dest::Discard, out);
            continue;
        };
        let ty = cx.pat_ty(p);
        let v = expr::lower_expr_as(cx, e, &ty, out);
        let v = if matches!(ty.strip_refs(), Ty::Option(_)) {
            expr::unwitness_empty(v)
        } else {
            v
        };
        let name = cx.declare(&ip.name().unwrap().text());
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: decl_type(&ty),
                name,
                init: Some(v),
            },
        ));
    }
    true
}

pub fn lower_let(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let line = cx.line(l.syntax());
    let Some(p) = l.pat() else { return };
    if l.let_else().is_some() {
        pat::lower_let_else(cx, l, out);
        return;
    }
    if matches!(p, ast::Pat::WildcardPat(_)) {
        if let Some(init) = l.initializer() {
            lower_to_dest(cx, &init, &Dest::Discard, out);
        }
        return;
    }
    let mut ty = match l.ty() {
        Some(t) => cx.type_ref(&t),
        None => cx.pat_ty(&p),
    };
    if let ast::Pat::IdentPat(ip) = &p
        && cx
            .sema
            .to_def(ip)
            .is_some_and(|local| cx.wide.locals.contains(&local))
    {
        ty = Ty::Int(crate::ty::Int::I64);
    }
    if let ast::Pat::TuplePat(tp) = &p
        && lower_tuple_literal_let(cx, l, tp, out)
    {
        return;
    }
    let ast::Pat::IdentPat(ip) = &p else {
        pat::lower_destructuring_let(cx, l, out);
        return;
    };
    if ip.pat().is_some() {
        pat::lower_destructuring_let(cx, l, out);
        return;
    }
    let rust_name = ip
        .name()
        .map(|n| n.text().to_string())
        .unwrap_or_else(|| "_".into());
    // `let stdin = io::stdin();`: the name just stands for the shared handle.
    if ip.mut_token().is_none()
        && let Some(ast::Expr::CallExpr(c)) = l.initializer()
        && ast::HasArgList::arg_list(&c).is_none_or(|a| a.args().next().is_none())
        && let Ty::Other(handle) = ty.strip_refs()
    {
        let java = match handle.as_str() {
            "Stdin" => Some("STDIN"),
            "Stdout" => Some("System.out"),
            "Stderr" => Some("System.err"),
            _ => None,
        };
        if let Some(java) = java {
            let _ = expr::lower_expr(cx, &ast::Expr::CallExpr(c), out);
            cx.declare_as(&rust_name, java);
            return;
        }
    }
    // `let line = line.unwrap();` on a line-mode stdin loop: the binding already names a String.
    if ip.mut_token().is_none()
        && let Some(init) = l.initializer()
        && let Some(java) = super::std_map::io::phantom_line_let(cx, &init)
    {
        cx.declare_as(&rust_name, &java);
        return;
    }
    if matches!(ty, Ty::Iter(_)) && super::iter::lower_iter_let(cx, l, ip, &rust_name, &ty, out) {
        return;
    }
    if let Some(inner) = super::std_map::ptr::array_cell_pat(cx, ip) {
        lower_handle_let(cx, l, ip, &rust_name, &inner, out);
        return;
    }
    if let Some(inner) = super::std_map::ptr::ref_mut_pat(cx, ip)
        && !l.initializer().is_some_and(|i| option_ref_init(cx, &i))
    {
        if lower_slot_let(cx, l, ip, &inner, out) {
            return;
        }
        lower_ref_let(cx, l, ip, &rust_name, &inner, out);
        return;
    }
    if cx.binding_needs_cell(ip, &ty) {
        lower_cell_let(cx, l, ip, &rust_name, &ty, out);
        return;
    }
    if let Some(init) = l.initializer()
        && !is_control_flow(&init)
        && super::views::binding_is_view(cx, ip, &init)
    {
        let line = cx.line(l.syntax());
        super::views::lower_view_let(cx, ip, &rust_name, &init, line, out);
        return;
    }
    if l.initializer().is_some_and(|i| !is_control_flow(&i)) && cx.binding_is_builder(ip) {
        let line = cx.line(l.syntax());
        let init = expr::lower_expr_as(cx, &l.initializer().unwrap(), &ty, out);
        let name = cx.declare(&rust_name);
        if let Some(local) = cx.sema.to_def(ip) {
            cx.builders.insert(local);
        }
        let args = if expr::is_empty_string_expr(&init) {
            vec![]
        } else {
            vec![init]
        };
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "StringBuilder".into(),
                name,
                init: Some(Expr::New {
                    ty: "StringBuilder".into(),
                    args,
                }),
            },
        ));
        return;
    }
    // `var` can't be initialized with a bare `null`, which is what a `()` value becomes.
    let jty = if ty.is_unit() {
        "Object".into()
    } else {
        decl_type(&ty)
    };
    match l.initializer() {
        None => {
            let name = cx.declare(&rust_name);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: decl_type_explicit(&ty),
                    name,
                    init: None,
                },
            ));
        }
        Some(init) if is_control_flow(&init) => {
            // `let x = if/match/loop/{...}`: declare first, then assign from each branch.
            let name = cx.declare(&rust_name);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: decl_type_explicit(&ty),
                    name: name.clone(),
                    init: None,
                },
            ));
            lower_to_dest(cx, &init, &Dest::Assign(name), out);
        }
        Some(init) => {
            cx.read_only_literal = is_list_literal(&init) && cx.binding_read_only(ip);
            // `let have = *self.balance()?;`: the deref of a copied value lowers to the value.
            let target = match super::strip_parens(&init) {
                ast::Expr::PrefixExpr(pe) if pe.op_kind() == Some(ast::UnaryOp::Deref) => pe
                    .expr()
                    .map(|i| super::strip_parens(&i))
                    .unwrap_or(init.clone()),
                other => other,
            };
            cx.try_let = Some((target.syntax().clone(), rust_name.clone()));
            // The initializer is lowered before declaring so `let x = x + 1` sees the old `x`.
            let e = expr::lower_expr_as(cx, &init, &ty, out);
            cx.try_let = None;
            if let Some(java) = cx.try_let_declared.take()
                && matches!(&e, Expr::Name(n) if *n == java)
            {
                cx.read_only_literal = false;
                return;
            }
            let e = if matches!(ty.strip_refs(), Ty::Option(_)) {
                expr::unwitness_empty(e)
            } else {
                e
            };
            cx.read_only_literal = false;
            let name = cx.declare(&rust_name);
            let in_macro = !ast::SourceFile::can_cast(l.syntax().ancestors().last().map_or(l.syntax().kind(), |r| r.kind()));
            if in_macro {
                cx.macro_locals.insert(name.clone());
            } else {
                cx.user_locals.insert(name.clone());
                let plain = super::escape_keyword(&rust_name);
                if name != plain {
                    cx.renamed_locals.push((name.clone(), plain));
                }
            }
            let jty = if ip.mut_token().is_none() && var_infers(&jty, &e) {
                "var".to_string()
            } else {
                jty
            };
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: jty,
                    name,
                    init: Some(e),
                },
            ));
        }
    }
}

// `let mut n = 0;` that is borrowed `&mut` or mutated in a closure: `int[] n = {0};`, used as `n[0]`.
fn lower_cell_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    ip: &ast::IdentPat,
    rust_name: &str,
    ty: &Ty,
    out: &mut Vec<Stmt>,
) {
    let line = cx.line(l.syntax());
    let elem = ty.strip_refs().java();
    let arr = format!("{elem}[]");
    // Java can't create `Optional<T>[]`; the raw array is the usual workaround.
    let raw_elem = elem.split('<').next().unwrap_or(&elem).to_string();
    match l.initializer() {
        Some(init) if !is_control_flow(&init) => {
            let e = expr::lower_expr_as(cx, &init, ty, out);
            let name = cx.declare(rust_name);
            cx.mark_cell(ip);
            let init = if raw_elem == elem {
                Expr::raw(format!("{{{}}}", render(&e)))
            } else {
                Expr::raw(format!("new {raw_elem}[]{{{}}}", render(&e)))
            };
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: arr,
                    name,
                    init: Some(init),
                },
            ));
        }
        init => {
            let name = cx.declare(rust_name);
            cx.mark_cell(ip);
            let alloc = Expr::raw(format!("new {raw_elem}[1]"));
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: arr,
                    name: name.clone(),
                    init: Some(alloc),
                },
            ));
            if let Some(init) = init {
                lower_to_dest(cx, &init, &Dest::Assign(format!("{name}[0]")), out);
            }
        }
    }
}

// `o.as_mut()` / `o.get_or_insert(..)` / `o.insert(..)`: the binding aliases the Option itself
// (see `expr::option_ref_place`), so it must not become a cell array.
fn option_ref_init(cx: &Cx, init: &ast::Expr) -> bool {
    let ast::Expr::MethodCallExpr(mc) = super::strip_parens(init) else {
        return false;
    };
    let named = mc.name_ref().is_some_and(|n| {
        matches!(
            &*n.text(),
            "as_mut" | "get_or_insert" | "get_or_insert_with" | "insert"
        )
    });
    named
        && mc
            .receiver()
            .is_some_and(|r| matches!(cx.ty(&r).strip_refs(), Ty::Option(_)))
}

// `let c = m.entry(k).or_insert(0);` / `let d = m.get_mut(&k).unwrap();` where Java can't point
// into the value (a number, String, Option): the binding names the map slot, so reads become
// `m.get(k)` and writes `m.put(k, ..)` (see `ElemAliasKind::Map`).
fn lower_slot_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    ip: &ast::IdentPat,
    inner: &Ty,
    out: &mut Vec<Stmt>,
) -> bool {
    use ast::HasArgList;
    if !super::is_mut_ref_cell_ty(inner) {
        return false;
    }
    let (Some(init), Some(local)) = (l.initializer(), cx.sema.to_def(ip)) else {
        return false;
    };
    // `m.get_mut(&k)?` / `m.get_mut(&k).ok_or(e)?`: the `?` check runs first, then the alias.
    let (outer, call, checked) = match super::strip_parens(&init) {
        ast::Expr::TryExpr(t) => {
            let Some(ast::Expr::MethodCallExpr(e)) = t.expr().map(|e| super::strip_parens(&e))
            else {
                return false;
            };
            match e.receiver().map(|r| super::strip_parens(&r)) {
                Some(ast::Expr::MethodCallExpr(call))
                    if matches!(
                        super::iter::method_name(&e).as_str(),
                        "ok_or" | "ok_or_else"
                    ) =>
                {
                    (None, call, true)
                }
                _ => (None, e, true),
            }
        }
        ast::Expr::MethodCallExpr(outer) => {
            let Some(ast::Expr::MethodCallExpr(call)) =
                outer.receiver().map(|r| super::strip_parens(&r))
            else {
                return false;
            };
            (Some(outer), call, false)
        }
        _ => return false,
    };
    let outer_name = outer
        .as_ref()
        .map(super::iter::method_name)
        .unwrap_or_default();
    let call_name = super::iter::method_name(&call);
    let fill = match (call_name.as_str(), outer_name.as_str()) {
        ("entry", "or_insert" | "or_default" | "or_insert_with") => Some(outer_name.as_str()),
        ("get_mut", "unwrap" | "expect") => None,
        ("get_mut", "") if checked => None,
        _ => return false,
    };
    let (Some(map_ast), Some(key_ast)) = (
        call.receiver(),
        call.arg_list().and_then(|a| a.args().next()),
    ) else {
        return false;
    };
    if !matches!(cx.ty(&map_ast).strip_refs(), Ty::Map { .. }) {
        return false;
    }
    let key_ast = match super::strip_parens(&key_ast) {
        ast::Expr::RefExpr(r) => r.expr().unwrap_or(key_ast),
        k => k,
    };
    let arg = outer
        .as_ref()
        .and_then(|o| o.arg_list())
        .and_then(|a| a.args().next());
    let line = cx.line(l.syntax());
    if checked {
        let _ = expr::lower_expr(cx, &init, out);
    }
    let map = super::pat::place(cx, &map_ast, out);
    let key = super::pat::place(cx, &key_ast, out);
    let ensure = match fill {
        Some("or_insert") => {
            let Some(v) = arg else { return false };
            let v = expr::lower_expr_as(cx, &v, inner, out);
            Some(Expr::call(map.clone(), "putIfAbsent", vec![key.clone(), v]))
        }
        Some("or_default") => {
            let v = Expr::raw(super::items::default_value(cx, inner.strip_refs()));
            Some(Expr::call(map.clone(), "putIfAbsent", vec![key.clone(), v]))
        }
        Some(_) => match arg.map(|f| expr::lower_expr(cx, &f, out)) {
            Some(Expr::Lambda { params, body }) if params.is_empty() => {
                let f = Expr::Lambda {
                    params: vec!["unused".into()],
                    body,
                };
                Some(Expr::call(
                    map.clone(),
                    "computeIfAbsent",
                    vec![key.clone(), f],
                ))
            }
            _ => return false,
        },
        None => None,
    };
    if let Some(e) = ensure {
        out.push(Stmt::new(line, StmtKind::Expr(e)));
    }
    cx.elem_aliases.insert(
        local,
        (render(&map), render(&key), super::ElemAliasKind::Map),
    );
    true
}

// `let r = &mut x;` names the array `x` lives in, like a `&mut` parameter.
fn lower_ref_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    ip: &ast::IdentPat,
    rust_name: &str,
    inner: &Ty,
    out: &mut Vec<Stmt>,
) {
    let line = cx.line(l.syntax());
    if l.initializer().is_some_and(|i| expr::is_call_result(&i)) {
        cx.unsupported(
            l.syntax(),
            "binding a `&mut` returned by a call to a value Java can't share",
        );
        return;
    }
    let init = l
        .initializer()
        .map(|init| match super::strip_parens(&init) {
            // Re-borrowing another cell passes the array itself.
            p if cx.cell_path(&p).is_some() => Expr::name(cx.cell_path(&p).unwrap()),
            _ => expr::lower_expr(cx, &init, out),
        });
    let name = cx.declare(rust_name);
    cx.mark_cell(ip);
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: format!("{}[]", inner.java()),
            name,
            init,
        },
    ));
}

// `let c = Cell::new(0);` -> `int[] c = {0};`: the handle is the array, `c.get()` is `c[0]`.
fn lower_handle_let(
    cx: &mut Cx,
    l: &ast::LetStmt,
    ip: &ast::IdentPat,
    rust_name: &str,
    inner: &Ty,
    out: &mut Vec<Stmt>,
) {
    let line = cx.line(l.syntax());
    let arr = format!("{}[]", inner.java());
    match l.initializer() {
        Some(init) if !is_control_flow(&init) => {
            let v = expr::lower_expr(cx, &init, out);
            let v = super::std_map::ptr::as_handle(cx, &init, v, inner);
            let name = cx.declare(rust_name);
            cx.mark_cell_handle(ip);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: arr,
                    name,
                    init: Some(v),
                },
            ));
        }
        init => {
            let name = cx.declare(rust_name);
            cx.mark_cell_handle(ip);
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: arr,
                    name,
                    init: None,
                },
            ));
            if let Some(init) = init {
                cx.unsupported(init.syntax(), "control-flow initializer for a cell handle");
            }
        }
    }
}

pub fn decl_type(ty: &Ty) -> String {
    if ty.is_nameable() && !ty.is_unit() {
        ty.java()
    } else if let Some(t) = formatter_fn_type(ty) {
        t
    } else {
        "var".into()
    }
}

// A local closure over a `Formatter` (`let wrap = |e, f: &mut Formatter| -> fmt::Result`): Java
// can't declare a lambda with `var`; the Formatter is the StringBuilder and fmt::Result is Void.
fn formatter_fn_type(ty: &Ty) -> Option<String> {
    let Ty::Fn { params, ret } = ty.strip_refs() else {
        return None;
    };
    let sb = || Ty::Adt {
        name: "Formatter".into(),
        java: "StringBuilder".into(),
        args: vec![],
    };
    let mut saw = false;
    let params: Vec<Ty> = params
        .iter()
        .map(|p| match p.strip_refs() {
            Ty::Other(s) if s.contains("Formatter") => {
                saw = true;
                sb()
            }
            t => t.clone(),
        })
        .collect();
    let ret = match ret.strip_refs() {
        Ty::Result(ok, err) if ok.is_unit() && matches!(**err, Ty::Other(_)) => Ty::Adt {
            name: "Void".into(),
            java: "Void".into(),
            args: vec![],
        },
        t => t.clone(),
    };
    let f = Ty::Fn {
        params,
        ret: Box::new(ret),
    };
    (saw && f.is_nameable()).then(|| f.java())
}

// For declarations without initializer, where `var` is not allowed.
pub fn decl_type_explicit(ty: &Ty) -> String {
    // Iterator items may contain opaque I/O errors, but Stream<Result<..., Object>> is nameable.
    // Likewise `Result<T, Object>` for a std error payload (`ParseIntError`).
    let std_error_result = matches!(ty.strip_refs(), Ty::Result(ok, err)
        if ok.is_nameable() && matches!(**err, Ty::Other(_)));
    if matches!(ty.strip_refs(), Ty::Iter(_))
        || std_error_result
        || (ty.is_nameable() && !ty.is_unit())
    {
        ty.java()
    } else {
        "Object".into()
    }
}

pub fn is_control_flow(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::IfExpr(i) => !expr::is_simple_if(i),
        ast::Expr::MatchExpr(_)
        | ast::Expr::LoopExpr(_)
        | ast::Expr::WhileExpr(_)
        | ast::Expr::ForExpr(_) => true,
        ast::Expr::BlockExpr(b) => {
            b.stmt_list()
                .is_some_and(|l| l.statements().next().is_some())
                || b.label().is_some()
        }
        ast::Expr::MacroExpr(m) => macros::is_diverging(m),
        _ => false,
    }
}

// Lowers `e` in statement context, sending its value to `dest`.
pub fn lower_to_dest(cx: &mut Cx, e: &ast::Expr, dest: &Dest, out: &mut Vec<Stmt>) {
    let line = cx.line(e.syntax());
    // Simple mode: eligibility only lets an `Err` value reach a dest on its way to the return.
    if crate::mode::throws()
        && let Some(payload) = super::throws::err_payload(cx, e)
    {
        super::throws::push_throw(cx, &payload, line, out);
        return;
    }
    match e {
        ast::Expr::BlockExpr(b) => {
            if b.unsafe_token().is_some() || b.async_token().is_some() || b.const_token().is_some()
            {
                let u = cx.unsupported(b.syntax(), "unsafe/async/const block");
                out.push(Stmt::new(line, StmtKind::Expr(u)));
                return;
            }
            if let Some(label) = b.label() {
                let rust_label = label.lifetime().map(|l| l.text().to_string());
                let java_label = rust_label.as_deref().map(java_label_name);
                cx.loops.push(LoopFrame {
                    rust_label,
                    java_label: java_label.clone(),
                    value_dest: break_dest(dest),
                    switch_depth: cx.switch_depth,
                    is_block: true,
                });
                let body = lower_block(cx, b, dest);
                cx.loops.pop();
                out.push(Stmt::new(
                    line,
                    StmtKind::Labeled {
                        label: java_label.unwrap(),
                        body,
                    },
                ));
                return;
            }
            let body = lower_block(cx, b, dest);
            out.push(Stmt::new(line, StmtKind::Block(body)));
        }
        ast::Expr::IfExpr(i) => lower_if(cx, i, dest, out),
        ast::Expr::MatchExpr(m) => pat::lower_match_stmt(cx, m, dest, out),
        ast::Expr::LoopExpr(l) => {
            push_loop(cx, l.label(), dest);
            let body = l
                .loop_body()
                .map(|b| lower_block(cx, &b, &Dest::Discard))
                .unwrap_or_default();
            let frame = cx.loops.pop().unwrap();
            out.push(Stmt::new(
                line,
                StmtKind::While {
                    label: frame.java_label,
                    cond: Expr::raw("true"),
                    body,
                },
            ));
        }
        ast::Expr::WhileExpr(w) => lower_while(cx, w, out),
        ast::Expr::ForExpr(f) => super::iter::lower_for(cx, f, out),
        ast::Expr::ReturnExpr(r) => match r.expr() {
            Some(v) => lower_to_dest(cx, &v, &Dest::Return, out),
            None => out.push(Stmt::new(line, StmtKind::Return(None))),
        },
        ast::Expr::BreakExpr(b) => lower_break(cx, b, out),
        ast::Expr::ContinueExpr(c) => {
            let label = c.lifetime().map(|l| java_label_name(l.text().as_ref()));
            out.push(Stmt::new(line, StmtKind::Continue(label)));
        }
        ast::Expr::MacroExpr(m) if macros::is_diverging(m) => {
            let e = macros::lower_macro_expr(cx, m, out);
            out.push(Stmt::new(line, StmtKind::Throw(e)));
        }
        _ => {
            let ty = cx.ty(e);
            // A call statement may pass `&mut field` through a temporary cell (`expr::write_back_cell`).
            let call = matches!(
                super::strip_parens(e),
                ast::Expr::CallExpr(_) | ast::Expr::MethodCallExpr(_)
            );
            let saved = std::mem::replace(
                &mut cx.write_back_call,
                call.then(|| super::strip_parens(e).syntax().clone()),
            );
            let saved_backs = std::mem::take(&mut cx.write_backs);
            lower_value_to_dest(cx, e, &ty, dest, line, out);
            cx.write_back_call = saved;
            cx.write_backs = saved_backs;
        }
    }
}

// The value of `e` delivered to `dest`, followed by any write-backs its call needed.
fn lower_value_to_dest(
    cx: &mut Cx,
    e: &ast::Expr,
    ty: &Ty,
    dest: &Dest,
    line: u32,
    out: &mut Vec<Stmt>,
) {
    {
        {
            match dest {
                Dest::Discard => {
                    let v = expr::lower_expr(cx, e, out);
                    push_discard(out, line, v);
                    out.append(&mut cx.write_backs);
                }
                Dest::Return => {
                    let unit_fn = cx.ret_ty.as_ref().is_none_or(Ty::is_unit);
                    if unit_fn {
                        let v = expr::lower_expr(cx, e, out);
                        push_discard(out, line, v);
                        out.append(&mut cx.write_backs);
                        if !is_tail_of_fn(e) {
                            out.push(Stmt::new(line, StmtKind::Return(None)));
                        }
                    } else {
                        let ret = cx.ret_ty.clone().unwrap();
                        let v = expr::lower_expr_as(cx, e, &ret, out);
                        // A lambda's return type comes from its returns; a bare empty is Object.
                        let v = match (&v, ret.strip_refs()) {
                            (Expr::Raw(s), Ty::Option(t))
                                if cx.lambda_depth > 0
                                    && s == "Optional.empty()"
                                    && t.is_nameable() =>
                            {
                                Expr::raw(format!("Optional.<{}>empty()", t.boxed()))
                            }
                            _ => v,
                        };
                        // A `Cell<i32>` return value is the one-element array, not its contents.
                        let v = match super::std_map::ptr::is_array(cx, e) {
                            Some(inner) => super::std_map::ptr::as_handle(cx, e, v, &inner),
                            None => v,
                        };
                        let v = hold_for_write_backs(cx, v, &ret, line, out);
                        out.push(Stmt::new(line, StmtKind::Return(Some(v))));
                    }
                }
                Dest::Assign(var) => {
                    if ty.is_unit() {
                        let v = expr::lower_expr(cx, e, out);
                        push_discard(out, line, v);
                    } else {
                        let v = expr::lower_expr(cx, e, out);
                        out.push(Stmt::new(
                            line,
                            StmtKind::Expr(Expr::Assign(
                                Box::new(Expr::name(var.clone())),
                                "=",
                                Box::new(v),
                            )),
                        ));
                    }
                    out.append(&mut cx.write_backs);
                }
                Dest::Yield => {
                    let v = expr::lower_expr(cx, e, out);
                    let v = hold_for_write_backs(cx, v, ty, line, out);
                    out.push(Stmt::new(
                        line,
                        StmtKind::Raw(format!("yield {};", render(&v))),
                    ));
                }
            }
        }
    }
}

fn is_tail_of_fn(e: &ast::Expr) -> bool {
    // Tail expression of the fn body itself: no explicit `return;` needed.
    e.syntax()
        .parent()
        .and_then(ast::StmtList::cast)
        .and_then(|l| l.syntax().parent())
        .and_then(ast::BlockExpr::cast)
        .and_then(|b| b.syntax().parent())
        .is_some_and(|p| ast::Fn::can_cast(p.kind()))
}

pub fn render(e: &Expr) -> String {
    crate::emit::render_expr(e)
}

pub fn push_discard(out: &mut Vec<Stmt>, line: u32, v: Expr) {
    if v.is_statement_expr() {
        out.push(Stmt::new(line, StmtKind::Expr(v)));
    } else if !matches!(&v, Expr::Name(_) | Expr::Raw(_))
        || matches!(&v, Expr::Raw(s) if s.contains('('))
    {
        // Java rejects non-statement expressions; keep side effects via a throwaway local.
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: "var".into(),
                name: "_unused".into(),
                init: Some(v),
            },
        ));
    }
}

pub fn java_label_name(rust: &str) -> String {
    rust.trim_start_matches('\'').to_string()
}

// Pushes a loop frame; pop it after lowering the body to read the final label.
fn push_loop(cx: &mut Cx, label: Option<ast::Label>, dest: &Dest) {
    let rust_label = label
        .and_then(|l| l.lifetime())
        .map(|l| l.text().to_string());
    let frame = LoopFrame {
        java_label: rust_label.as_deref().map(java_label_name),
        rust_label,
        value_dest: break_dest(dest),
        switch_depth: cx.switch_depth,
        is_block: false,
    };
    cx.loops.push(frame);
}

fn break_dest(dest: &Dest) -> Option<Dest> {
    match dest {
        Dest::Discard => None,
        d => Some(d.clone()),
    }
}

fn lower_break(cx: &mut Cx, b: &ast::BreakExpr, out: &mut Vec<Stmt>) {
    let line = cx.line(b.syntax());
    let rust_label = b.lifetime().map(|l| l.text().to_string());
    let idx = match &rust_label {
        Some(l) => cx
            .loops
            .iter()
            .rposition(|f| f.rust_label.as_deref() == Some(l)),
        None => cx.loops.iter().rposition(|f| !f.is_block),
    };
    if let Some(v) = b.expr() {
        match idx.and_then(|i| cx.loops[i].value_dest.clone()) {
            // `return v;` / `yield v;` already leave the loop.
            Some(d @ (Dest::Return | Dest::Yield)) => return lower_to_dest(cx, &v, &d, out),
            Some(d) => lower_to_dest(cx, &v, &d, out),
            None => lower_to_dest(cx, &v, &Dest::Discard, out),
        }
    }
    let mut label = rust_label.as_deref().map(java_label_name);
    // An unlabeled `break` inside a Java switch statement would only leave the switch.
    if let Some(i) = idx
        && label.is_none()
        && cx.switch_depth > cx.loops[i].switch_depth
    {
        if cx.loops[i].java_label.is_none() {
            let l = cx.fresh("loop");
            cx.loops[i].java_label = Some(l);
        }
        label = cx.loops[i].java_label.clone();
    }
    out.push(Stmt::new(line, StmtKind::Break(label)));
}

pub fn lower_if(cx: &mut Cx, i: &ast::IfExpr, dest: &Dest, out: &mut Vec<Stmt>) {
    let line = cx.line(i.syntax());
    let Some(cond) = i.condition() else { return };
    if crate::mode::throws()
        && let ast::Expr::LetExpr(le) = &cond
        && le
            .expr()
            .is_some_and(|s| matches!(cx.ty(&s).strip_refs(), Ty::Result(..)))
        && super::throws::lower_if_let(cx, i, dest, out)
    {
        return;
    }
    cx.push_scope();
    let mut binds = Vec::new();
    let c = pat::lower_condition(cx, &cond, out, &mut binds);
    let mut then = i
        .then_branch()
        .map(|b| lower_block(cx, &b, dest))
        .unwrap_or_default();
    cx.pop_scope();
    if !binds.is_empty() {
        binds.append(&mut then.stmts);
        then.stmts = binds;
    }
    let els = match i.else_branch() {
        None => None,
        Some(ast::ElseBranch::Block(b)) => {
            let l = cx.line(b.syntax());
            Some(Box::new(Stmt::new(
                l,
                StmtKind::Block(lower_block(cx, &b, dest)),
            )))
        }
        Some(ast::ElseBranch::IfExpr(ei)) => {
            let mut sub = Vec::new();
            lower_if(cx, &ei, dest, &mut sub);
            match sub.len() {
                1 => Some(Box::new(sub.pop().unwrap())),
                _ => {
                    // The else-if needed hoisted statements: wrap them in a block.
                    let l = cx.line(ei.syntax());
                    Some(Box::new(Stmt::new(
                        l,
                        StmtKind::Block(Block {
                            stmts: sub,
                            close_line: cx.end_line(ei.syntax()),
                        }),
                    )))
                }
            }
        }
    };
    out.push(Stmt::new(line, StmtKind::If { cond: c, then, els }));
}

fn lower_while(cx: &mut Cx, w: &ast::WhileExpr, out: &mut Vec<Stmt>) {
    let line = cx.line(w.syntax());
    push_loop(cx, w.label(), &Dest::Discard);
    cx.push_scope();
    let mut pre = Vec::new();
    let mut binds = Vec::new();
    let cond = match w.condition() {
        Some(c) => pat::lower_condition(cx, &c, &mut pre, &mut binds),
        None => Expr::raw("true"),
    };
    let mut body = w
        .loop_body()
        .map(|b| lower_block(cx, &b, &Dest::Discard))
        .unwrap_or_default();
    cx.pop_scope();
    let frame = cx.loops.pop().unwrap();
    // Simple mode: `T x = q.poll(); x != null` becomes `T x; while ((x = q.poll()) != null)`.
    if crate::mode::simple()
        && let [Stmt {
            kind:
                StmtKind::Local {
                    ty,
                    name,
                    init: Some(next),
                },
            ..
        }] = pre.as_slice()
        && matches!(&cond, Expr::Binary(l, "!=", r)
            if matches!(l.as_ref(), Expr::Name(n) if n == name)
                && matches!(r.as_ref(), Expr::Raw(z) if z == "null"))
    {
        out.push(Stmt::new(
            line,
            StmtKind::Local {
                ty: ty.clone(),
                name: name.clone(),
                init: None,
            },
        ));
        let assign = Expr::Assign(Box::new(Expr::name(name)), "=", Box::new(next.clone()));
        binds.append(&mut body.stmts);
        body.stmts = binds;
        out.push(Stmt::new(
            line,
            StmtKind::While {
                label: frame.java_label,
                cond: Expr::bin(assign, "!=", Expr::raw("null")),
                body,
            },
        ));
        return;
    }
    if pre.is_empty() {
        binds.append(&mut body.stmts);
        body.stmts = binds;
        out.push(Stmt::new(
            line,
            StmtKind::While {
                label: frame.java_label,
                cond,
                body,
            },
        ));
    } else {
        // Condition needs hoisted statements: `while (true) { pre; if (!cond) break; ... }`.
        let mut stmts = pre;
        stmts.push(Stmt::new(
            line,
            StmtKind::If {
                cond: Expr::Unary("!", Box::new(cond)),
                then: Block {
                    stmts: vec![Stmt::new(line, StmtKind::Break(None))],
                    close_line: line,
                },
                els: None,
            },
        ));
        stmts.append(&mut binds);
        stmts.append(&mut body.stmts);
        out.push(Stmt::new(
            line,
            StmtKind::While {
                label: frame.java_label,
                cond: Expr::raw("true"),
                body: Block {
                    stmts,
                    close_line: body.close_line,
                },
            },
        ));
    }
}
