use std::collections::BTreeMap;

use crate::jir::*;

// Line-aware printer: every node is placed on its Rust line; when Java runs long, output
// continues on the current line instead of shifting later lines.
#[derive(Default)]
pub struct Printer {
    out: String,
    line: u32,
    indent: usize,
    at_line_start: bool,
    comments: BTreeMap<u32, Vec<String>>,
    pub report: LineReport,
    // Collapse everything onto one line (for code moved out of its place).
    flat: bool,
}

// Statements printed, and (rust_line, java_line) for those that couldn't keep their line.
#[derive(Default, Debug)]
pub struct LineReport {
    pub stmts: u32,
    pub drift: Vec<(u32, u32)>,
}

pub fn print_file(
    file: &JavaFile,
    comments: BTreeMap<u32, Vec<String>>,
    trailer: &str,
    last_line: u32,
) -> (String, LineReport) {
    let mut p = Printer {
        line: 1,
        at_line_start: true,
        comments,
        ..Default::default()
    };
    if !file.package.is_empty() {
        p.word(&format!("package {};", file.package));
    }
    for imp in &file.imports {
        p.word(&format!("import {imp};"));
    }
    let c = &file.class;
    p.word(&format!("{} {{", c.header));
    p.indent += 1;
    for m in &c.members {
        p.member(m);
    }
    p.indent -= 1;
    p.goto(last_line.max(c.close_line));
    for t in &c.trailer {
        p.word(t);
    }
    if !trailer.is_empty() {
        p.word(trailer);
    }
    p.word("}");
    p.flush_comments_through(u32::MAX);
    p.out.push('\n');
    (p.out, p.report)
}

// Renders a standalone expression (for building raw statement text).
pub fn render_expr(e: &Expr) -> String {
    // Flat: line breaks inside raw text would shift every later statement off its Rust line.
    let mut p = Printer {
        flat: true,
        ..Default::default()
    };
    p.expr(e, 0);
    p.out
}

// Renders a whole member on one line, for code that must move out of its Rust line.
pub fn render_member_flat(m: &Member) -> String {
    let mut p = Printer {
        at_line_start: true,
        flat: true,
        ..Default::default()
    };
    p.member(m);
    p.out
}

// Simple mode: `var` where the initializer names the type (`var m = new HashMap<String, Integer>()`,
// `var n = Integer.parseInt(s)`). Only when the declared and constructed classes are the same, so
// later assignments keep compiling.
fn var_local(ty: &str, init: &Expr) -> Option<(String, Expr)> {
    match init {
        Expr::New { ty: new_ty, args } => {
            let class = new_ty.strip_suffix("<>")?;
            let targs = ty.strip_prefix(class)?;
            if !targs.starts_with('<') {
                return None;
            }
            Some((
                "var".into(),
                Expr::New {
                    ty: format!("{class}{targs}"),
                    args: args.clone(),
                },
            ))
        }
        Expr::Call {
            recv: None, name, ..
        } if matches!(
            (ty, name.as_str()),
            ("int", "Integer.parseInt" | "Integer.parseUnsignedInt")
                | ("long", "Long.parseLong")
                | ("double", "Double.parseDouble")
        ) =>
        {
            Some(("var".into(), init.clone()))
        }
        _ => None,
    }
}

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Lambda { .. } => 0,
        Expr::Assign(..) => 1,
        Expr::Ternary(..) | Expr::Switch { .. } => 2,
        Expr::Binary(_, op, _) => binop_prec(op),
        Expr::InstanceOf(..) => 9,
        Expr::Cast(..) | Expr::Unary(..) => 13,
        _ => 15,
    }
}

fn binop_prec(op: &str) -> u8 {
    match op {
        "||" => 3,
        "&&" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "==" | "!=" => 8,
        "<" | ">" | "<=" | ">=" => 9,
        "<<" | ">>" | ">>>" => 10,
        "+" | "-" => 11,
        _ => 12,
    }
}

impl Printer {
    fn word(&mut self, s: &str) {
        if self.at_line_start {
            for _ in 0..self.indent {
                self.out.push_str("    ");
            }
            self.at_line_start = false;
        } else if !self.out.is_empty() && !self.out.ends_with([' ', '(', '\n']) {
            self.out.push(' ');
        }
        self.push(s);
    }

    // Raw text that may contain newlines (keeps the line counter right).
    fn push(&mut self, s: &str) {
        for (i, part) in s.split('\n').enumerate() {
            if i > 0 {
                self.flush_comments_through(self.line);
                self.out.push('\n');
                self.line += 1;
            }
            self.out.push_str(part);
        }
    }

    fn newline(&mut self) {
        self.flush_comments_through(self.line);
        self.out.push('\n');
        self.line += 1;
        self.at_line_start = true;
    }

    fn flush_comments_through(&mut self, line: u32) {
        let keys: Vec<u32> = self.comments.range(..=line).map(|(k, _)| *k).collect();
        for k in keys {
            for c in self.comments.remove(&k).unwrap() {
                self.word(&c);
            }
        }
    }

    pub fn goto(&mut self, line: u32) {
        if self.flat {
            return;
        }
        while self.line < line {
            self.newline();
        }
    }

    fn member(&mut self, m: &Member) {
        match m {
            Member::Field { line, text } | Member::Raw { line, text } => {
                self.goto(*line);
                self.word(text);
            }
            Member::Method(m) => {
                self.goto(m.line);
                match &m.body {
                    None => self.word(&format!("{};", m.header)),
                    Some(b) => {
                        self.word(&m.header);
                        self.block(b);
                    }
                }
            }
            Member::Class(c) => {
                self.goto(c.line);
                self.word(&format!("{} {{", c.header));
                self.indent += 1;
                for m in &c.members {
                    self.member(m);
                }
                self.indent -= 1;
                self.goto(c.close_line);
                for t in &c.trailer {
                    self.word(t);
                }
                self.word("}");
            }
        }
    }

    fn block(&mut self, b: &Block) {
        self.word("{");
        self.indent += 1;
        for s in &b.stmts {
            self.stmt(s);
        }
        self.indent -= 1;
        self.goto(b.close_line);
        self.word("}");
    }

    fn label(&mut self, l: &Option<String>) {
        if let Some(l) = l {
            self.word(&format!("{l}:"));
        }
    }

    pub fn stmt(&mut self, s: &Stmt) {
        self.goto(s.line);
        self.report.stmts += 1;
        if self.line != s.line {
            self.report.drift.push((s.line, self.line));
        }
        match &s.kind {
            StmtKind::Local { ty, name, init } => match init {
                Some(e) => {
                    let (ty, e) = match crate::mode::simple().then(|| var_local(ty, e)).flatten() {
                        Some((t, e)) => (t, self.expr_str(&e, 0)),
                        None => (ty.clone(), self.expr_str(e, 0)),
                    };
                    self.word(&format!("{ty} {name} = {e};"));
                }
                None => self.word(&format!("{ty} {name};")),
            },
            StmtKind::Expr(e) => {
                let e = self.expr_str(e, 0);
                self.word(&format!("{e};"));
            }
            StmtKind::If { cond, then, els } => {
                let c = self.expr_str(cond, 0);
                self.word(&format!("if ({c})"));
                self.block(then);
                if let Some(els) = els {
                    self.word("else");
                    match &els.kind {
                        StmtKind::Block(b) => self.block(b),
                        _ => self.stmt(els),
                    }
                }
            }
            StmtKind::While { label, cond, body } => {
                self.label(label);
                let c = self.expr_str(cond, 0);
                self.word(&format!("while ({c})"));
                self.block(body);
            }
            StmtKind::For {
                label,
                header,
                body,
            } => {
                self.label(label);
                self.word(&format!("for ({header})"));
                self.block(body);
            }
            StmtKind::ForEach {
                label,
                ty,
                name,
                iter,
                body,
            } => {
                self.label(label);
                let it = self.expr_str(iter, 0);
                self.word(&format!("for ({ty} {name} : {it})"));
                self.block(body);
            }
            StmtKind::Switch { scrut, arms } => {
                let sc = self.expr_str(scrut, 0);
                self.word(&format!("switch ({sc}) {{"));
                self.indent += 1;
                for a in arms {
                    self.arm(a, false);
                }
                self.indent -= 1;
                self.word("}");
            }
            StmtKind::Return(None) => self.word("return;"),
            StmtKind::Return(Some(e)) => {
                let e = self.expr_str(e, 0);
                self.word(&format!("return {e};"));
            }
            StmtKind::Break(l) => match l {
                Some(l) => self.word(&format!("break {l};")),
                None => self.word("break;"),
            },
            StmtKind::Continue(l) => match l {
                Some(l) => self.word(&format!("continue {l};")),
                None => self.word("continue;"),
            },
            StmtKind::Throw(e) => {
                let e = self.expr_str(e, 0);
                self.word(&format!("throw {e};"));
            }
            StmtKind::Try {
                body,
                decl,
                handler,
            } => {
                self.word("try");
                self.block(body);
                self.word(&format!("catch ({decl})"));
                self.block(handler);
            }
            StmtKind::Block(b) => self.block(b),
            StmtKind::Labeled { label, body } => {
                self.word(&format!("{label}:"));
                self.block(body);
            }
            StmtKind::Raw(t) => self.word(t),
        }
    }

    fn arm(&mut self, a: &Arm, is_expr: bool) {
        self.goto(a.line);
        let label = if a.label == "default" {
            "default".to_string()
        } else {
            format!("case {}", a.label)
        };
        match &a.body {
            ArmBody::Expr(e) => {
                let e = self.expr_str(e, 0);
                self.word(&format!("{label} -> {e};"));
            }
            ArmBody::Throw(e) => {
                let e = self.expr_str(e, 0);
                self.word(&format!("{label} -> throw {e};"));
            }
            ArmBody::Block(b) => {
                self.word(&format!("{label} ->"));
                self.block(b);
            }
        }
        let _ = is_expr;
    }

    // Renders an expression; nested blocks (lambdas, switch expressions) advance lines as needed.
    fn expr_str(&mut self, e: &Expr, min_prec: u8) -> String {
        let mut sub = Printer {
            line: self.line,
            indent: self.indent,
            flat: self.flat,
            ..Default::default()
        };
        sub.expr(e, min_prec);
        // Newlines inside the rendered text are counted again when the caller pushes it.
        sub.out
    }

    fn expr(&mut self, e: &Expr, min_prec: u8) {
        let p = prec(e);
        let paren = p < min_prec;
        if paren {
            self.out.push('(');
        }
        match e {
            Expr::Raw(s) | Expr::Name(s) => self.push(s),
            Expr::Field(r, f) => {
                self.expr(r, 15);
                self.out.push('.');
                self.out.push_str(f);
            }
            Expr::Call { recv, name, args } => {
                if let Some(r) = recv {
                    self.expr(r, 15);
                    self.out.push('.');
                }
                self.out.push_str(name);
                self.args(args);
            }
            Expr::New { ty, args } => {
                self.out.push_str("new ");
                self.out.push_str(ty);
                self.args(args);
            }
            Expr::Unary(op, x) => {
                self.out.push_str(op);
                // 14 forces parens around nested unary (`- -1`, not `--1`).
                self.expr(x, 14);
            }
            Expr::Binary(l, op, r) => {
                let bp = binop_prec(op);
                self.expr(l, bp);
                self.out.push_str(&format!(" {op} "));
                self.expr(r, bp + 1);
            }
            Expr::Assign(l, op, r) => {
                self.expr(l, 15);
                self.out.push_str(&format!(" {op} "));
                self.expr(r, 1);
            }
            Expr::Ternary(c, a, b) => {
                self.expr(c, 3);
                self.out.push_str(" ? ");
                self.expr(a, 3);
                self.out.push_str(" : ");
                self.expr(b, 2);
            }
            Expr::Cast(t, x) => {
                self.out.push_str(&format!("({t}) "));
                self.expr(x, 13);
            }
            Expr::Lambda { params, body } => {
                match params.len() {
                    1 if !params[0].contains(' ') => self.out.push_str(&params[0]),
                    _ => self.out.push_str(&format!("({})", params.join(", "))),
                }
                self.out.push_str(" -> ");
                match body {
                    LambdaBody::Expr(x) => self.expr(x, 0),
                    LambdaBody::Block(b) => self.inline_block(b),
                }
            }
            Expr::InstanceOf(x, pat) => {
                self.expr(x, 10);
                self.out.push_str(" instanceof ");
                self.out.push_str(pat);
            }
            Expr::Switch { scrut, arms } => {
                self.out.push_str("switch (");
                self.expr(scrut, 0);
                self.out.push_str(") {");
                self.indent += 1;
                for a in arms {
                    self.at_line_start = false;
                    self.arm(a, true);
                }
                self.indent -= 1;
                self.word("}");
            }
        }
        if paren {
            self.out.push(')');
        }
    }

    fn args(&mut self, args: &[Expr]) {
        self.out.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.out.push_str(", ");
            }
            self.expr(a, 0);
        }
        self.out.push(')');
    }

    fn inline_block(&mut self, b: &Block) {
        self.at_line_start = false;
        self.block(b);
    }
}
