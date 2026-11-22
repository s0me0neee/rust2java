// `usize`/`isize` are Java ints, but Rust's are 64-bit: a local that only accumulates, compares,
// prints or widens is declared `long` so totals past 2^31 don't wrap. Index-like locals stay `int`.

use std::collections::{HashMap, HashSet};

use ra_ap_hir::{Local, PathResolution};
use ra_ap_syntax::{AstNode, SyntaxKind, SyntaxNode, ast, ast::HasName};

use super::Cx;
use crate::ty::{Int, Ty};

pub struct Wide<'db> {
    pub locals: HashSet<Local<'db>>,
    // `*`/`<<` whose left operand gets `(long)`, and `sum`/`product` calls summed as long.
    pub ops: HashSet<SyntaxNode>,
}

const ARITH_TOKENS: &[SyntaxKind] = &[
    SyntaxKind::WHITESPACE,
    SyntaxKind::STRING,
    SyntaxKind::INT_NUMBER,
    SyntaxKind::FLOAT_NUMBER,
    SyntaxKind::AS_KW,
    SyntaxKind::PLUS,
    SyntaxKind::MINUS,
    SyntaxKind::STAR,
    SyntaxKind::SLASH,
    SyntaxKind::PERCENT,
    SyntaxKind::AMP,
    SyntaxKind::PIPE,
    SyntaxKind::CARET,
    SyntaxKind::L_ANGLE,
    SyntaxKind::R_ANGLE,
    SyntaxKind::EQ,
    SyntaxKind::L_PAREN,
    SyntaxKind::R_PAREN,
];

const FORMAT_MACROS: &[&str] = &[
    "println", "print", "eprintln", "eprint", "format", "write", "writeln",
];

enum Use<'db> {
    Safe,
    Unsafe,
    // Flows into another candidate: safe while that one stays wide.
    Into(Local<'db>),
}

pub fn analyze<'db>(cx: &Cx<'_, 'db>, root: &SyntaxNode) -> Wide<'db> {
    let mut cands: HashMap<Local<'db>, (String, Vec<ast::Expr>)> = HashMap::new();
    let mut grows: HashSet<Local<'db>> = HashSet::new();
    for l in root.descendants().filter_map(ast::LetStmt::cast) {
        let Some(ast::Pat::IdentPat(ip)) = l.pat() else {
            continue;
        };
        if ip.pat().is_some() || ip.ref_token().is_some() || l.let_else().is_some() {
            continue;
        }
        if !is_word(&cx.pat_ty(&ast::Pat::IdentPat(ip.clone()))) {
            continue;
        }
        let (Some(local), Some(name)) = (cx.sema.to_def(&ip), ip.name()) else {
            continue;
        };
        if l.initializer().is_some_and(|i| grows_expr(&i)) {
            grows.insert(local);
        }
        cands.insert(
            local,
            (
                name.text().to_string(),
                l.initializer().into_iter().collect(),
            ),
        );
    }
    if cands.is_empty() {
        return Wide {
            locals: HashSet::new(),
            ops: HashSet::new(),
        };
    }
    let mut uses: HashMap<Local<'db>, Vec<Use<'db>>> = HashMap::new();
    for pe in root.descendants().filter_map(ast::PathExpr::cast) {
        let Some(PathResolution::Local(local)) = pe.path().and_then(|p| cx.sema.resolve_path(&p))
        else {
            continue;
        };
        if !cands.contains_key(&local) {
            continue;
        }
        let u = classify(cx, &pe, &mut cands, &mut grows, local);
        uses.entry(local).or_default().push(u);
    }
    // Macro arguments aren't resolved: only plain arithmetic in a formatting macro's argument is safe.
    let mut tainted: HashSet<String> = HashSet::new();
    for mc in root.descendants().filter_map(ast::MacroCall::cast) {
        let name = mc
            .path()
            .and_then(|p| p.segment())
            .map(|s| s.syntax().text().to_string())
            .unwrap_or_default();
        let Some(tt) = mc.token_tree() else { continue };
        let format = FORMAT_MACROS.contains(&name.as_str());
        let mut arg: Vec<ra_ap_syntax::SyntaxToken> = Vec::new();
        let mut flush = |arg: &mut Vec<ra_ap_syntax::SyntaxToken>| {
            let safe = format
                && arg.iter().enumerate().all(|(i, t)| {
                    let next = arg[i + 1..]
                        .iter()
                        .find(|n| n.kind() != SyntaxKind::WHITESPACE);
                    match t.kind() {
                        SyntaxKind::IDENT => next.is_none_or(|n| n.kind() != SyntaxKind::L_PAREN),
                        k => ARITH_TOKENS.contains(&k),
                    }
                });
            if !safe {
                for t in arg.iter().filter(|t| t.kind() == SyntaxKind::IDENT) {
                    tainted.insert(t.text().to_string());
                }
            }
            arg.clear();
        };
        for el in tt.syntax().children_with_tokens() {
            match el {
                ra_ap_syntax::NodeOrToken::Token(t) if t.kind() == SyntaxKind::COMMA => {
                    flush(&mut arg)
                }
                ra_ap_syntax::NodeOrToken::Token(t)
                    if matches!(t.kind(), SyntaxKind::L_PAREN | SyntaxKind::R_PAREN)
                        && t.parent().as_ref() == Some(tt.syntax()) => {}
                ra_ap_syntax::NodeOrToken::Token(t) => arg.push(t),
                ra_ap_syntax::NodeOrToken::Node(n) => {
                    arg.extend(n.descendants_with_tokens().filter_map(|e| e.into_token()))
                }
            }
        }
        flush(&mut arg);
    }
    let mut wide: HashSet<Local<'db>> = cands
        .iter()
        .filter(|(l, (name, _))| grows.contains(l) && !tainted.contains(name))
        .map(|(l, _)| *l)
        .collect();
    // A wide value can't flow into a local that stays `int`.
    loop {
        let snapshot = wide.clone();
        wide.retain(|l| {
            uses.get(l).is_none_or(|us| {
                us.iter().all(|u| match u {
                    Use::Safe => true,
                    Use::Unsafe => false,
                    Use::Into(t) => snapshot.contains(t),
                })
            })
        });
        if wide.len() == snapshot.len() {
            break;
        }
    }
    let mut ops = HashSet::new();
    for l in &wide {
        for rhs in &cands[l].1 {
            collect_ops(cx, &wide, rhs, &mut ops);
        }
    }
    Wide { locals: wide, ops }
}

fn is_word(t: &Ty) -> bool {
    matches!(t.strip_refs(), Ty::Int(Int::Usize | Int::Isize))
}

// Follows a use up through arithmetic to the place its value lands.
fn classify<'db>(
    cx: &Cx<'_, 'db>,
    pe: &ast::PathExpr,
    cands: &mut HashMap<Local<'db>, (String, Vec<ast::Expr>)>,
    grows: &mut HashSet<Local<'db>>,
    local: Local<'db>,
) -> Use<'db> {
    // A closure needs effectively-final captures and boxes its values: keep those `int`.
    if pe
        .syntax()
        .ancestors()
        .any(|a| a.kind() == SyntaxKind::CLOSURE_EXPR)
    {
        return Use::Unsafe;
    }
    let mut e = pe.syntax().clone();
    loop {
        let Some(parent) = e.parent() else {
            return Use::Unsafe;
        };
        if ast::ParenExpr::can_cast(parent.kind()) {
            e = parent;
            continue;
        }
        if let Some(p) = ast::PrefixExpr::cast(parent.clone()) {
            if p.op_kind() == Some(ast::UnaryOp::Neg) {
                e = parent;
                continue;
            }
            return Use::Unsafe;
        }
        if ast::ExprStmt::can_cast(parent.kind()) {
            return Use::Safe;
        }
        if let Some(c) = ast::CastExpr::cast(parent.clone()) {
            let to = c.ty().map(|t| cx.type_ref(&t));
            let ok = matches!(to, Some(Ty::Int(i)) if i.is_long())
                || to.as_ref().is_some_and(Ty::is_float);
            return if ok { Use::Safe } else { Use::Unsafe };
        }
        if let Some(m) = ast::MethodCallExpr::cast(parent.clone()) {
            let receiver = m.receiver().is_some_and(|r| r.syntax() == &e);
            let name = m
                .name_ref()
                .map(|n| n.text().to_string())
                .unwrap_or_default();
            return if receiver && name == "to_string" {
                Use::Safe
            } else {
                Use::Unsafe
            };
        }
        if let Some(l) = ast::LetStmt::cast(parent.clone()) {
            let into = match l.pat() {
                Some(ast::Pat::IdentPat(ip)) => cx.sema.to_def(&ip),
                _ => None,
            };
            return match into {
                Some(t) if cands.contains_key(&t) => Use::Into(t),
                _ => Use::Unsafe,
            };
        }
        let Some(b) = ast::BinExpr::cast(parent.clone()) else {
            return Use::Unsafe;
        };
        let is_lhs = b.lhs().is_some_and(|l| l.syntax() == &e);
        match b.op_kind() {
            Some(ast::BinaryOp::ArithOp(_)) => e = parent,
            Some(ast::BinaryOp::CmpOp(_)) => return Use::Safe,
            Some(ast::BinaryOp::Assignment { op }) => {
                if is_lhs {
                    // Assigning to the local itself: its new value must be widened too.
                    if e != *pe.syntax() {
                        return Use::Unsafe;
                    }
                    let literal = matches!(b.rhs(), Some(ast::Expr::Literal(_)));
                    if matches!(op, Some(ast::ArithOp::Mul | ast::ArithOp::Shl))
                        || (op == Some(ast::ArithOp::Add) && !literal)
                    {
                        grows.insert(local);
                    }
                    if let Some(r) = b.rhs() {
                        if grows_expr(&r) {
                            grows.insert(local);
                        }
                        cands.get_mut(&local).unwrap().1.push(r);
                    }
                    return Use::Safe;
                }
                let target = match b.lhs() {
                    Some(ast::Expr::PathExpr(t)) => {
                        match t.path().and_then(|p| cx.sema.resolve_path(&p)) {
                            Some(PathResolution::Local(t)) => Some(t),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                return match target {
                    Some(t) if cands.contains_key(&t) => Use::Into(t),
                    _ => Use::Unsafe,
                };
            }
            _ => return Use::Unsafe,
        }
    }
}

// A value that can outgrow its operands: a product, shift or iterator sum.
fn grows_expr(e: &ast::Expr) -> bool {
    match e {
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|i| grows_expr(&i)),
        ast::Expr::BinExpr(b) => {
            matches!(
                b.op_kind(),
                Some(ast::BinaryOp::ArithOp(
                    ast::ArithOp::Mul | ast::ArithOp::Shl
                ))
            ) || b.lhs().is_some_and(|l| grows_expr(&l))
                || b.rhs().is_some_and(|r| grows_expr(&r))
        }
        ast::Expr::MethodCallExpr(m) => m
            .name_ref()
            .is_some_and(|n| matches!(n.text().to_string().as_str(), "sum" | "product")),
        _ => false,
    }
}

fn collect_ops(cx: &Cx, locals: &HashSet<Local>, e: &ast::Expr, ops: &mut HashSet<SyntaxNode>) {
    match e {
        ast::Expr::ParenExpr(p) => {
            if let Some(i) = p.expr() {
                collect_ops(cx, locals, &i, ops);
            }
        }
        ast::Expr::PrefixExpr(p) => {
            if let Some(i) = p.expr() {
                collect_ops(cx, locals, &i, ops);
            }
        }
        ast::Expr::BinExpr(b) => {
            if let Some(ast::BinaryOp::ArithOp(op)) = b.op_kind() {
                for side in [b.lhs(), b.rhs()].into_iter().flatten() {
                    collect_ops(cx, locals, &side, ops);
                }
                if matches!(op, ast::ArithOp::Mul | ast::ArithOp::Shl)
                    && is_word(&cx.ty(&ast::Expr::BinExpr(b.clone())))
                    && !b.lhs().is_some_and(|l| is_long(cx, locals, ops, &l))
                {
                    ops.insert(b.syntax().clone());
                }
            }
        }
        ast::Expr::MethodCallExpr(m)
            if m.name_ref()
                .is_some_and(|n| matches!(n.text().to_string().as_str(), "sum" | "product")) =>
        {
            ops.insert(m.syntax().clone());
        }
        _ => {}
    }
}

// Whether `e` already evaluates as a Java long under the widening: Java promotes the rest.
pub fn is_long(cx: &Cx, locals: &HashSet<Local>, ops: &HashSet<SyntaxNode>, e: &ast::Expr) -> bool {
    match e {
        ast::Expr::ParenExpr(p) => p.expr().is_some_and(|i| is_long(cx, locals, ops, &i)),
        ast::Expr::PrefixExpr(p) => p.expr().is_some_and(|i| is_long(cx, locals, ops, &i)),
        ast::Expr::PathExpr(pe) => matches!(
            pe.path().and_then(|p| cx.sema.resolve_path(&p)),
            Some(PathResolution::Local(l)) if locals.contains(&l)
        ),
        ast::Expr::BinExpr(b) if matches!(b.op_kind(), Some(ast::BinaryOp::ArithOp(_))) => {
            ops.contains(b.syntax())
                || [b.lhs(), b.rhs()]
                    .into_iter()
                    .flatten()
                    .any(|s| is_long(cx, locals, ops, &s))
        }
        ast::Expr::MethodCallExpr(m) => ops.contains(m.syntax()),
        _ => false,
    }
}
