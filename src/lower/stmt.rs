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
    let mut stmts = Vec::new();
    if let Some(list) = b.stmt_list() {
        lower_stmt_list(cx, &list, dest, &mut stmts);
    }
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

pub fn lower_let(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let line = cx.line(l.syntax());
    let Some(p) = l.pat() else { return };
    if l.let_else().is_some() {
        pat::lower_let_else(cx, l, out);
        return;
    }
    let ty = match l.ty() {
        Some(t) => cx.type_ref(&t),
        None => cx.pat_ty(&p),
    };
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
    if matches!(ty, Ty::Iter(_)) && super::iter::lower_iter_let(cx, l, ip, &rust_name, &ty, out) {
        return;
    }
    if let Some(inner) = super::std_map::ptr::array_cell_pat(cx, ip) {
        lower_handle_let(cx, l, ip, &rust_name, &inner, out);
        return;
    }
    if let Some(inner) = super::std_map::ptr::ref_mut_pat(cx, ip) {
        lower_ref_let(cx, l, ip, &rust_name, &inner, out);
        return;
    }
    if cx.binding_needs_cell(ip, &ty) {
        lower_cell_let(cx, l, ip, &rust_name, &ty, out);
        return;
    }
    let jty = decl_type(&ty);
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
            // The initializer is lowered before declaring so `let x = x + 1` sees the old `x`.
            let e = expr::lower_expr_as(cx, &init, &ty, out);
            let name = cx.declare(&rust_name);
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
    match l.initializer() {
        Some(init) if !is_control_flow(&init) => {
            let e = expr::lower_expr_as(cx, &init, ty, out);
            let name = cx.declare(rust_name);
            cx.mark_cell(ip);
            let init = Expr::raw(format!("{{{}}}", render(&e)));
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
            let alloc = Expr::raw(format!("new {elem}[1]"));
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
    } else {
        "var".into()
    }
}

// For declarations without initializer, where `var` is not allowed.
pub fn decl_type_explicit(ty: &Ty) -> String {
    if ty.is_nameable() && !ty.is_unit() {
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
                    value_var: match dest {
                        Dest::Assign(v) => Some(v.clone()),
                        _ => None,
                    },
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
            match dest {
                Dest::Discard => {
                    let v = expr::lower_expr(cx, e, out);
                    push_discard(out, line, v);
                }
                Dest::Return => {
                    let unit_fn = cx.ret_ty.as_ref().is_none_or(Ty::is_unit);
                    if unit_fn {
                        let v = expr::lower_expr(cx, e, out);
                        push_discard(out, line, v);
                        if !is_tail_of_fn(e) {
                            out.push(Stmt::new(line, StmtKind::Return(None)));
                        }
                    } else {
                        let ret = cx.ret_ty.clone().unwrap();
                        let v = expr::lower_expr_as(cx, e, &ret, out);
                        // A `Cell<i32>` return value is the one-element array, not its contents.
                        let v = match super::std_map::ptr::is_array(cx, e) {
                            Some(inner) => super::std_map::ptr::as_handle(cx, e, v, &inner),
                            None => v,
                        };
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
                }
                Dest::Yield => {
                    let v = expr::lower_expr(cx, e, out);
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
        value_var: match dest {
            Dest::Assign(v) => Some(v.clone()),
            _ => None,
        },
        switch_depth: cx.switch_depth,
        is_block: false,
    };
    cx.loops.push(frame);
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
        match idx.and_then(|i| cx.loops[i].value_var.clone()) {
            Some(var) => lower_to_dest(cx, &v, &Dest::Assign(var), out),
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
