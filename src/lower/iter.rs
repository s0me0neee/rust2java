// `for` loops, ranges and iterator chains (-> counted loops / java.util.stream).

use ra_ap_syntax::{AstNode, ast, ast::HasLoopBody, ast::HasName};

use super::stmt::{self, Dest};
use super::{Cx, LoopFrame, expr};
use crate::jir::{Expr, Stmt, StmtKind};
use crate::ty::Ty;

pub fn lower_for(cx: &mut Cx, f: &ast::ForExpr, out: &mut Vec<Stmt>) {
    let line = cx.line(f.syntax());
    let (Some(p), Some(iterable)) = (f.pat(), f.iterable()) else {
        let e = cx.unsupported(f.syntax(), "for loop");
        out.push(Stmt::new(line, StmtKind::Expr(e)));
        return;
    };
    let rust_label = f.label().and_then(|l| l.lifetime()).map(|l| l.text().to_string());
    cx.loops.push(LoopFrame {
        java_label: rust_label.as_deref().map(stmt::java_label_name),
        rust_label,
        value_var: None,
        switch_depth: cx.switch_depth,
        is_block: false,
    });
    cx.push_scope();
    let kind = match &iterable {
        ast::Expr::RangeExpr(r) if simple_ident(&p).is_some() => {
            let (start, end, inclusive) = range_bounds(cx, r, out);
            let var = cx.declare(&simple_ident(&p).unwrap());
            let ty = cx.pat_ty(&p);
            let jt = ty.java();
            let cmp = if inclusive { "<=" } else { "<" };
            let header = format!(
                "{jt} {var} = {}; {var} {cmp} {}; {var}++",
                crate::emit::render_expr(&start),
                crate::emit::render_expr(&end)
            );
            let body = f.loop_body().map(|b| stmt::lower_block(cx, &b, &Dest::Discard)).unwrap_or_default();
            Some((header, body))
        }
        _ => None,
    };
    let frame_label = |cx: &mut Cx| cx.loops.pop().unwrap().java_label;
    match kind {
        Some((header, body)) => {
            cx.pop_scope();
            let label = frame_label(cx);
            out.push(Stmt::new(line, StmtKind::For { label, header, body }));
        }
        None => {
            // Generic: iterate a Java Iterable (collections) or a stream's iterator.
            let it_ty = cx.ty(&iterable);
            let src = expr::lower_expr(cx, &iterable, out);
            let elem_ty = cx.pat_ty(&p);
            let (name, binds) = match simple_ident(&p) {
                Some(n) => (cx.declare(&n), Vec::new()),
                None => {
                    let tmp = cx.fresh("item");
                    let mut binds = Vec::new();
                    super::pat::bind_pattern(cx, &p, Expr::name(tmp.clone()), &mut binds);
                    (tmp, binds)
                }
            };
            let mut body = f.loop_body().map(|b| stmt::lower_block(cx, &b, &Dest::Discard)).unwrap_or_default();
            cx.pop_scope();
            let label = frame_label(cx);
            let mut stmts = binds;
            stmts.append(&mut body.stmts);
            body.stmts = stmts;
            let iter = iterable_expr(src, &it_ty);
            out.push(Stmt::new(
                line,
                StmtKind::ForEach { label, ty: stmt::decl_type(&elem_ty), name, iter, body },
            ));
        }
    }
}

// Something usable on the right of `for (x : ...)`.
fn iterable_expr(src: Expr, ty: &Ty) -> Expr {
    match ty.strip_refs() {
        Ty::Iter(t) => Expr::Cast(format!("Iterable<{}>", t.boxed()), Box::new(Expr::raw(format!(
            "{}::iterator",
            crate::emit::render_expr(&src)
        )))),
        Ty::Map { .. } => Expr::call(src, "entrySet", vec![]),
        _ => src,
    }
}

fn simple_ident(p: &ast::Pat) -> Option<String> {
    match p {
        ast::Pat::IdentPat(ip) if ip.pat().is_none() => ip.name().map(|n| n.text().to_string()),
        _ => None,
    }
}

fn range_bounds(cx: &mut Cx, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> (Expr, Expr, bool) {
    use ast::RangeItem;
    let start = r.start().map(|s| expr::lower_expr(cx, &s, out)).unwrap_or(Expr::raw("0"));
    let end = r.end().map(|e| expr::lower_expr(cx, &e, out)).unwrap_or(Expr::raw("Integer.MAX_VALUE"));
    let inclusive = r.op_kind() == Some(ast::RangeOp::Inclusive);
    (start, end, inclusive)
}

// A range used as a value (not directly in `for`): an IntStream.
pub fn lower_range_expr(cx: &mut Cx, r: &ast::RangeExpr, out: &mut Vec<Stmt>) -> Expr {
    let (s, e, inclusive) = range_bounds(cx, r, out);
    let ty = cx.ty(&ast::Expr::RangeExpr(r.clone()));
    let long = matches!(ty, Ty::Iter(ref t) if matches!(**t, Ty::Int(i) if i.is_long()));
    let class = if long { "LongStream" } else { "IntStream" };
    let m = if inclusive { "rangeClosed" } else { "range" };
    Expr::call(Expr::raw(class), m, vec![s, e])
}

// Iterator adaptors / consumers. Returns None when the method isn't an iterator method.
pub fn lower_iter_method(
    cx: &mut Cx,
    m: &ast::MethodCallExpr,
    recv_ty: &Ty,
    key: &str,
    out: &mut Vec<Stmt>,
) -> Option<Expr> {
    let _ = (cx, m, recv_ty, key, out);
    None
}
