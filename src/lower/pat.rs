// Patterns: match, if-let / while-let / let-chains, let-else, destructuring, `?`.

use ra_ap_hir::{Adt, ModuleDef, PathResolution, StructKind};
use ra_ap_syntax::{AstNode, SyntaxNode, ast, ast::HasName, ast::RangeItem};

use super::stmt::{self, Dest};
use super::{Cx, escape_keyword, expr, items};
use crate::jir::{Arm, ArmBody, Block, Expr, Stmt, StmtKind};
use crate::ty::Ty;

// A Rust binding created by a pattern. Extraction bindings need a `var name = value;` in the
// body; the others are Java pattern variables that the condition already introduces.
struct Bind {
    line: u32,
    rust: String,
    java: String,
    value: Expr,
    ty: Ty,
    extracted: bool,
}

#[derive(Default)]
struct PatLowering {
    binds: Vec<Bind>,
}

// Where Java puts the bindings of a pattern.
#[derive(Clone, Copy, PartialEq)]
enum Mode {
    // Condition and body share a scope (`if`, `when`): record patterns can carry the bindings.
    If,
    // Loop conditions and let-else: extract into locals, never into the condition.
    Extract,
    // `matches!`: throwaway names, nothing binds.
    Test,
}

// --- conditions (if let / while let / let chains) --------------------------------------------

// Lowers an `if`/`while` condition. Bindings introduced by `let` patterns are pushed to `binds`
// (placed at the start of the body); statements needed before the test go to `out`.
pub fn lower_condition(cx: &mut Cx, cond: &ast::Expr, out: &mut Vec<Stmt>, binds: &mut Vec<Stmt>) -> Expr {
    if !expr::contains_let(cond) {
        return expr::lower_expr(cx, cond, out);
    }
    let in_while = cond.syntax().parent().is_some_and(|p| ast::WhileExpr::can_cast(p.kind()));
    let mode = if in_while { Mode::Extract } else { Mode::If };
    let parts = flatten_and(cond);
    let mut tests: Vec<Expr> = Vec::new();
    let mut all: Vec<Bind> = Vec::new();
    let mut aliased = false;
    for (i, part) in parts.iter().enumerate() {
        let ast::Expr::LetExpr(le) = part else {
            tests.push(expr::lower_expr(cx, part, out));
            continue;
        };
        let (Some(p), Some(init)) = (le.pat(), le.expr()) else {
            cx.unsupported(part.syntax(), "`let` expression without pattern or value");
            continue;
        };
        // `while let Some(x) = v.pop()`: test emptiness, pop in the body.
        if i + 1 == parts.len() && let Some((cond, bind)) = pop_idiom(cx, &p, &init) {
            tests.push(cond);
            all.extend(bind);
            continue;
        }
        let first = i == 0;
        let value = chain_value(cx, &init, first, out, &mut tests);
        let mut lowered = PatLowering::default();
        let c = lower_pat(cx, &p, value, mode, &mut lowered);
        tests.extend(c);
        if i + 1 < parts.len() && !lowered.binds.is_empty() {
            if !aliased {
                cx.push_alias();
                aliased = true;
            }
            for b in &lowered.binds {
                cx.bind_alias(&b.rust, crate::emit::render_expr(&b.value));
            }
        }
        all.extend(lowered.binds);
    }
    if aliased {
        cx.pop_alias();
    }
    emit_binds(&all, binds);
    and(tests).unwrap_or(Expr::raw("true"))
}

// Value for one `let` component of a chain, evaluated exactly once.
fn chain_value(cx: &mut Cx, init: &ast::Expr, first: bool, out: &mut Vec<Stmt>, tests: &mut Vec<Expr>) -> Expr {
    let line = cx.line(init.syntax());
    let v = expr::lower_expr(cx, init, out);
    let ty = cx.ty(init);
    if is_place(&v) {
        return v;
    }
    if first {
        let tmp = cx.fresh("val");
        out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type(&ty), name: tmp.clone(), init: Some(v) }));
        return Expr::name(tmp);
    }
    // Later components must stay inside the test: earlier components may guard this one.
    let tmp = cx.fresh("val");
    out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type_explicit(&ty), name: tmp.clone(), init: None }));
    let assign = Expr::Assign(Box::new(Expr::name(tmp.clone())), "=", Box::new(v));
    tests.push(assign_test(assign, &ty));
    Expr::name(tmp)
}

// A boolean whose evaluation assigns `assign`: `(n = e) == n` / `(o = e) != null`.
fn assign_test(assign: Expr, ty: &Ty) -> Expr {
    if ty.is_primitive() {
        Expr::bin(assign.clone(), "==", assign)
    } else {
        Expr::bin(assign, "!=", Expr::raw("null"))
    }
}

// A pure place: names and field chains, never a call (which would run twice).
fn is_place(e: &Expr) -> bool {
    match e {
        Expr::Name(_) => true,
        Expr::Field(base, _) => is_place(base),
        _ => false,
    }
}

fn flatten_and(e: &ast::Expr) -> Vec<ast::Expr> {
    use ast::{BinaryOp, LogicOp};
    match e {
        ast::Expr::BinExpr(b) if matches!(b.op_kind(), Some(BinaryOp::LogicOp(LogicOp::And))) => match (b.lhs(), b.rhs()) {
            (Some(l), Some(r)) => {
                let mut parts = flatten_and(&l);
                parts.extend(flatten_and(&r));
                parts
            }
            _ => vec![e.clone()],
        },
        ast::Expr::ParenExpr(p) => p.expr().map(|i| flatten_and(&i)).unwrap_or_else(|| vec![e.clone()]),
        _ => vec![e.clone()],
    }
}

// `while let Some(x) = v.pop()` -> `while (!v.isEmpty()) { var x = v.removeLast(); ... }`.
fn pop_idiom(cx: &mut Cx, p: &ast::Pat, init: &ast::Expr) -> Option<(Expr, Option<Bind>)> {
    let ast::Pat::TupleStructPat(tp) = p else { return None };
    if !matches!(cx.pat_ty(p).strip_refs(), Ty::Option(_)) {
        return None;
    }
    let inner: ast::Pat = tp.fields().next()?;
    let ast::Expr::MethodCallExpr(m) = init else { return None };
    let recv_ast = m.receiver()?;
    let rty = cx.ty(&recv_ast);
    let pop = match (rty.strip_refs(), m.name_ref()?.text().to_string().as_str()) {
        (Ty::Vec(_), "pop") => "removeLast",
        (Ty::Deque(_), "pop_back") => "pollLast",
        (Ty::Deque(_), "pop_front") => "pollFirst",
        _ => return None,
    };
    let mut pre = Vec::new();
    let recv = expr::lower_expr(cx, &recv_ast, &mut pre);
    if !pre.is_empty() || !is_place(&recv) {
        return None;
    }
    let cond = Expr::Unary("!", Box::new(Expr::call(recv.clone(), "isEmpty", vec![])));
    let bind = match &inner {
        ast::Pat::IdentPat(ip) if ip.pat().is_none() && bind_pat_def(cx, ip).is_none() => {
            let line = cx.line(ip.syntax());
            let rust = ident_name(&ip);
            let java = cx.declare(&rust);
            let ty = cx.pat_ty(&inner);
            Some(Bind {
                line,
                rust,
                java,
                value: Expr::call(recv, pop, vec![]),
                ty,
                extracted: true,
            })
        }
        ast::Pat::WildcardPat(_) => None,
        _ => return None,
    };
    Some((cond, bind))
}

// --- `let … else` and plain destructuring ---------------------------------------------------

pub fn lower_let_else(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let line = cx.line(l.syntax());
    let (Some(p), Some(init), Some(le)) = (l.pat(), l.initializer(), l.let_else()) else {
        cx.unsupported(l.syntax(), "`let` without pattern, value or `else`");
        return;
    };
    let Some(else_block) = le.block_expr() else {
        cx.unsupported(l.syntax(), "`let … else` without block");
        return;
    };
    let value = place(cx, &init, out);
    let mut lowered = PatLowering::default();
    let conds = lower_pat(cx, &p, value, Mode::Extract, &mut lowered);
    let cond = and(conds).unwrap_or(Expr::raw("true"));
    let diverge = stmt::lower_block(cx, &else_block, &Dest::Discard);
    out.push(Stmt::new(
        line,
        StmtKind::If { cond: Expr::Unary("!", Box::new(cond)), then: diverge, els: None },
    ));
    emit_binds(&lowered.binds, out);
}

// `let (a, b) = e;` and other irrefutable destructuring lets.
pub fn lower_destructuring_let(cx: &mut Cx, l: &ast::LetStmt, out: &mut Vec<Stmt>) {
    let (Some(p), Some(init)) = (l.pat(), l.initializer()) else {
        cx.unsupported(l.syntax(), "destructuring `let` without a value");
        return;
    };
    let value = place(cx, &init, out);
    let mut lowered = PatLowering::default();
    let conds = lower_pat(cx, &p, value, Mode::Extract, &mut lowered);
    if !conds.is_empty() {
        cx.unsupported(l.syntax(), "refutable pattern in `let` without `else`");
    }
    emit_binds(&lowered.binds, out);
}

// Binds an irrefutable pattern against `value`, declaring Java locals.
pub fn bind_pattern(cx: &mut Cx, p: &ast::Pat, value: Expr, out: &mut Vec<Stmt>) {
    let mut lowered = PatLowering::default();
    let conds = lower_pat(cx, p, value, Mode::Extract, &mut lowered);
    if !conds.is_empty() {
        // Param/closure patterns are irrefutable; a test here means an unsupported construct.
        cx.error(p.syntax(), "pattern needs a test but only bindings are allowed here");
    }
    emit_binds(&lowered.binds, out);
}

// Boolean test for a pattern (no bindings reach the caller); `matches!` uses this.
#[allow(dead_code)]
pub fn pattern_test(cx: &mut Cx, p: &ast::Pat, scrut: Expr, out: &mut Vec<Stmt>) -> Expr {
    let value = if is_place(&scrut) {
        scrut
    } else {
        let tmp = cx.fresh("m");
        out.push(Stmt::new(cx.line(p.syntax()), StmtKind::Local { ty: "var".into(), name: tmp.clone(), init: Some(scrut) }));
        Expr::name(tmp)
    };
    let mut lowered = PatLowering::default();
    let conds = lower_pat(cx, p, value, Mode::Test, &mut lowered);
    and(conds).unwrap_or(Expr::raw("true"))
}

// --- match ----------------------------------------------------------------------------------

pub fn lower_match_stmt(cx: &mut Cx, m: &ast::MatchExpr, dest: &Dest, out: &mut Vec<Stmt>) {
    let line = cx.line(m.syntax());
    if let Dest::Yield = dest {
        if let Some(e) = try_match_expr(cx, m, out) {
            out.push(Stmt::new(line, StmtKind::Raw(format!("yield {};", crate::emit::render_expr(&e)))));
            return;
        }
        let ty = cx.ty(&ast::Expr::MatchExpr(m.clone()));
        let tmp = cx.fresh("arm_value");
        out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type_explicit(&ty), name: tmp.clone(), init: None }));
        lower_match_stmt(cx, m, &Dest::Assign(tmp.clone()), out);
        out.push(Stmt::new(line, StmtKind::Raw(format!("yield {tmp};"))));
        return;
    }
    let (Some(scrut), Some(list)) = (m.expr(), m.match_arm_list()) else {
        let e = cx.unsupported(m.syntax(), "`match` without scrutinee");
        out.push(Stmt::new(line, StmtKind::Expr(e)));
        return;
    };
    let arms: Vec<ast::MatchArm> = list.arms().collect();
    if arms.is_empty() {
        let e = cx.unsupported(m.syntax(), "`match` without arms");
        out.push(Stmt::new(line, StmtKind::Expr(e)));
        return;
    }
    let scrut_ty = cx.ty(&scrut);
    let arm_dest = match dest {
        Dest::Return if cx.ret_ty.as_ref().is_none_or(Ty::is_unit) => Dest::Discard,
        d => d.clone(),
    };
    // A single irrefutable arm is just a binding followed by the body.
    if arms.len() == 1 && arms[0].guard().is_none() && arms[0].pat().is_some_and(|p| is_total(cx, &p)) {
        bound_match(cx, &scrut, &arms[0], &arm_dest, out);
        return;
    }
    if can_switch(cx, &arms, &scrut_ty) {
        lower_switch_stmt(cx, &scrut, &arms, &arm_dest, out, line);
    } else {
        lower_chain(cx, &scrut, &arms, &arm_dest, out, line);
    }
}

// `match` in value position as a Java switch expression, when every arm allows it.
pub fn try_match_expr(cx: &mut Cx, m: &ast::MatchExpr, out: &mut Vec<Stmt>) -> Option<Expr> {
    let scrut = m.expr()?;
    if cx.ty(&ast::Expr::MatchExpr(m.clone())).is_unit() {
        return None;
    }
    let arms: Vec<ast::MatchArm> = m.match_arm_list()?.arms().collect();
    if arms.is_empty() || !can_switch(cx, &arms, &cx.ty(&scrut)) {
        return None;
    }
    if arms.iter().any(|a| a.expr().is_some_and(|b| !arm_body_safe(&b))) {
        return None;
    }
    let line = cx.line(m.syntax());
    let value = place(cx, &scrut, out);
    let mut jarms = Vec::new();
    for arm in &arms {
        let aline = cx.line(arm.syntax());
        let Some(p) = arm.pat() else { continue };
        cx.push_scope();
        let mut binds = PatLowering::default();
        let mut label = if is_binding_pat(cx, &p) {
            lower_pat(cx, &p, value.clone(), Mode::Extract, &mut binds);
            "default".to_string()
        } else {
            pattern_text(cx, &p, true).unwrap_or_else(|| {
                cx.error(p.syntax(), "pattern cannot be a Java switch label");
                "default".to_string()
            })
        };
        if let Some(g) = arm.guard() {
            label = format!("{label} when {}", guard_text(cx, &g, &binds));
        }
        let body = match arm.expr() {
            Some(b) => arm_body_expr(cx, &b, aline, &binds),
            None => ArmBody::Block(Block::default()),
        };
        cx.pop_scope();
        jarms.push(Arm { line: aline, label, body });
    }
    let _ = line;
    Some(Expr::Switch { scrut: Box::new(value), arms: jarms })
}

fn lower_switch_stmt(cx: &mut Cx, scrut: &ast::Expr, arms: &[ast::MatchArm], dest: &Dest, out: &mut Vec<Stmt>, line: u32) {
    let value = place(cx, scrut, out);
    let mut jarms = Vec::new();
    // A `break` in an arm body would leave the switch, not the enclosing loop.
    cx.switch_depth += 1;
    for arm in arms {
        let aline = cx.line(arm.syntax());
        let Some(p) = arm.pat() else { continue };
        cx.push_scope();
        let mut binds = PatLowering::default();
        let mut label = if is_binding_pat(cx, &p) {
            lower_pat(cx, &p, value.clone(), Mode::Extract, &mut binds);
            "default".to_string()
        } else {
            pattern_text(cx, &p, true).unwrap_or_else(|| {
                cx.error(p.syntax(), "pattern cannot be a Java switch label");
                "default".to_string()
            })
        };
        if let Some(g) = arm.guard() {
            label = format!("{label} when {}", guard_text(cx, &g, &binds));
        }
        let body = match arm.expr() {
            Some(b) => arm_body_stmt(cx, &b, dest, aline, &binds),
            None => ArmBody::Block(Block::default()),
        };
        cx.pop_scope();
        jarms.push(Arm { line: aline, label, body });
    }
    cx.switch_depth -= 1;
    out.push(Stmt::new(line, StmtKind::Switch { scrut: value, arms: jarms }));
}

// Single irrefutable arm: bindings, then the body.
fn bound_match(cx: &mut Cx, scrut: &ast::Expr, arm: &ast::MatchArm, dest: &Dest, out: &mut Vec<Stmt>) {
    let value = place(cx, scrut, out);
    cx.push_scope();
    let mut lowered = PatLowering::default();
    if let Some(p) = arm.pat() {
        lower_pat(cx, &p, value, Mode::Extract, &mut lowered);
    }
    emit_binds(&lowered.binds, out);
    if let Some(b) = arm.expr() {
        stmt::lower_to_dest(cx, &b, dest, out);
    }
    cx.pop_scope();
}

fn guard_text(cx: &mut Cx, g: &ast::MatchGuard, binds: &PatLowering) -> String {
    let mut pre = Vec::new();
    let e = lower_guard(cx, g, &mut pre, binds);
    if !pre.is_empty() {
        cx.error(g.syntax(), "match guard needs hoisted statements (e.g. `?`)");
    }
    crate::emit::render_expr(&e)
}

fn lower_guard(cx: &mut Cx, g: &ast::MatchGuard, pre: &mut Vec<Stmt>, binds: &PatLowering) -> Expr {
    cx.push_alias();
    for b in binds.binds.iter().filter(|b| b.extracted) {
        cx.bind_alias(&b.rust, crate::emit::render_expr(&b.value));
    }
    let e = g
        .syntax()
        .children()
        .find_map(ast::Expr::cast)
        .map(|e| expr::lower_expr(cx, &e, pre))
        .unwrap_or_else(|| Expr::raw("true"));
    cx.pop_alias();
    e
}

// If/else-if chain on a temporary: anything Java's switch cannot express.
fn lower_chain(cx: &mut Cx, scrut: &ast::Expr, arms: &[ast::MatchArm], dest: &Dest, out: &mut Vec<Stmt>, line: u32) {
    let _ = line;
    let end = arms
        .iter()
        .position(|a| a.guard().is_none() && a.pat().is_some_and(|p| is_total(cx, &p)))
        .map_or(arms.len(), |i| i + 1);
    let arms = &arms[..end];
    if arms.len() == 1 && arms[0].guard().is_none() {
        bound_match(cx, scrut, &arms[0], dest, out);
        return;
    }
    let value = place(cx, scrut, out);
    let mut els: Option<Box<Stmt>> = None;
    for (i, arm) in arms.iter().enumerate().rev() {
        let aline = cx.line(arm.syntax());
        // rustc already checked exhaustiveness: nothing else can reach the last arm.
        let last = i + 1 == arms.len();
        cx.push_scope();
        let mut lowered = PatLowering::default();
        let mut conds = match arm.pat() {
            Some(p) if !last => lower_pat(cx, &p, value.clone(), Mode::If, &mut lowered),
            Some(p) => {
                lower_pat(cx, &p, value.clone(), Mode::Extract, &mut lowered);
                Vec::new()
            }
            None => Vec::new(),
        };
        if let Some(g) = arm.guard() {
            let mut pre = Vec::new();
            let e = lower_guard(cx, &g, &mut pre, &lowered);
            if !pre.is_empty() {
                cx.error(g.syntax(), "match guard needs statements");
            }
            conds.push(e);
        }
        let mut stmts = Vec::new();
        emit_binds(&lowered.binds, &mut stmts);
        let close_line = match arm.expr() {
            Some(b) => {
                stmt::lower_to_dest(cx, &b, dest, &mut stmts);
                cx.end_line(b.syntax())
            }
            None => aline,
        };
        cx.pop_scope();
        els = Some(match and(conds) {
            Some(cond) => Box::new(Stmt::new(
                aline,
                StmtKind::If { cond, then: Block { stmts, close_line }, els: els.take() },
            )),
            None => Box::new(Stmt::new(aline, StmtKind::Block(Block { stmts, close_line }))),
        });
    }
    if let Some(e) = els {
        out.push(*e);
    }
}

fn arm_body_stmt(cx: &mut Cx, body: &ast::Expr, dest: &Dest, line: u32, binds: &PatLowering) -> ArmBody {
    let mut stmts = Vec::new();
    emit_binds(&binds.binds, &mut stmts);
    stmt::lower_to_dest(cx, body, dest, &mut stmts);
    let close_line = cx.end_line(body.syntax());
    match stmts.len() {
        1 => match stmts.remove(0).kind {
            StmtKind::Throw(e) => ArmBody::Throw(e),
            StmtKind::Expr(e) if e.is_statement_expr() => ArmBody::Expr(e),
            StmtKind::Block(b) => ArmBody::Block(b),
            kind => ArmBody::Block(Block { stmts: vec![Stmt::new(line, kind)], close_line }),
        },
        _ => ArmBody::Block(Block { stmts, close_line }),
    }
}

fn arm_body_expr(cx: &mut Cx, body: &ast::Expr, line: u32, binds: &PatLowering) -> ArmBody {
    let mut stmts = Vec::new();
    emit_binds(&binds.binds, &mut stmts);
    let close_line = cx.end_line(body.syntax());
    let block = |stmts: Vec<Stmt>| ArmBody::Block(Block { stmts, close_line });
    if let ast::Expr::MacroExpr(m) = body
        && super::macros::is_diverging(m)
    {
        let e = expr::lower_expr(cx, body, &mut stmts);
        return match stmts.len() {
            0 => ArmBody::Throw(e),
            _ => {
                stmts.push(Stmt::new(line, StmtKind::Throw(e)));
                block(stmts)
            }
        };
    }
    let yields = matches!(body, ast::Expr::MatchExpr(_) | ast::Expr::BlockExpr(_)) || stmt::is_control_flow(body);
    let mut v = None;
    if yields {
        stmt::lower_to_dest(cx, body, &Dest::Yield, &mut stmts);
    } else {
        let mut pre = Vec::new();
        let e = expr::lower_expr(cx, body, &mut pre);
        stmts.extend(pre);
        v = Some(e);
    }
    if stmts.is_empty() && let Some(e) = v {
        return ArmBody::Expr(e);
    }
    if let Some(e) = v {
        stmts.push(Stmt::new(line, StmtKind::Raw(format!("yield {};", crate::emit::render_expr(&e)))));
    }
    block(stmts)
}

// --- switch labels --------------------------------------------------------------------------

fn can_switch(cx: &mut Cx, arms: &[ast::MatchArm], scrut_ty: &Ty) -> bool {
    if !selector_switchable(scrut_ty) {
        return false;
    }
    arms.iter().all(|a| {
        let Some(p) = a.pat() else { return false };
        if let Some(g) = a.guard()
            && (!guard_simple(&g) || constant_only(cx, &p))
        {
            // Java allows `when` only on patterns, not on constant labels.
            return false;
        }
        if is_binding_pat(cx, &p) {
            a.guard().is_none()
        } else {
            pattern_text(cx, &p, false).is_some()
        }
    })
}

fn selector_switchable(ty: &Ty) -> bool {
    match ty.strip_refs() {
        Ty::Int(i) => !i.is_long(),
        Ty::Char | Ty::Str | Ty::Result(..) | Ty::Adt { .. } => true,
        _ => false,
    }
}

// Java pattern text for a case label / component. `declare` is false for the feasibility check.
fn pattern_text(cx: &mut Cx, p: &ast::Pat, declare: bool) -> Option<String> {
    match p {
        ast::Pat::ParenPat(x) => pattern_text(cx, &x.pat()?, declare),
        ast::Pat::RefPat(x) => pattern_text(cx, &x.pat()?, declare),
        ast::Pat::LiteralPat(lp) => literal_label(cx, lp, p),
        ast::Pat::OrPat(op) => {
            let mut parts = Vec::new();
            for alt in op.pats() {
                parts.push(constant_label(cx, &alt)?);
            }
            (!parts.is_empty()).then(|| parts.join(", "))
        }
        ast::Pat::PathPat(pp) => {
            let def = resolve_path(cx, pp.path().as_ref()?)?;
            def_label(cx, p, def)
        }
        ast::Pat::IdentPat(ip) => def_label(cx, p, PathResolution::Def(bind_pat_def(cx, ip)?)),
        ast::Pat::TupleStructPat(tp) => {
            let v = variant_of(cx, tp.path().as_ref()?)?;
            let pats = positional_pats(tp, v.fields(cx.db).len());
            variant_label(cx, v, &pats, declare)
        }
        ast::Pat::RecordPat(rp) => {
            let v = variant_of(cx, rp.path().as_ref()?)?;
            let pats = record_pats(cx, rp, &v.fields(cx.db));
            variant_label(cx, v, &pats, declare)
        }
        _ => None,
    }
}

// A label that may appear in a multi-pattern case: literals and plain enum constants only.
fn constant_label(cx: &mut Cx, p: &ast::Pat) -> Option<String> {
    match p {
        ast::Pat::LiteralPat(lp) => literal_label(cx, lp, p),
        ast::Pat::PathPat(pp) => variant_constant(cx, resolve_path(cx, pp.path().as_ref()?)?),
        ast::Pat::IdentPat(ip) => {
            variant_constant(cx, PathResolution::Def(bind_pat_def(cx, ip)?))
        }
        ast::Pat::ParenPat(x) => constant_label(cx, &x.pat()?),
        _ => None,
    }
}

fn literal_label(cx: &mut Cx, lp: &ast::LiteralPat, p: &ast::Pat) -> Option<String> {
    let lit = lp.literal()?;
    if matches!(
        lit.kind(),
        ast::LiteralKind::Bool(_) | ast::LiteralKind::FloatNumber(_) | ast::LiteralKind::Byte(_)
    ) {
        return None;
    }
    let ty = cx.pat_ty(p);
    let mut e = expr::literal_expr(cx, &lit, &ty);
    if lp.minus_token().is_some() {
        e = Expr::Unary("-", Box::new(e));
    }
    Some(crate::emit::render_expr(&e))
}

// Patterns whose Java case label is a constant, which cannot carry a `when` guard.
fn constant_only(cx: &Cx, p: &ast::Pat) -> bool {
    match p {
        ast::Pat::LiteralPat(_) => true,
        ast::Pat::IdentPat(ip) => bind_pat_def(cx, ip).is_some_and(|d| !is_record_variant(cx, d)),
        ast::Pat::PathPat(pp) => pp
            .path()
            .and_then(|path| resolve_path(cx, &path))
            .is_some_and(|r| matches!(r, PathResolution::Def(d) if !is_record_variant(cx, d))),
        ast::Pat::OrPat(op) => op.pats().all(|p| constant_only(cx, &p)),
        ast::Pat::ParenPat(x) => x.pat().is_some_and(|i| constant_only(cx, &i)),
        _ => false,
    }
}

// A local data-enum variant gets a record pattern label (patterns may carry guards).
fn is_record_variant(cx: &Cx, def: ModuleDef) -> bool {
    match def {
        ModuleDef::EnumVariant(v) => {
            let e = v.parent_enum(cx.db);
            !local(cx, e.module(cx.db)) || !items::enum_is_plain(cx, e)
        }
        _ => false,
    }
}

// Unqualified enum constant for a multi-pattern case label.
fn variant_constant(cx: &Cx, res: PathResolution) -> Option<String> {
    let PathResolution::Def(ModuleDef::EnumVariant(v)) = res else { return None };
    let e = v.parent_enum(cx.db);
    (local(cx, e.module(cx.db)) && items::enum_is_plain(cx, e)).then(|| escape_keyword(v.name(cx.db).as_str()))
}

fn def_label(cx: &mut Cx, p: &ast::Pat, res: PathResolution) -> Option<String> {
    if matches!(cx.pat_ty(p).strip_refs(), Ty::Option(_)) {
        return None;
    }
    let PathResolution::Def(ModuleDef::EnumVariant(v)) = res else { return None };
    let e = v.parent_enum(cx.db);
    if local(cx, e.module(cx.db)) {
        if items::enum_is_plain(cx, e) {
            Some(escape_keyword(v.name(cx.db).as_str()))
        } else {
            Some(format!("{}()", items::variant_java_name(cx, v)))
        }
    } else if is_result(cx, e) {
        Some(format!("{}()", escape_keyword(v.name(cx.db).as_str())))
    } else {
        None
    }
}

fn variant_label(cx: &mut Cx, v: ra_ap_hir::EnumVariant, pats: &[FieldPat], declare: bool) -> Option<String> {
    let ctor = variant_ctor(cx, v)?;
    let e = v.parent_enum(cx.db);
    if local(cx, e.module(cx.db)) && items::enum_is_plain(cx, e) {
        return None;
    }
    let mut comps = Vec::new();
    for fp in pats {
        comps.push(component_text(cx, fp, declare)?);
    }
    Some(format!("{ctor}({})", comps.join(", ")))
}

// One component of a tuple/record pattern, plus the name a shorthand field binds.
#[derive(Clone, Default)]
struct FieldPat {
    pat: Option<ast::Pat>,
    bind: Option<String>,
}

fn component_text(cx: &mut Cx, fp: &FieldPat, declare: bool) -> Option<String> {
    if let Some(name) = &fp.bind {
        let java = if declare { cx.declare(name) } else { "x".to_string() };
        return Some(format!("var {java}"));
    }
    match &fp.pat {
        None | Some(ast::Pat::WildcardPat(_)) | Some(ast::Pat::RestPat(_)) => {
            let name = if declare { cx.fresh("unused") } else { "unused".to_string() };
            Some(format!("var {name}"))
        }
        Some(ast::Pat::IdentPat(ip)) if ip.pat().is_none() && bind_pat_def(cx, ip).is_none() => {
            let name = if declare { cx.declare(&ident_name(ip)) } else { "x".to_string() };
            Some(format!("var {name}"))
        }
        Some(ast::Pat::IdentPat(_)) => None,
        Some(ast::Pat::ParenPat(x)) => component_text(cx, &FieldPat { pat: x.pat(), bind: None }, declare),
        Some(ast::Pat::RefPat(x)) => component_text(cx, &FieldPat { pat: x.pat(), bind: None }, declare),
        Some(nested) => nested_component_text(cx, nested, declare),
    }
}

// Record patterns nest, but literals and enum constants are not patterns in Java.
fn nested_component_text(cx: &mut Cx, p: &ast::Pat, declare: bool) -> Option<String> {
    match p {
        ast::Pat::ParenPat(x) => nested_component_text(cx, &x.pat()?, declare),
        ast::Pat::RefPat(x) => nested_component_text(cx, &x.pat()?, declare),
        ast::Pat::TupleStructPat(tp) => {
            if matches!(cx.pat_ty(p).strip_refs(), Ty::Option(_)) {
                return None;
            }
            let v = variant_of(cx, tp.path().as_ref()?)?;
            variant_label(cx, v, &positional_pats(tp, v.fields(cx.db).len()), declare)
        }
        ast::Pat::RecordPat(rp) => {
            if matches!(cx.pat_ty(p).strip_refs(), Ty::Option(_)) {
                return None;
            }
            let v = variant_of(cx, rp.path().as_ref()?)?;
            variant_label(cx, v, &record_pats(cx, rp, &v.fields(cx.db)), declare)
        }
        _ => None,
    }
}

// Aligns a tuple-struct pattern's positional fields with the variant's declared fields.
fn positional_pats(tp: &ast::TupleStructPat, n: usize) -> Vec<FieldPat> {
    let mut pre: Vec<ast::Pat> = Vec::new();
    let mut post: Vec<ast::Pat> = Vec::new();
    let mut after_rest = false;
    for f in tp.fields() {
        if matches!(f, ast::Pat::RestPat(_)) {
            after_rest = true;
            continue;
        }
        if after_rest { post.push(f) } else { pre.push(f) }
    }
    let mut out: Vec<FieldPat> = vec![FieldPat::default(); n];
    for (i, f) in pre.into_iter().enumerate() {
        if i < n {
            out[i].pat = Some(f);
        }
    }
    let base = n.saturating_sub(post.len());
    for (j, f) in post.into_iter().enumerate() {
        if base + j < n {
            out[base + j].pat = Some(f);
        }
    }
    out
}

// Aligns `{ x, y, .. }` with the declared fields by name.
fn record_pats(cx: &Cx, rp: &ast::RecordPat, fields: &[ra_ap_hir::Field]) -> Vec<FieldPat> {
    let mut out: Vec<FieldPat> = vec![FieldPat::default(); fields.len()];
    let Some(list) = rp.record_pat_field_list() else { return out };
    for f in list.fields() {
        let Some(name) = f.name_ref().map(|n| n.text().to_string()) else { continue };
        let Some(i) = fields.iter().position(|d| d.name(cx.db).as_str() == name) else { continue };
        out[i] = match f.pat() {
            Some(sub) => FieldPat { pat: Some(sub), bind: None },
            None => FieldPat { pat: None, bind: Some(name) },
        };
    }
    out
}

fn variant_of(cx: &Cx, path: &ast::Path) -> Option<ra_ap_hir::EnumVariant> {
    match resolve_path(cx, path)? {
        PathResolution::Def(ModuleDef::EnumVariant(v)) => Some(v),
        _ => None,
    }
}

fn resolve_path<'db>(cx: &Cx<'_, 'db>, path: &ast::Path) -> Option<PathResolution<'db>> {
    cx.sema.resolve_path(path)
}

fn variant_ctor(cx: &Cx, v: ra_ap_hir::EnumVariant) -> Option<String> {
    let e = v.parent_enum(cx.db);
    if local(cx, e.module(cx.db)) {
        Some(items::variant_java_name(cx, v))
    } else if is_result(cx, e) {
        Some(escape_keyword(v.name(cx.db).as_str()))
    } else {
        None
    }
}

fn local(cx: &Cx, m: ra_ap_hir::Module) -> bool {
    m.krate(cx.db).origin(cx.db).is_local()
}

fn is_result(cx: &Cx, e: ra_ap_hir::Enum) -> bool {
    e.name(cx.db).as_str() == "Result" && !local(cx, e.module(cx.db))
}

// --- pattern lowering ------------------------------------------------------------------------

fn lower_pat(cx: &mut Cx, p: &ast::Pat, value: Expr, mode: Mode, out: &mut PatLowering) -> Vec<Expr> {
    match p {
        ast::Pat::WildcardPat(_) | ast::Pat::RestPat(_) => Vec::new(),
        ast::Pat::ParenPat(x) => x.pat().map(|i| lower_pat(cx, &i, value, mode, out)).unwrap_or_default(),
        ast::Pat::RefPat(x) => x.pat().map(|i| lower_pat(cx, &i, value, mode, out)).unwrap_or_default(),
        ast::Pat::IdentPat(ip) => {
            if let Some(def) = bind_pat_def(cx, ip) {
                return def_pat(cx, p, def, value, mode);
            }
            if mode != Mode::Test {
                let line = cx.line(p.syntax());
                let rust = ident_name(ip);
                let java = cx.declare(&rust);
                let ty = cx.pat_ty(p);
                out.binds.push(Bind { line, rust, java, value: value.clone(), ty, extracted: true });
            }
            match ip.pat() {
                Some(sub) => lower_pat(cx, &sub, value, mode, out),
                None => Vec::new(),
            }
        }
        ast::Pat::LiteralPat(lp) => {
            let ty = cx.pat_ty(p);
            match pattern_literal(cx, lp, &ty) {
                Some(lit) => vec![equality(value, lit, &ty)],
                None => Vec::new(),
            }
        }
        ast::Pat::RangePat(rp) => {
            let ty = cx.pat_ty(p);
            let mut conds = Vec::new();
            if let Some(bound) = rp.start() {
                match literal_bound(cx, &bound, &ty) {
                    Some(start) => conds.push(Expr::bin(value.clone(), ">=", start)),
                    None => {
                        cx.unsupported(bound.syntax(), "range bound");
                    }
                }
            }
            if let Some(bound) = rp.end() {
                match literal_bound(cx, &bound, &ty) {
                    Some(end) => {
                        let op = if rp.op_kind() == Some(ast::RangeOp::Inclusive) { "<=" } else { "<" };
                        conds.push(Expr::bin(value.clone(), op, end));
                    }
                    None => {
                        cx.unsupported(bound.syntax(), "range bound");
                    }
                }
            }
            conds
        }
        ast::Pat::OrPat(op) => {
            if mode != Mode::Test && op.pats().any(|p| binds_any(cx, &p)) {
                cx.unsupported(op.syntax(), "or-pattern that binds variables");
                return Vec::new();
            }
            let mut alts = Vec::new();
            for alt in op.pats() {
                let mut sub = PatLowering::default();
                let c = lower_pat(cx, &alt, value.clone(), mode, &mut sub);
                alts.push(and(c).unwrap_or(Expr::raw("true")));
            }
            or(alts).into_iter().collect()
        }
        ast::Pat::TuplePat(tp) => {
            let ty = cx.pat_ty(p);
            let arity = match ty.strip_refs() {
                Ty::Tuple(ts) => ts.len(),
                _ => tp.fields().count(),
            };
            let mut conds = Vec::new();
            for (i, f) in tp.fields().enumerate() {
                conds.extend(lower_pat(cx, &f, expr::tuple_field(value.clone(), i, arity), mode, out));
            }
            conds
        }
        ast::Pat::TupleStructPat(tp) => tuple_struct_pat(cx, p, tp, value, mode, out),
        ast::Pat::RecordPat(rp) => record_pat(cx, p, rp, value, mode, out),
        ast::Pat::PathPat(pp) => path_pat(cx, p, pp, value, mode),
        ast::Pat::SlicePat(sp) => slice_pat(cx, p, sp, value, out),
        _ => {
            cx.unsupported(p.syntax(), "pattern kind");
            Vec::new()
        }
    }
}

fn ident_name(ip: &ast::IdentPat) -> String {
    ip.name().map(|n| n.text().to_string()).unwrap_or_else(|| "_".into())
}

fn literal_bound(cx: &mut Cx, p: &ast::Pat, ty: &Ty) -> Option<Expr> {
    match p {
        ast::Pat::LiteralPat(lp) => pattern_literal(cx, lp, ty),
        _ => None,
    }
}

fn pattern_literal(cx: &mut Cx, lp: &ast::LiteralPat, ty: &Ty) -> Option<Expr> {
    let lit = lp.literal()?;
    let e = expr::literal_expr(cx, &lit, ty);
    Some(if lp.minus_token().is_some() { Expr::Unary("-", Box::new(e)) } else { e })
}

fn equality(value: Expr, lit: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Str => Expr::call(lit, "equals", vec![value]),
        Ty::Bool => match &lit {
            Expr::Raw(s) if s == "true" => value,
            Expr::Raw(s) if s == "false" => Expr::Unary("!", Box::new(value)),
            _ => Expr::bin(value, "==", lit),
        },
        _ => Expr::bin(value, "==", lit),
    }
}

fn tuple_struct_pat(cx: &mut Cx, p: &ast::Pat, tp: &ast::TupleStructPat, value: Expr, mode: Mode, out: &mut PatLowering) -> Vec<Expr> {
    let ty = cx.pat_ty(p);
    if let Ty::Option(_) = ty.strip_refs() {
        let mut conds = vec![Expr::call(value.clone(), "isPresent", vec![])];
        if let Some(inner) = tp.fields().next() {
            conds.extend(lower_pat(cx, &inner, Expr::call(value, "get", vec![]), mode, out));
        }
        return conds;
    }
    let Some(path) = tp.path() else {
        cx.unsupported(p.syntax(), "tuple-struct pattern without a path");
        return Vec::new();
    };
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Def(ModuleDef::EnumVariant(v))) => {
            let fields = v.fields(cx.db);
            let pats = positional_pats(tp, fields.len());
            variant_pat(cx, p, v, &pats, value, mode, out)
        }
        Some(PathResolution::Def(ModuleDef::Adt(Adt::Struct(s)))) => {
            let names = items::java_field_names(cx, &s.fields(cx.db));
            let owner = items::adt_java_name(cx, Adt::Struct(s));
            let mut conds = Vec::new();
            for (i, sub) in tp.fields().enumerate() {
                let Some(name) = names.get(i) else { continue };
                let v = struct_field(&value, &owner, name, &cx.pat_ty(&sub), mode);
                conds.extend(lower_pat(cx, &sub, v, mode, out));
            }
            conds
        }
        _ => {
            cx.unsupported(p.syntax(), "tuple-struct pattern");
            Vec::new()
        }
    }
}

fn record_pat(cx: &mut Cx, p: &ast::Pat, rp: &ast::RecordPat, value: Expr, mode: Mode, out: &mut PatLowering) -> Vec<Expr> {
    let Some(path) = rp.path() else {
        cx.unsupported(p.syntax(), "record pattern without a path");
        return Vec::new();
    };
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Def(ModuleDef::EnumVariant(v))) => {
            let pats = record_pats(cx, rp, &v.fields(cx.db));
            variant_pat(cx, p, v, &pats, value, mode, out)
        }
        Some(PathResolution::Def(ModuleDef::Adt(Adt::Struct(s)))) => {
            let decl = s.fields(cx.db);
            let names = items::java_field_names(cx, &decl);
            let owner = items::adt_java_name(cx, Adt::Struct(s));
            let mut conds = Vec::new();
            let Some(list) = rp.record_pat_field_list() else { return conds };
            for f in list.fields() {
                let name = f.name_ref().map(|n| n.text().to_string()).unwrap_or_default();
                let Some(i) = decl.iter().position(|d| d.name(cx.db).as_str() == name) else { continue };
                let Some(jname) = names.get(i) else { continue };
                let ty = field_ty(cx, &f).unwrap_or_else(|| cx.convert(&decl[i].ty(cx.db)));
                let field = struct_field(&value, &owner, jname, &ty, mode);
                match f.pat() {
                    Some(sub) => conds.extend(lower_pat(cx, &sub, field, mode, out)),
                    // `S { x }` binds the field to `x`.
                    None if mode != Mode::Test => {
                        let java = cx.declare(&name);
                        out.binds.push(Bind { line: cx.line(f.syntax()), rust: name, java, value: field, ty, extracted: true });
                    }
                    None => {}
                }
            }
            conds
        }
        _ => {
            cx.unsupported(p.syntax(), "record pattern");
            Vec::new()
        }
    }
}

fn field_ty(cx: &Cx, f: &ast::RecordPatField) -> Option<Ty> {
    let (_, ty) = cx.sema.resolve_record_pat_field(f)?;
    Some(cx.convert(&ty))
}

// One enum-variant arm: a record pattern in `if` conditions, type tests plus extraction elsewhere.
fn variant_pat(cx: &mut Cx, p: &ast::Pat, v: ra_ap_hir::EnumVariant, pats: &[FieldPat], value: Expr, mode: Mode, out: &mut PatLowering) -> Vec<Expr> {
    let Some(ctor) = variant_ctor(cx, v) else {
        cx.unsupported(p.syntax(), "pattern on a non-local enum");
        return Vec::new();
    };
    if mode == Mode::If {
        let mut comps = Vec::new();
        let mut conds = Vec::new();
        for fp in pats {
            comps.push(match component_text(cx, fp, true) {
                Some(text) => text,
                None => {
                    let part = cx.fresh("part");
                    if let Some(sub) = &fp.pat {
                        conds.extend(lower_pat(cx, sub, Expr::name(part.clone()), Mode::If, out));
                    }
                    format!("var {part}")
                }
            });
        }
        let mut all = vec![Expr::InstanceOf(Box::new(value), format!("{ctor}({})", comps.join(", ")))];
        all.append(&mut conds);
        return all;
    }
    let decl = v.fields(cx.db);
    let names = variant_field_names(cx, v);
    let mut conds = vec![Expr::InstanceOf(Box::new(value.clone()), ctor.clone())];
    for (i, fp) in pats.iter().enumerate() {
        let Some(name) = names.get(i) else { continue };
        let ty = match &fp.pat {
            Some(sub) => cx.pat_ty(sub),
            None => decl.get(i).map(|f| cx.convert(&f.ty(cx.db))).unwrap_or(Ty::Other("?".into())),
        };
        let raw = Expr::call(Expr::Cast(ctor.clone(), Box::new(value.clone())), name.clone(), vec![]);
        if let Some(sub) = &fp.pat {
            let v = if mode == Mode::Test { raw } else { Expr::Cast(ty.boxed(), Box::new(raw)) };
            conds.extend(lower_pat(cx, sub, v, mode, out));
        } else if let Some(bind) = &fp.bind
            && mode != Mode::Test
        {
            let java = cx.declare(bind);
            out.binds.push(Bind {
                line: cx.line(p.syntax()),
                rust: bind.clone(),
                java,
                value: Expr::Cast(ty.boxed(), Box::new(raw)),
                ty,
                extracted: true,
            });
        }
    }
    conds
}

// Java component names of a variant: `Result.java` calls them `value` / `error`.
fn variant_field_names(cx: &Cx, v: ra_ap_hir::EnumVariant) -> Vec<String> {
    let e = v.parent_enum(cx.db);
    if !local(cx, e.module(cx.db)) && is_result(cx, e) {
        return match v.name(cx.db).as_str() {
            "Ok" => vec!["value".to_string()],
            "Err" => vec!["error".to_string()],
            _ => Vec::new(),
        };
    }
    items::java_field_names(cx, &v.fields(cx.db))
}

fn struct_field(value: &Expr, owner: &str, name: &str, ty: &Ty, mode: Mode) -> Expr {
    if mode == Mode::Extract {
        Expr::Field(Box::new(Expr::Cast(owner.to_string(), Box::new(value.clone()))), name.to_string())
    } else {
        let _ = ty;
        Expr::Field(Box::new(value.clone()), name.to_string())
    }
}

fn path_pat(cx: &mut Cx, p: &ast::Pat, pp: &ast::PathPat, value: Expr, mode: Mode) -> Vec<Expr> {
    let Some(path) = pp.path() else {
        cx.unsupported(p.syntax(), "path pattern without a path");
        return Vec::new();
    };
    match cx.sema.resolve_path(&path) {
        Some(PathResolution::Def(def)) => def_pat(cx, p, def, value, mode),
        _ => {
            cx.unsupported(p.syntax(), "path pattern");
            Vec::new()
        }
    }
}

// Pattern naming a const / unit variant / unit struct rather than binding a value.
fn def_pat(cx: &mut Cx, p: &ast::Pat, def: ModuleDef, value: Expr, mode: Mode) -> Vec<Expr> {
    let _ = mode;
    let ty = cx.pat_ty(p);
    if let Ty::Option(_) = ty.strip_refs() {
        return vec![Expr::call(value, "isEmpty", vec![])];
    }
    match def {
        ModuleDef::EnumVariant(v) => {
            let e = v.parent_enum(cx.db);
            if local(cx, e.module(cx.db)) {
                if items::enum_is_plain(cx, e) {
                    vec![Expr::bin(value, "==", Expr::raw(items::variant_java_name(cx, v)))]
                } else {
                    vec![Expr::InstanceOf(Box::new(value), items::variant_java_name(cx, v))]
                }
            } else if is_result(cx, e) {
                vec![Expr::InstanceOf(Box::new(value), escape_keyword(v.name(cx.db).as_str()))]
            } else {
                cx.unsupported(p.syntax(), "pattern on a non-local enum");
                Vec::new()
            }
        }
        // Unit struct: exactly one value, nothing to test.
        ModuleDef::Adt(Adt::Struct(_)) => Vec::new(),
        ModuleDef::Const(c) => {
            let name = escape_keyword(c.name(cx.db).map(|n| n.as_str().to_string()).unwrap_or_default().as_str());
            let qualified = match def.module(cx.db) {
                Some(m) if m != cx.module => format!("{}.{name}", crate::frontend::module_class_name(cx.db, m)),
                _ => name,
            };
            vec![equality(value, Expr::raw(qualified), &ty)]
        }
        _ => {
            cx.unsupported(p.syntax(), "pattern naming a non-const item");
            Vec::new()
        }
    }
}

fn bind_pat_def(cx: &Cx, ip: &ast::IdentPat) -> Option<ModuleDef> {
    cx.sema.resolve_bind_pat_to_const(ip)
}

fn slice_pat(cx: &mut Cx, p: &ast::Pat, sp: &ast::SlicePat, value: Expr, out: &mut PatLowering) -> Vec<Expr> {
    let _ = p;
    let pats: Vec<ast::Pat> = sp.pats().collect();
    let rest = pats.iter().position(|p| is_rest(p));
    let fixed = pats.iter().filter(|p| !is_rest(p)).count();
    let size = || Expr::call(value.clone(), "size", vec![]);
    let mut conds = Vec::new();
    conds.push(Expr::bin(
        size(),
        if rest.is_some() { ">=" } else { "==" },
        Expr::raw(fixed.to_string()),
    ));
    let post = rest.map_or(0, |r| pats[r + 1..].iter().filter(|p| !is_rest(p)).count());
    let mut before = 0usize;
    for (i, sub) in pats.iter().enumerate() {
        if is_rest(sub) {
            continue;
        }
        let from_end = rest.is_some_and(|r| i > r);
        let idx = if from_end {
            Expr::bin(size(), "-", Expr::raw((post - (pats.len() - 1 - i)).to_string()))
        } else {
            let at = before;
            before += 1;
            Expr::raw(at.to_string())
        };
        let elem = Expr::call(value.clone(), "get", vec![idx]);
        conds.extend(lower_pat(cx, sub, elem, Mode::Extract, out));
    }
    // `rest @ ..`: a sublist view of the leftover elements.
    if let Some(r) = rest
        && let ast::Pat::IdentPat(ip) = &pats[r]
        && ip.pat().is_some()
    {
        let line = cx.line(ip.syntax());
        let rust = ident_name(ip);
        let java = cx.declare(&rust);
        let ty = cx.pat_ty(&pats[r]);
        let start = Expr::raw(pats[..r].iter().filter(|p| !is_rest(p)).count().to_string());
        let end = Expr::bin(size(), "-", Expr::raw(post.to_string()));
        out.binds.push(Bind {
            line,
            rust,
            java,
            value: Expr::call(value.clone(), "subList", vec![start, end]),
            ty,
            extracted: true,
        });
    }
    conds
}

fn is_rest(p: &ast::Pat) -> bool {
    match p {
        ast::Pat::RestPat(_) => true,
        ast::Pat::IdentPat(ip) => matches!(ip.pat(), Some(ast::Pat::RestPat(_))),
        _ => false,
    }
}

fn binds_any(cx: &Cx, p: &ast::Pat) -> bool {
    match p {
        ast::Pat::IdentPat(ip) => bind_pat_def(cx, ip).is_none(),
        ast::Pat::ParenPat(x) => x.pat().is_some_and(|i| binds_any(cx, &i)),
        ast::Pat::RefPat(x) => x.pat().is_some_and(|i| binds_any(cx, &i)),
        ast::Pat::TuplePat(tp) => tp.fields().any(|f| binds_any(cx, &f)),
        ast::Pat::TupleStructPat(tp) => tp.fields().any(|f| binds_any(cx, &f)),
        ast::Pat::RecordPat(rp) => rp.record_pat_field_list().is_some_and(|l| {
            l.fields().any(|f| match f.pat() {
                Some(p) => binds_any(cx, &p),
                None => f.name_ref().is_some(),
            })
        }),
        ast::Pat::SlicePat(sp) => sp.pats().any(|f| binds_any(cx, &f)),
        ast::Pat::OrPat(op) => op.pats().any(|f| binds_any(cx, &f)),
        _ => false,
    }
}

fn is_binding_pat(cx: &Cx, p: &ast::Pat) -> bool {
    match p {
        ast::Pat::WildcardPat(_) => true,
        ast::Pat::IdentPat(ip) => ip.pat().is_none() && bind_pat_def(cx, ip).is_none(),
        ast::Pat::ParenPat(x) => x.pat().is_some_and(|i| is_binding_pat(cx, &i)),
        ast::Pat::RefPat(x) => x.pat().is_some_and(|i| is_binding_pat(cx, &i)),
        _ => false,
    }
}

// Whether the pattern always matches (so it can be an `else`/`default`).
fn is_total(cx: &mut Cx, p: &ast::Pat) -> bool {
    match p {
        ast::Pat::WildcardPat(_) => true,
        ast::Pat::IdentPat(ip) => match bind_pat_def(cx, ip) {
            Some(ModuleDef::Adt(Adt::Struct(s))) => s.kind(cx.db) == StructKind::Unit,
            Some(_) => false,
            None => ip.pat().is_none() || ip.pat().is_some_and(|s| is_total(cx, &s)),
        },
        ast::Pat::ParenPat(x) => x.pat().is_some_and(|i| is_total(cx, &i)),
        ast::Pat::RefPat(x) => x.pat().is_some_and(|i| is_total(cx, &i)),
        ast::Pat::TuplePat(tp) => tp.fields().all(|f| is_total(cx, &f)),
        ast::Pat::TupleStructPat(tp) => {
            tp.path().is_some_and(|path| is_struct(cx, &path)) && tp.fields().all(|f| is_total(cx, &f))
        }
        ast::Pat::RecordPat(rp) => {
            rp.path().is_some_and(|path| is_struct(cx, &path))
                && rp.record_pat_field_list().is_some_and(|l| {
                    l.fields().all(|f| f.pat().is_none_or(|p| is_total(cx, &p)))
                })
        }
        ast::Pat::PathPat(pp) => pp.path().is_some_and(|path| match cx.sema.resolve_path(&path) {
            Some(PathResolution::Def(ModuleDef::Adt(Adt::Struct(s)))) => s.kind(cx.db) == StructKind::Unit,
            _ => false,
        }),
        ast::Pat::OrPat(op) => op.pats().any(|p| is_total(cx, &p)),
        _ => false,
    }
}

fn is_struct(cx: &Cx, path: &ast::Path) -> bool {
    matches!(cx.sema.resolve_path(path), Some(PathResolution::Def(ModuleDef::Adt(Adt::Struct(_)))))
}

// --- `?` ------------------------------------------------------------------------------------

pub fn lower_try(cx: &mut Cx, t: &ast::TryExpr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(t.syntax());
    let Some(inner) = t.expr() else {
        return cx.unsupported(t.syntax(), "`?` without an operand");
    };
    let ty = cx.ty(&inner);
    let v = expr::lower_expr(cx, &inner, out);
    match ty.strip_refs() {
        Ty::Result(ok, err) => try_result(cx, t, v, ok, err, line, out),
        Ty::Option(inner_ty) => try_option(cx, t, v, inner_ty, line, out),
        _ => cx.unsupported(t.syntax(), "`?` on a value that is not a Result or Option"),
    }
}

fn try_result(cx: &mut Cx, node: &ast::TryExpr, v: Expr, ok: &Ty, err: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    cx.needs_result = true;
    let want = match cx.ret_ty.as_ref().map(|t| t.strip_refs()) {
        Some(Ty::Result(_, e)) => Some((**e).clone()),
        _ => None,
    };
    let Some(want) = want else {
        return cx.unsupported(node.syntax(), "`?` outside a function returning Result");
    };
    let ok_name = cx.fresh("val");
    let err_name = cx.fresh("error");
    let tmp = cx.fresh("ok_value");
    let unit = ok.is_unit();
    if !unit {
        out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type_explicit(ok), name: tmp.clone(), init: None }));
    }
    let err_expr = if want == *err {
        Expr::New { ty: "Err<>".into(), args: vec![Expr::name(err_name.clone())] }
    } else if want.is_nameable() && !want.is_unit() && !matches!(want, Ty::Param(_)) {
        Expr::New {
            ty: "Err<>".into(),
            args: vec![Expr::call(Expr::raw(want.java()), "from", vec![Expr::name(err_name.clone())])],
        }
    } else {
        return cx.unsupported(node.syntax(), "`?` error type conversion");
    };
    let arms = vec![
        Arm {
            line,
            label: format!("Ok(var {ok_name})"),
            body: if unit {
                ArmBody::Block(Block::default())
            } else {
                ArmBody::Expr(Expr::Assign(Box::new(Expr::name(tmp.clone())), "=", Box::new(Expr::name(ok_name))))
            },
        },
        Arm {
            line,
            label: format!("Err(var {err_name})"),
            body: ArmBody::Block(Block {
                stmts: vec![Stmt::new(line, StmtKind::Return(Some(err_expr)))],
                close_line: line,
            }),
        },
    ];
    out.push(Stmt::new(line, StmtKind::Switch { scrut: v, arms }));
    if unit { Expr::raw("null") } else { Expr::name(tmp) }
}

fn try_option(cx: &mut Cx, node: &ast::TryExpr, v: Expr, inner_ty: &Ty, line: u32, out: &mut Vec<Stmt>) -> Expr {
    let is_option = matches!(cx.ret_ty.as_ref().map(|t| t.strip_refs()), Some(Ty::Option(_)));
    if !is_option {
        return cx.unsupported(node.syntax(), "`?` on Option outside a function returning Option");
    }
    // The operand is read twice (`.isEmpty()` and `.get()`).
    let v = if is_place(&v) {
        v
    } else {
        let tmp = cx.fresh("try_value");
        let ty = Ty::Option(Box::new(inner_ty.clone()));
        out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type(&ty), name: tmp.clone(), init: Some(v) }));
        Expr::name(tmp)
    };
    out.push(Stmt::new(
        line,
        StmtKind::If {
            cond: Expr::call(v.clone(), "isEmpty", vec![]),
            then: Block {
                stmts: vec![Stmt::new(
                    line,
                    StmtKind::Return(Some(Expr::call(Expr::raw("Optional"), "empty", vec![]))),
                )],
                close_line: line,
            },
            els: None,
        },
    ));
    if inner_ty.is_unit() {
        return Expr::raw("null");
    }
    let tmp = cx.fresh("some_value");
    out.push(Stmt::new(
        line,
        StmtKind::Local {
            ty: stmt::decl_type_explicit(inner_ty),
            name: tmp.clone(),
            init: Some(Expr::call(v, "get", vec![])),
        },
    ));
    Expr::name(tmp)
}

// --- helpers --------------------------------------------------------------------------------

fn place(cx: &mut Cx, e: &ast::Expr, out: &mut Vec<Stmt>) -> Expr {
    let line = cx.line(e.syntax());
    let ty = cx.ty(e);
    if ty.is_unit() {
        // Nothing to bind: keep the side effects of the expression.
        stmt::lower_to_dest(cx, e, &Dest::Discard, out);
        return Expr::raw("null");
    }
    let v = expr::lower_expr(cx, e, out);
    if is_place(&v) {
        return v;
    }
    let tmp = cx.fresh("m");
    out.push(Stmt::new(line, StmtKind::Local { ty: stmt::decl_type(&ty), name: tmp.clone(), init: Some(v) }));
    Expr::name(tmp)
}

fn emit_binds(binds: &[Bind], out: &mut Vec<Stmt>) {
    for b in binds.iter().filter(|b| b.extracted) {
        out.push(Stmt::new(
            b.line,
            StmtKind::Local { ty: stmt::decl_type(&b.ty), name: b.java.clone(), init: Some(b.value.clone()) },
        ));
    }
}

fn and(conds: Vec<Expr>) -> Option<Expr> {
    conds.into_iter().reduce(|a, b| Expr::bin(a, "&&", b))
}

fn or(conds: Vec<Expr>) -> Option<Expr> {
    conds.into_iter().reduce(|a, b| Expr::bin(a, "||", b))
}

fn guard_simple(g: &ast::MatchGuard) -> bool {
    !scan(g.syntax(), &|e| {
        matches!(
            e,
            ast::Expr::TryExpr(_)
                | ast::Expr::IfExpr(_)
                | ast::Expr::MatchExpr(_)
                | ast::Expr::BlockExpr(_)
                | ast::Expr::LoopExpr(_)
                | ast::Expr::WhileExpr(_)
                | ast::Expr::ForExpr(_)
                | ast::Expr::LetExpr(_)
        )
    })
}

fn arm_body_safe(body: &ast::Expr) -> bool {
    !scan(body.syntax(), &|e| {
        matches!(
            e,
            ast::Expr::ReturnExpr(_) | ast::Expr::BreakExpr(_) | ast::Expr::ContinueExpr(_) | ast::Expr::TryExpr(_)
        )
    })
}

// Whether any expression under `node` matches `pred`; closures are separate scopes.
fn scan(node: &SyntaxNode, pred: &dyn Fn(&ast::Expr) -> bool) -> bool {
    if ast::ClosureExpr::can_cast(node.kind()) {
        return false;
    }
    if let Some(e) = ast::Expr::cast(node.clone())
        && pred(&e)
    {
        return true;
    }
    node.children().any(|c| scan(&c, pred))
}
