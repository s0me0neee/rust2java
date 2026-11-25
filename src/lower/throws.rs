// Simple mode: `Result` as exceptions (DESIGN §22.3). A function returning `Result<T, E>` returns
// `T` and throws on `Err`; `?`, `unwrap` and `expect` disappear; `match`/`if let` on a Result
// become try/catch. A file using Result any other way keeps exact-mode Result values.

use ra_ap_hir::{ModuleDef, PathResolution};
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast, ast::HasArgList, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, expr, strip_parens};
use crate::jir::{Block, Expr, Stmt, StmtKind};
use crate::ty::Ty;

// `e.unwrap()`, `e.expect(..)`, or in throws mode `e?`: the node taking the Ok value out of `e`.
pub fn unwrapper(e: &SyntaxNode) -> Option<SyntaxNode> {
    let p = e.parent()?;
    if let Some(m) = ast::MethodCallExpr::cast(p.clone()) {
        let is_recv = m
            .receiver()
            .is_some_and(|r| r.syntax().text_range() == e.text_range());
        let name = m.name_ref().map(|n| n.text().to_string()).unwrap_or_default();
        return (is_recv && matches!(name.as_str(), "unwrap" | "expect")).then_some(p);
    }
    (crate::mode::throws() && ast::TryExpr::can_cast(p.kind())).then_some(p)
}

// Whether every Result in the file can be erased; otherwise the first use that can't, and why.
// Runs with `mode::throws()` already on, so the stdin-lines analysis counts `?` as an unwrap.
pub fn eligible(cx: &Cx, root: &SyntaxNode) -> Result<(), (SyntaxNode, &'static str)> {
    // Macro expansions too: `println!("{:?}", f())` prints a Result.
    for node in cx.expanded_descendants(root) {
        if in_fmt_impl(&node) {
            continue;
        }
        if let Some(t) = ast::Type::cast(node.clone()) {
            if has_result(&cx.type_ref(&t)) && !allowed_type(cx, &t) {
                return Err((node, "a Result type outside a function's return type"));
            }
            continue;
        }
        let Some(e) = ast::Expr::cast(node.clone()) else {
            continue;
        };
        // A `binary_search` read only for its position never becomes a Result.
        if super::std_map::coll::plain_search_use(cx, &e) {
            continue;
        }
        let ty = cx.ty(&e);
        // The callee of a call (`Ok` in `Ok(x)`, `f` in `f()`) has a function type.
        let callee = node
            .parent()
            .and_then(ast::CallExpr::cast)
            .and_then(|c| c.expr())
            .is_some_and(|f| f.syntax() == &node);
        // `r.and_then(f)` passes `f` by name; it becomes a call `f(r)`.
        let applied = node
            .parent()
            .filter(|a| ast::ArgList::can_cast(a.kind()))
            .and_then(|a| a.parent())
            .and_then(ast::MethodCallExpr::cast)
            .is_some_and(|m| {
                matches!(method_name(&m).as_str(), "map" | "and_then")
                    && m.receiver().is_some_and(|r| matches!(cx.ty(&r).strip_refs(), Ty::Result(..)))
            });
        if !callee && !applied && nested_result(&ty, false) {
            return Err((node, "a Result inside another type"));
        }
        if let ast::Expr::MethodCallExpr(m) = &e
            && method_name(m) == "lines"
            && matches!(ty.strip_refs(), Ty::Iter(t) if matches!(**t, Ty::Result(..)))
            && !super::std_map::io::plain_lines(cx, m)
        {
            return Err((node, "stdin lines used as Results"));
        }
        if !matches!(ty.strip_refs(), Ty::Result(..)) {
            continue;
        }
        if !producer_ok(cx, &e) {
            return Err((node, "a Result from a call that can't become a throwing call"));
        }
        if !context_ok(cx, &e) {
            return Err((node, "a Result value that is stored or passed on"));
        }
        if !error_reads_ok(cx, &e) {
            return Err((node, "an error value of this type read after it is caught"));
        }
    }
    Ok(())
}

// `impl Display`/`Debug` bodies keep their own `fmt::Result` handling (`in_display`).
fn in_fmt_impl(node: &SyntaxNode) -> bool {
    node.ancestors()
        .filter_map(ast::Fn::cast)
        .any(|f| f.name().is_some_and(|n| n.text() == "fmt"))
}

fn has_result(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Result(..)) || nested_result(ty, false)
}

// A Result inside another type. Stdin lines (`Result<String, io::Error>` items) are allowed:
// their uses are checked like any other Result.
fn nested_result(ty: &Ty, inner: bool) -> bool {
    let here = |t: &Ty| nested_result(t, true);
    match ty {
        Ty::Result(ok, err) => {
            (inner && !(ok.is_str() && is_io_error(err))) || here(ok) || here(err)
        }
        Ty::Vec(t) | Ty::Set { t, .. } | Ty::Deque(t) | Ty::Heap(t) | Ty::Option(t) | Ty::Iter(t) => {
            here(t)
        }
        Ty::Map { k, v, .. } => here(k) || here(v),
        Ty::Tuple(ts) | Ty::Adt { args: ts, .. } => ts.iter().any(here),
        Ty::Ref { inner: t, .. } => nested_result(t, inner),
        Ty::Fn { params, ret } => params.iter().any(here) || here(ret),
        _ => false,
    }
}

fn is_io_error(ty: &Ty) -> bool {
    matches!(ty.strip_refs(), Ty::Other(n) if n == "Error" || n.ends_with("io::Error"))
}

// A written type mentioning Result: only a function's return type (or a type alias for one).
fn allowed_type(cx: &Cx, t: &ast::Type) -> bool {
    for a in t.syntax().ancestors() {
        match a.kind() {
            SyntaxKind::TYPE_ALIAS => return true,
            SyntaxKind::RET_TYPE => {
                let ret = ast::RetType::cast(a.clone()).and_then(|r| r.ty());
                return a.parent().is_some_and(|p| ast::Fn::can_cast(p.kind()))
                    && ret.is_some_and(|r| !nested_result(&cx.type_ref(&r), false));
            }
            _ => {}
        }
    }
    false
}

fn method_name(m: &ast::MethodCallExpr) -> String {
    m.name_ref().map(|n| n.text().to_string()).unwrap_or_default()
}

fn is_local_fn(cx: &Cx, f: ra_ap_hir::Function) -> bool {
    f.module(cx.db).krate(cx.db).origin(cx.db).is_local()
}

// `Ok`/`Err` when `c` constructs a std Result.
fn result_ctor(cx: &Cx, c: &ast::CallExpr) -> Option<&'static str> {
    let ast::Expr::PathExpr(pe) = c.expr()? else {
        return None;
    };
    let PathResolution::Def(ModuleDef::EnumVariant(v)) = cx.sema.resolve_path(&pe.path()?)? else {
        return None;
    };
    let e = v.parent_enum(cx.db);
    if e.name(cx.db).as_str() != "Result" || e.module(cx.db).krate(cx.db).origin(cx.db).is_local()
    {
        return None;
    }
    match v.name(cx.db).as_str() {
        "Ok" => Some("Ok"),
        "Err" => Some("Err"),
        _ => None,
    }
}

// `Err(x)` (through parentheses) -> `x`.
pub fn err_payload(cx: &Cx, e: &ast::Expr) -> Option<ast::Expr> {
    let ast::Expr::CallExpr(c) = strip_parens(e) else {
        return None;
    };
    if result_ctor(cx, &c)? != "Err" {
        return None;
    }
    c.arg_list()?.args().next()
}

// Expressions whose lowering already yields the Ok value (or throws) in throws mode.
fn producer_ok(cx: &Cx, e: &ast::Expr) -> bool {
    match strip_parens(e) {
        ast::Expr::IfExpr(_) | ast::Expr::MatchExpr(_) | ast::Expr::BlockExpr(_) => true,
        ast::Expr::CallExpr(c) => {
            if let Some(ctor) = result_ctor(cx, &c) {
                return ctor == "Ok" || err_context_ok(&c);
            }
            let Some(ast::Expr::PathExpr(pe)) = c.expr() else {
                return false;
            };
            match pe.path().and_then(|p| cx.sema.resolve_path(&p)) {
                Some(PathResolution::Def(ModuleDef::Function(f))) => {
                    is_local_fn(cx, f)
                        || matches!(f.name(cx.db).as_str(), "from_str_radix" | "from_utf8")
                }
                _ => false,
            }
        }
        ast::Expr::MethodCallExpr(m) => {
            if cx
                .sema
                .resolve_method_call(&m)
                .is_some_and(|f| is_local_fn(cx, f))
            {
                return true;
            }
            let ok = match cx.ty(&ast::Expr::MethodCallExpr(m.clone())).strip_refs() {
                Ty::Result(ok, _) => (**ok).clone(),
                _ => return false,
            };
            match method_name(&m).as_str() {
                "parse" => matches!(
                    ok.strip_refs(),
                    Ty::Int(_) | Ty::F32 | Ty::F64 | Ty::Bool | Ty::Str | Ty::Adt { .. }
                ),
                // `write_fmt` is a `write!` expansion; the macro call is checked in the file.
                "read_line" | "read_to_string" | "read" | "flush" | "write_all" | "write_fmt"
                | "write_str" | "write_char" | "map_err" | "ok_or"
                | "ok_or_else" | "map" | "and_then" => true,
                "unwrap" | "expect" => super::std_map::io::phantom_line_result(cx, e),
                _ => false,
            }
        }
        ast::Expr::MacroExpr(m) => m.macro_call().is_some_and(|mc| {
            matches!(super::macros::macro_name(&mc).as_str(), "write" | "writeln")
        }),
        e @ ast::Expr::PathExpr(_) => super::std_map::io::phantom_line_result(cx, &e),
        _ => false,
    }
}

// `Err(x)` only where a `throw` can stand: returned from the function, or `Err(x)?`.
fn err_context_ok(c: &ast::CallExpr) -> bool {
    matches!(
        walk_up(c.syntax()),
        Some(Context::Return | Context::Try)
    )
}

enum Context {
    Return,
    Try,
    // Unwrapping method (`unwrap`, `ok`, ...), match or if-let scrutinee, or a discarded value.
    Consumed,
}

fn context_ok(cx: &Cx, e: &ast::Expr) -> bool {
    match walk_up(e.syntax()) {
        None => false,
        Some(Context::Try) => same_error(cx, e),
        Some(_) => true,
    }
}

// `?` converting between error types via `From`: the exception would keep the original type and
// slip past a `catch` of the converted one, so only conversions into strings and std errors pass.
// A std parse or I/O error converted `From` into a user error is caught and rethrown converted.
fn same_error(cx: &Cx, e: &ast::Expr) -> bool {
    let Ty::Result(_, from) = cx.ty(e) else {
        return true;
    };
    match fn_error(cx, e.syntax()) {
        // Every error travels as a RuntimeException, which is what a `dyn Error` is caught as.
        Some(Ty::Other(n)) if n.contains("dyn Error") || n.contains("dyn std::error::Error") => true,
        Some(to) => {
            let user = user_error(&from).is_some() || user_error(&to).is_some();
            !user || from.strip_refs() == to.strip_refs() || from_conversion(cx, e.syntax(), &from, &to)
        }
        None => true,
    }
}

// The error type of the function `node` is in, when it returns a Result.
fn fn_error(cx: &Cx, node: &SyntaxNode) -> Option<Ty> {
    let f = node.ancestors().find_map(ast::Fn::cast)?;
    match cx.convert(&cx.sema.to_def(&f)?.ret_type(cx.db)) {
        Ty::Result(_, to) => Some(*to),
        _ => None,
    }
}

// `from` is a std error with its own exception class, and the file has `impl From<from> for to`.
fn from_conversion(cx: &Cx, node: &SyntaxNode, from: &Ty, to: &Ty) -> bool {
    let std_exception = match from.strip_refs() {
        Ty::Other(n) => crate::ty::is_parse_error_name(n) || is_io_error(from),
        _ => false,
    };
    let Some(to_name) = user_error(to) else { return false };
    if !std_exception {
        return false;
    }
    let Some(root) = node.ancestors().last() else { return false };
    root.descendants().filter_map(ast::Impl::cast).any(|imp| {
        let is_from = imp.trait_().is_some_and(|t| {
            let text = t.syntax().text().to_string();
            text.starts_with("From<") || text.contains("::From<")
        });
        let for_to = imp.self_ty().is_some_and(|t| cx.type_ref(&t).strip_refs().java() == to_name);
        let from_arg = imp
            .trait_()
            .and_then(|t| t.syntax().descendants().find_map(ast::GenericArgList::cast))
            .and_then(|g| g.generic_args().next())
            .and_then(|a| match a {
                ast::GenericArg::TypeArg(t) => t.ty(),
                _ => None,
            })
            .is_some_and(|t| cx.type_ref(&t).strip_refs() == from.strip_refs());
        is_from && for_to && from_arg
    })
}

// `e?` whose error converts `From` a std error into the function's user error:
// `try { v = e; } catch (NumberFormatException ex) { throw new AppErrorException(AppError.from(ex)); }`.
pub fn try_converted(cx: &mut Cx, t: &ast::TryExpr, inner: &ast::Expr, line: u32, out: &mut Vec<Stmt>) -> Option<Expr> {
    let Ty::Result(ok, from) = cx.ty(inner) else { return None };
    let to = fn_error(cx, t.syntax())?;
    if from.strip_refs() == to.strip_refs() || !from_conversion(cx, t.syntax(), &from, &to) {
        return None;
    }
    let to_java = to.strip_refs().java();
    let class = exception_class(cx, &from);
    let var = (!ok.is_unit()).then(|| cx.take_let_target(t.syntax()).unwrap_or_else(|| cx.fresh("value")));
    if let Some(v) = &var {
        out.push(Stmt::new(
            line,
            StmtKind::Local { ty: stmt::decl_type_explicit(&ok), name: v.clone(), init: None },
        ));
    }
    let taken = cx.method_names.clone();
    cx.push_scope();
    let mut body = Vec::new();
    let value = expr::lower_expr(cx, inner, &mut body);
    match &var {
        Some(v) => body.push(Stmt::new(
            line,
            StmtKind::Expr(Expr::Assign(Box::new(Expr::name(v)), "=", Box::new(value))),
        )),
        None => stmt::push_discard(&mut body, line, value),
    }
    cx.pop_scope();
    cx.method_names = taken;
    let ex = cx.fresh_param("e");
    let converted = Expr::call(Expr::raw(to_java), "from", vec![Expr::name(&ex)]);
    let thrown = exception_of(cx, converted, &to);
    out.push(Stmt::new(
        line,
        StmtKind::Try {
            body: Block { stmts: body, close_line: line },
            decl: format!("{class} {ex}"),
            handler: Block { stmts: vec![Stmt::new(line, StmtKind::Throw(thrown))], close_line: line },
        },
    ));
    cx.release_param(&ex);
    Some(var.map_or(Expr::raw("null"), Expr::name))
}

// Where a Result-typed expression's value ends up, looking through blocks and branches.
fn walk_up(node: &SyntaxNode) -> Option<Context> {
    let mut cur = node.clone();
    loop {
        // The root of a macro expansion (`write!`): the macro call itself is checked in the file.
        let Some(parent) = cur.parent() else {
            return (cur.kind() != SyntaxKind::SOURCE_FILE).then_some(Context::Consumed);
        };
        if matches!(parent.kind(), SyntaxKind::MACRO_STMTS | SyntaxKind::MACRO_EXPR) {
            return Some(Context::Consumed);
        }
        match parent.kind() {
            SyntaxKind::PAREN_EXPR => cur = parent,
            SyntaxKind::STMT_LIST => {
                let list = ast::StmtList::cast(parent.clone())?;
                if list.tail_expr()?.syntax() != &cur {
                    return None;
                }
                let block = parent.parent()?;
                match block.parent() {
                    Some(p) if ast::Fn::can_cast(p.kind()) => return Some(Context::Return),
                    Some(p) if ast::ClosureExpr::can_cast(p.kind()) => return None,
                    _ => cur = block,
                }
            }
            SyntaxKind::IF_EXPR => {
                let i = ast::IfExpr::cast(parent.clone())?;
                if i.condition().is_some_and(|c| c.syntax() == &cur) {
                    return None;
                }
                cur = parent;
            }
            SyntaxKind::MATCH_ARM => {
                let arm = ast::MatchArm::cast(parent.clone())?;
                if arm.expr().is_none_or(|x| x.syntax() != &cur) {
                    return None;
                }
                cur = parent.parent()?.parent()?;
            }
            // Only a `match` written in the file: `matches!(r, Ok(_))` lowers as a pattern test.
            SyntaxKind::MATCH_EXPR => {
                let in_file = parent
                    .ancestors()
                    .last()
                    .is_some_and(|r| r.kind() == SyntaxKind::SOURCE_FILE);
                let m = ast::MatchExpr::cast(parent)?;
                return (in_file && match_on_result_ok(&m)).then_some(Context::Consumed);
            }
            SyntaxKind::LET_EXPR => {
                let le = ast::LetExpr::cast(parent.clone())?;
                let in_if = parent
                    .parent()
                    .and_then(ast::IfExpr::cast)
                    .and_then(|i| i.condition())
                    .is_some_and(|c| c.syntax() == &parent);
                return (in_if && le.pat().is_some_and(|p| simple_result_pat(&p).is_some()))
                    .then_some(Context::Consumed);
            }
            SyntaxKind::RETURN_EXPR => {
                let owner = parent
                    .ancestors()
                    .find(|a| ast::Fn::can_cast(a.kind()) || ast::ClosureExpr::can_cast(a.kind()))?;
                return ast::Fn::can_cast(owner.kind()).then_some(Context::Return);
            }
            SyntaxKind::TRY_EXPR => return Some(Context::Try),
            SyntaxKind::FN => return Some(Context::Return),
            SyntaxKind::METHOD_CALL_EXPR => {
                let m = ast::MethodCallExpr::cast(parent)?;
                let is_recv = m.receiver().is_some_and(|r| r.syntax() == &cur);
                let consuming = matches!(
                    method_name(&m).as_str(),
                    "unwrap"
                        | "expect"
                        | "unwrap_or"
                        | "unwrap_or_default"
                        | "unwrap_or_else"
                        | "ok"
                        | "is_ok"
                        | "is_err"
                        | "map_err"
                        | "map"
                        | "and_then"
                );
                return (is_recv && consuming).then_some(Context::Consumed);
            }
            SyntaxKind::EXPR_STMT => return Some(Context::Consumed),
            SyntaxKind::LET_STMT => {
                let l = ast::LetStmt::cast(parent)?;
                return matches!(l.pat(), Some(ast::Pat::WildcardPat(_))).then_some(Context::Consumed);
            }
            _ => return None,
        }
    }
}

// A caught error can be read back only when its exception carries it: a message string or a
// std error (the exception itself). `unwrap_or_else(|e| ..)` reading `e` isn't supported.
fn error_reads_ok(cx: &Cx, e: &ast::Expr) -> bool {
    let Ty::Result(_, err) = cx.ty(e) else {
        return true;
    };
    let readable = match err.strip_refs() {
        Ty::Str => true,
        Ty::Other(n) => n.contains("Error"),
        t => user_error(t).is_some(),
    };
    let Some(parent) = e.syntax().parent() else {
        return true;
    };
    let binds_err = |p: Option<ast::Pat>| {
        p.and_then(|p| simple_result_pat(&p))
            .is_some_and(|(which, b)| which == "Err" && b.is_some())
    };
    if let Some(m) = ast::MatchExpr::cast(parent.clone()) {
        let reads = m
            .match_arm_list()
            .is_some_and(|l| l.arms().any(|a| binds_err(a.pat())));
        return readable || !reads;
    }
    if let Some(le) = ast::LetExpr::cast(parent.clone()) {
        return readable || !binds_err(le.pat());
    }
    if let Some(m) = ast::MethodCallExpr::cast(parent)
        && method_name(&m) == "unwrap_or_else"
    {
        return closure_ignores_arg(&m);
    }
    true
}

// `Ok(x)` / `Err(x)` / `Err(_)` / `_` with plain bindings: ("Ok" | "Err" | "_", binding).
fn simple_result_pat(p: &ast::Pat) -> Option<(&'static str, Option<ast::IdentPat>)> {
    match p {
        ast::Pat::WildcardPat(_) => Some(("_", None)),
        ast::Pat::TupleStructPat(tp) => {
            let name = tp.path()?.segment()?.name_ref()?.text().to_string();
            let which = match name.as_str() {
                "Ok" => "Ok",
                "Err" => "Err",
                _ => return None,
            };
            let mut fields = tp.fields();
            let f = fields.next()?;
            if fields.next().is_some() {
                return None;
            }
            match f {
                ast::Pat::WildcardPat(_) => Some((which, None)),
                ast::Pat::TuplePat(t) if t.fields().next().is_none() => Some((which, None)),
                ast::Pat::IdentPat(ip) if ip.pat().is_none() && ip.ref_token().is_none() => {
                    Some((which, Some(ip)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

// `Ok(..)` arms (any inner pattern, guards allowed) and one plain `Err(e)` / `_` arm.
fn match_on_result_ok(m: &ast::MatchExpr) -> bool {
    result_arms(m).is_some()
}

fn result_arms(m: &ast::MatchExpr) -> Option<(Vec<ast::MatchArm>, Option<ast::MatchArm>)> {
    let arms: Vec<ast::MatchArm> = m.match_arm_list()?.arms().collect();
    let (mut ok, mut err) = (Vec::new(), None);
    // Whether an unguarded `Ok(x)` / `Ok(_)` already takes every Ok value.
    let mut ok_total = false;
    for a in arms {
        let pat = a.pat()?;
        if let Some((which, _)) = simple_result_pat(&pat)
            && which != "Ok"
        {
            // `_` would also take unmatched Ok values, which the catch block can't see.
            let err_only = which == "Err" || ok_total;
            if a.guard().is_some() || err.is_some() || !err_only {
                return None;
            }
            err = Some(a);
            continue;
        }
        ok_pat_inner(&pat)?;
        ok_total |= a.guard().is_none() && simple_result_pat(&pat).is_some();
        ok.push(a);
    }
    (!ok.is_empty()).then_some((ok, err))
}

// The pattern inside `Ok(p)`.
fn ok_pat_inner(p: &ast::Pat) -> Option<ast::Pat> {
    let ast::Pat::TupleStructPat(tp) = p else { return None };
    if tp.path()?.segment()?.name_ref()?.text() != "Ok" {
        return None;
    }
    let mut fields = tp.fields();
    let f = fields.next()?;
    fields.next().is_none().then_some(f)
}

// The Java exception an `Err` of this type travels as.
pub fn exception_class(cx: &mut Cx, err: &Ty) -> String {
    match err.strip_refs() {
        Ty::Other(n) if crate::ty::is_parse_error_name(n) => {
            "NumberFormatException".into()
        }
        t if is_io_error(t) => {
            cx.import("java.io.*");
            "UncheckedIOException".into()
        }
        t => match user_error(t) {
            Some(name) => user_exception(cx, name),
            None => "RuntimeException".into(),
        },
    }
}

// A user error type (non-generic, declared in this file) travels in its own exception class.
fn user_error(ty: &Ty) -> Option<&str> {
    match ty.strip_refs() {
        Ty::Adt { java, args, .. } if args.is_empty() && !java.contains('.') => Some(java),
        _ => None,
    }
}

// `AppErrorException`, declared once per file: it carries the error value, and its message is
// the value's Display text.
fn user_exception(cx: &mut Cx, name: &str) -> String {
    let class = format!("{name}Exception");
    let decl = format!(
        "static class {class} extends RuntimeException {{ final {name} error; {class}({name} error) {{ super(String.valueOf(error)); this.error = error; }} }}"
    );
    if !cx.file_trailer.contains(&decl) {
        cx.file_trailer.push(decl);
    }
    class
}

// `throw` for `Err(payload)`.
pub fn push_throw(cx: &mut Cx, payload: &ast::Expr, line: u32, out: &mut Vec<Stmt>) {
    let inner = peel_conversions(payload);
    let ty = cx.ty(&inner);
    let v = expr::lower_expr(cx, &inner, out);
    let thrown = exception_of(cx, v, &ty);
    out.push(Stmt::new(line, StmtKind::Throw(thrown)));
}

// The type of an error value before `.into()`/`Box::new` (a `&str` boxed as `dyn Error`).
pub fn payload_ty(cx: &Cx, e: &ast::Expr) -> Ty {
    let e = match strip_parens(e) {
        ast::Expr::ClosureExpr(c) => match c.body() {
            Some(b) => b,
            None => return Ty::Other("?".into()),
        },
        e => e,
    };
    cx.ty(&peel_conversions(&e))
}

// `"msg".into()`, `Box::new(e)`, `X::from(e)`: the message or exception underneath.
fn peel_conversions(payload: &ast::Expr) -> ast::Expr {
    let mut inner = strip_parens(payload);
    loop {
        let next = match &inner {
            ast::Expr::MethodCallExpr(m) if method_name(m) == "into" => m.receiver(),
            ast::Expr::CallExpr(c)
                if c.expr().is_some_and(|f| {
                    let t = f.syntax().text().to_string();
                    t.ends_with("Box::new") || t.ends_with("::from")
                }) =>
            {
                c.arg_list().and_then(|a| a.args().next())
            }
            _ => None,
        };
        match next {
            Some(n) => inner = strip_parens(&n),
            None => return inner,
        }
    }
}

// The exception carrying an error value of type `ty`: a message, or the exception itself.
pub fn exception_of(cx: &mut Cx, v: Expr, ty: &Ty) -> Expr {
    if let Some(name) = user_error(ty) {
        return Expr::New {
            ty: user_exception(cx, name),
            args: vec![v],
        };
    }
    match ty.strip_refs() {
        Ty::Str => Expr::New {
            ty: "RuntimeException".into(),
            args: vec![v],
        },
        // Std errors exist only as caught exceptions; a boxed `dyn Error` may hold any value.
        Ty::Other(n) if n.contains("Error") && !n.contains("dyn ") => v,
        _ => Expr::New {
            ty: "RuntimeException".into(),
            args: vec![Expr::call(Expr::raw("String"), "valueOf", vec![v])],
        },
    }
}

fn err_ty(cx: &Cx, scrut: &ast::Expr) -> Ty {
    match cx.ty(scrut).strip_refs() {
        Ty::Result(_, e) => (**e).clone(),
        _ => Ty::Other("?".into()),
    }
}

fn arm_dest(cx: &Cx, dest: &Dest) -> Dest {
    match dest {
        Dest::Return if cx.ret_ty.as_ref().is_none_or(Ty::is_unit) => Dest::Discard,
        d => d.clone(),
    }
}

// try { <ok binding> = scrut; ok_body } catch (X e) { err_body }
// A block arm ending the try or catch block is that block's own statements (nothing follows it).
fn flatten_tail_block(stmts: &mut Vec<Stmt>) {
    if let Some(Stmt { kind: StmtKind::Block(_), .. }) = stmts.last()
        && let Some(Stmt { kind: StmtKind::Block(b), .. }) = stmts.pop()
    {
        stmts.extend(b.stmts);
    }
}

// What the try block does with the Ok value: bind it to one name, or test it against `Ok(..)` arms.
enum OkValue<'a> {
    Bind(Option<ast::IdentPat>),
    Arms(&'a [(Option<ast::Pat>, &'a ast::MatchArm)], &'a Dest),
}

#[allow(clippy::too_many_arguments)]
fn lower_try_catch(
    cx: &mut Cx,
    scrut: &ast::Expr,
    ok_bind: OkValue,
    ok_body: &dyn Fn(&mut Cx, &mut Vec<Stmt>),
    err_bind: Option<ast::IdentPat>,
    err_body: &dyn Fn(&mut Cx, &mut Vec<Stmt>),
    line: u32,
    close_line: u32,
    out: &mut Vec<Stmt>,
) {
    let err = err_ty(cx, scrut);
    let class = exception_class(cx, &err);
    let mut body = Vec::new();
    // Names declared in the try or catch block end with it, as in `stmt::lower_block`.
    let taken = cx.method_names.clone();
    cx.push_scope();
    let v = expr::lower_expr(cx, scrut, &mut body);
    match ok_bind {
        OkValue::Bind(Some(ip)) => super::pat::bind_pattern(cx, &ast::Pat::IdentPat(ip), v, &mut body),
        OkValue::Bind(None) => stmt::push_discard(&mut body, line, v),
        OkValue::Arms(arms, dest) => {
            let ok_ty = match cx.ty(scrut).strip_refs() {
                Ty::Result(ok, _) => (**ok).clone(),
                t => t.clone(),
            };
            let value = cx.fresh("value");
            body.push(Stmt::new(
                line,
                StmtKind::Local { ty: stmt::decl_type_explicit(&ok_ty), name: value.clone(), init: Some(v) },
            ));
            // `case Tok.Num(var n) when n > 40 ->` reads better than an if-chain with casts.
            if super::pat::can_switch_alts(cx, arms, &ok_ty) {
                super::pat::switch_over(cx, Expr::name(value), arms, dest, &mut body, line);
            } else {
                super::pat::chain_over(cx, Expr::name(value), arms, dest, &mut body);
                flatten_tail_block(&mut body);
            }
        }
    }
    ok_body(cx, &mut body);
    cx.pop_scope();
    cx.method_names = taken.clone();
    let mut handler = Vec::new();
    cx.push_scope();
    let decl = catch_decl(cx, err_bind, &err, &class, line, &mut handler);
    err_body(cx, &mut handler);
    flatten_tail_block(&mut handler);
    cx.pop_scope();
    cx.method_names = taken;
    out.push(Stmt::new(
        line,
        StmtKind::Try {
            body: Block {
                stmts: body,
                close_line,
            },
            decl,
            handler: Block {
                stmts: handler,
                close_line,
            },
        },
    ));
}

// The catch clause for an error bound as `err_bind`, reading the error value back when needed.
fn catch_decl(
    cx: &mut Cx,
    err_bind: Option<ast::IdentPat>,
    err: &Ty,
    class: &str,
    line: u32,
    handler: &mut Vec<Stmt>,
) -> String {
    let rust_name = err_bind.as_ref().and_then(|ip| ip.name()).map(|n| n.text().to_string());
    match (&rust_name, err.strip_refs()) {
        (Some(n), Ty::Str) => {
            let ex = cx.fresh_param("ex");
            let java = cx.declare(n);
            handler.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: "String".into(),
                    name: java,
                    init: Some(Expr::call(Expr::name(&ex), "getMessage", vec![])),
                },
            ));
            format!("{class} {ex}")
        }
        (Some(n), t) if user_error(t).is_some() => {
            let ex = cx.fresh_param("ex");
            let java = cx.declare(n);
            handler.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: t.java(),
                    name: java,
                    init: Some(Expr::Field(Box::new(Expr::name(&ex)), "error".into())),
                },
            ));
            format!("{class} {ex}")
        }
        (Some(n), _) => format!("{class} {}", cx.declare(n)),
        (None, _) => format!("{class} {}", cx.fresh_param("e")),
    }
}

// `r.map_err(|e| f(e))`: `try { v = r; } catch (X e) { throw <f(e)>; }`, so the converted error
// is what travels on. None for a unit Ok value or a closure this can't bind.
fn map_err_rethrow(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    ok_ty: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    if ok_ty.is_unit() || !ok_ty.is_nameable() {
        return None;
    }
    let ast::Expr::ClosureExpr(c) = strip_parens(&m.arg_list()?.args().next()?) else {
        return None;
    };
    let bind = match c.param_list()?.params().next()?.pat()? {
        ast::Pat::IdentPat(ip) if ip.pat().is_none() => Some(ip),
        ast::Pat::WildcardPat(_) => None,
        _ => return None,
    };
    let body = c.body()?;
    let target = m
        .syntax()
        .parent()
        .filter(|p| ast::TryExpr::can_cast(p.kind()))
        .unwrap_or_else(|| m.syntax().clone());
    let var = cx.take_let_target(&target).unwrap_or_else(|| cx.fresh("value"));
    out.push(Stmt::new(
        line,
        StmtKind::Local { ty: stmt::decl_type_explicit(ok_ty), name: var.clone(), init: None },
    ));
    let err = err_ty(cx, recv_ast);
    let class = exception_class(cx, &err);
    let taken = cx.method_names.clone();
    cx.push_scope();
    let mut handler = Vec::new();
    let decl = catch_decl(cx, bind, &err, &class, line, &mut handler);
    push_throw(cx, &body, line, &mut handler);
    cx.pop_scope();
    cx.method_names = taken;
    let assign = Stmt::new(line, StmtKind::Expr(Expr::Assign(Box::new(Expr::name(&var)), "=", Box::new(recv))));
    out.push(Stmt::new(
        line,
        StmtKind::Try {
            body: Block { stmts: vec![assign], close_line: line },
            decl,
            handler: Block { stmts: handler, close_line: line },
        },
    ));
    Some(Expr::name(var))
}

// Result methods on an erased Result (`recv` is the Ok value, or throws).
pub fn result_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ast: &ast::Expr,
    recv: Expr,
    args: Vec<Expr>,
    name: &str,
    ret: &Ty,
    line: u32,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let ok_ty = match cx.ty(recv_ast).strip_refs() {
        Ty::Result(ok, _) => (**ok).clone(),
        _ => Ty::Other("?".into()),
    };
    if name == "map_err"
        && let Some(v) = map_err_rethrow(cx, m, recv_ast, recv.clone(), &ok_ty, line, out)
    {
        return Some(v);
    }
    // `r.map(f)` / `r.and_then(f)`: `r` already is the Ok value, so this is `f(r)` (which may throw).
    if name == "map" || name == "and_then" {
        let arg = strip_parens(&m.arg_list()?.args().next()?);
        if let ast::Expr::ClosureExpr(c) = &arg
            && let [p] = c.param_list()?.params().collect::<Vec<_>>().as_slice()
        {
            cx.push_scope();
            super::pat::bind_pattern(cx, &p.pat()?, recv, out);
            let v = expr::lower_expr(cx, &c.body()?, out);
            cx.pop_scope();
            return Some(v);
        }
        // A method reference, possibly cast to its functional interface.
        let f = match args.first()? {
            Expr::Cast(_, inner) => crate::emit::render_expr(inner),
            e => crate::emit::render_expr(e),
        };
        let (owner, method) = f.rsplit_once("::")?;
        return Some(if method == "new" {
            Expr::New { ty: owner.to_string(), args: vec![recv] }
        } else if owner == "Main" {
            Expr::static_call(method, vec![recv])
        } else {
            Expr::call(Expr::raw(owner), method, vec![recv])
        });
    }
    let (value, fallback) = match name {
        "unwrap" | "expect" | "map_err" => return Some(recv),
        "unwrap_or" => (recv, args.into_iter().next()?),
        "unwrap_or_default" => (recv, Expr::raw(super::items::default_value(cx, &ok_ty))),
        // `|_| v`: the closure's value; a closure reading the error isn't supported here.
        "unwrap_or_else" => match args.into_iter().next()? {
            Expr::Lambda {
                body: crate::jir::LambdaBody::Expr(b),
                ..
            } if closure_ignores_arg(m) => (recv, *b),
            _ => {
                return Some(cx.unsupported(
                    m.syntax(),
                    "`unwrap_or_else` reading the error (simple mode)",
                ));
            }
        },
        "ok" => (
            Expr::call(Expr::raw("Optional"), "of", vec![recv]),
            Expr::raw("Optional.empty()"),
        ),
        "is_ok" | "is_err" => {
            let mut try_body = Vec::new();
            stmt::push_discard(&mut try_body, line, recv);
            let name_ok = name == "is_ok";
            let flag = cx
                .take_let_target(m.syntax())
                .unwrap_or_else(|| cx.fresh(if name_ok { "ok" } else { "failed" }));
            out.push(Stmt::new(
                line,
                StmtKind::Local {
                    ty: "boolean".into(),
                    name: flag.clone(),
                    init: None,
                },
            ));
            let set = |v: bool| {
                Stmt::new(
                    line,
                    StmtKind::Expr(Expr::Assign(
                        Box::new(Expr::name(&flag)),
                        "=",
                        Box::new(Expr::raw(if v { "true" } else { "false" })),
                    )),
                )
            };
            try_body.push(set(name_ok));
            push_try(cx, recv_ast, try_body, vec![set(!name_ok)], line, out);
            return Some(Expr::name(flag));
        }
        _ => return None,
    };
    let hint = if name == "ok" { "maybe" } else { "value" };
    let var = cx.take_let_target(m.syntax()).unwrap_or_else(|| cx.fresh(hint));
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: stmt::decl_type_explicit(ret),
            name: var.clone(),
            init: None,
        },
    ));
    let assign = |v: Expr| {
        Stmt::new(
            line,
            StmtKind::Expr(Expr::Assign(Box::new(Expr::name(&var)), "=", Box::new(v))),
        )
    };
    push_try(cx, recv_ast, vec![assign(value)], vec![assign(fallback)], line, out);
    Some(Expr::name(var))
}

fn closure_ignores_arg(m: &ast::MethodCallExpr) -> bool {
    let Some(ast::Expr::ClosureExpr(c)) = m.arg_list().and_then(|l| l.args().next()) else {
        return false;
    };
    c.param_list()
        .and_then(|pl| pl.params().next())
        .and_then(|p| p.pat())
        .is_some_and(|p| matches!(p, ast::Pat::WildcardPat(_)))
}

fn push_try(
    cx: &mut Cx,
    recv_ast: &ast::Expr,
    body: Vec<Stmt>,
    handler: Vec<Stmt>,
    line: u32,
    out: &mut Vec<Stmt>,
) {
    let err = err_ty(cx, recv_ast);
    let class = exception_class(cx, &err);
    let e = cx.fresh_param("e");
    out.push(Stmt::new(
        line,
        StmtKind::Try {
            body: Block {
                stmts: body,
                close_line: line,
            },
            decl: format!("{class} {e}"),
            handler: Block {
                stmts: handler,
                close_line: line,
            },
        },
    ));
    cx.release_param(&e);
}

// `match r { Ok(v) => a, Err(e) => b }` as try/catch. False when the shape doesn't fit.
pub fn lower_match(cx: &mut Cx, m: &ast::MatchExpr, dest: &Dest, out: &mut Vec<Stmt>) -> bool {
    let (Some(scrut), Some((oks, err))) = (m.expr(), result_arms(m)) else {
        return false;
    };
    let line = cx.line(m.syntax());
    let close = cx.end_line(m.syntax());
    let dest = arm_dest(cx, dest);
    let err_bind = err
        .as_ref()
        .and_then(|a| a.pat())
        .and_then(|p| simple_result_pat(&p))
        .and_then(|(_, b)| b);
    let err_expr = err.and_then(|a| a.expr());
    let run_err = |cx: &mut Cx, out: &mut Vec<Stmt>| {
        if let Some(e) = &err_expr {
            stmt::lower_to_dest(cx, e, &dest, out);
        }
    };
    // `Ok(Some(x))`, `Ok((a, b))`, guards, several Ok arms: test the value inside the try.
    let plain = matches!(oks.as_slice(), [a] if a.guard().is_none() && a.pat().and_then(|p| simple_result_pat(&p)).is_some());
    if !plain {
        let alts: Vec<(Option<ast::Pat>, &ast::MatchArm)> =
            oks.iter().map(|a| (a.pat().as_ref().and_then(ok_pat_inner), a)).collect();
        let ok = OkValue::Arms(&alts, &dest);
        lower_try_catch(cx, &scrut, ok, &|_, _| {}, err_bind, &run_err, line, close, out);
        return true;
    }
    let ok = oks.into_iter().next().unwrap();
    let ok_bind = ok.pat().and_then(|p| simple_result_pat(&p)).and_then(|(_, b)| b);
    let ok_expr = ok.expr();
    let run_ok = |cx: &mut Cx, out: &mut Vec<Stmt>| {
        if let Some(e) = &ok_expr {
            stmt::lower_to_dest(cx, e, &dest, out);
        }
    };
    lower_try_catch(cx, &scrut, OkValue::Bind(ok_bind), &run_ok, err_bind, &run_err, line, close, out);
    true
}

// `if let Ok(v) = r { a } else { b }` / `if let Err(e) = r { b }` as try/catch.
pub fn lower_if_let(cx: &mut Cx, i: &ast::IfExpr, dest: &Dest, out: &mut Vec<Stmt>) -> bool {
    let Some(ast::Expr::LetExpr(le)) = i.condition() else {
        return false;
    };
    let (Some(scrut), Some(pat)) = (le.expr(), le.pat()) else {
        return false;
    };
    let Some((which, bind)) = simple_result_pat(&pat) else {
        return false;
    };
    let line = cx.line(i.syntax());
    let close = cx.end_line(i.syntax());
    let dest = arm_dest(cx, dest);
    let then = i.then_branch();
    let els = i.else_branch();
    let run_then = |cx: &mut Cx, out: &mut Vec<Stmt>| {
        if let Some(b) = &then {
            out.extend(stmt::lower_block(cx, b, &dest).stmts);
        }
    };
    let run_else = |cx: &mut Cx, out: &mut Vec<Stmt>| match &els {
        Some(ast::ElseBranch::Block(b)) => out.extend(stmt::lower_block(cx, b, &dest).stmts),
        Some(ast::ElseBranch::IfExpr(ei)) => stmt::lower_if(cx, ei, &dest, out),
        None => {}
    };
    if which == "Ok" {
        lower_try_catch(cx, &scrut, OkValue::Bind(bind), &run_then, None, &run_else, line, close, out);
    } else {
        lower_try_catch(cx, &scrut, OkValue::Bind(None), &run_else, bind, &run_then, line, close, out);
    }
    true
}
