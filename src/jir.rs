// Java IR: just enough structure to print Java with Rust line numbers.

#[derive(Clone, Debug)]
pub enum Expr {
    // Already-rendered Java treated as a primary expression.
    Raw(String),
    Name(String),
    Field(Box<Expr>, String),
    // `recv.name(args)` or `name(args)`
    Call {
        recv: Option<Box<Expr>>,
        name: String,
        args: Vec<Expr>,
    },
    New {
        ty: String,
        args: Vec<Expr>,
    },
    Unary(&'static str, Box<Expr>),
    Binary(Box<Expr>, &'static str, Box<Expr>),
    Assign(Box<Expr>, &'static str, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Cast(String, Box<Expr>),
    Lambda {
        params: Vec<String>,
        body: LambdaBody,
    },
    // `e instanceof <pattern text>`
    InstanceOf(Box<Expr>, String),
    Switch {
        scrut: Box<Expr>,
        arms: Vec<Arm>,
    },
}

#[derive(Clone, Debug)]
pub enum LambdaBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Clone, Debug)]
pub struct Arm {
    pub line: u32,
    // `case X ->` label text without `case`/`->`, e.g. `Some(var x) when x > 1`, or `default`.
    pub label: String,
    pub body: ArmBody,
}

#[derive(Clone, Debug)]
pub enum ArmBody {
    Expr(Expr),
    Block(Block),
    Throw(Expr),
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub close_line: u32,
}

#[derive(Clone, Debug)]
pub struct Stmt {
    pub line: u32,
    pub kind: StmtKind,
}

#[derive(Clone, Debug)]
pub enum StmtKind {
    // `ty name = init;` / `ty name;`
    Local {
        ty: String,
        name: String,
        init: Option<Expr>,
    },
    Expr(Expr),
    If {
        cond: Expr,
        then: Block,
        els: Option<Box<Stmt>>,
    },
    While {
        label: Option<String>,
        cond: Expr,
        body: Block,
    },
    // `for (header) body`, header is raw text such as `int i = 0; i < n; i++`.
    For {
        label: Option<String>,
        header: String,
        body: Block,
    },
    ForEach {
        label: Option<String>,
        ty: String,
        name: String,
        iter: Expr,
        body: Block,
    },
    Switch {
        scrut: Expr,
        arms: Vec<Arm>,
    },
    Return(Option<Expr>),
    Break(Option<String>),
    Continue(Option<String>),
    Throw(Expr),
    Block(Block),
    // Labeled block, used for `'a: { ... break 'a; }`.
    Labeled {
        label: String,
        body: Block,
    },
    Raw(String),
}

#[derive(Clone, Debug)]
pub struct Method {
    pub line: u32,
    // Everything before the body, e.g. `static int add(int a, int b)`.
    pub header: String,
    // `None` for abstract/interface methods (printed with `;`).
    pub body: Option<Block>,
}

#[derive(Clone, Debug)]
pub enum Member {
    Field { line: u32, text: String },
    Method(Method),
    Class(Class),
    Raw { line: u32, text: String },
}

#[derive(Clone, Debug)]
pub struct Class {
    pub line: u32,
    // e.g. `static final class Point implements Comparable<Point>`
    pub header: String,
    pub members: Vec<Member>,
    pub close_line: u32,
    // Extra members printed on the closing line, before `}` (derives, constructors).
    pub trailer: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct JavaFile {
    pub package: String,
    pub imports: Vec<String>,
    pub class: Class,
}

impl Expr {
    pub fn name(s: impl Into<String>) -> Expr {
        Expr::Name(s.into())
    }

    pub fn raw(s: impl Into<String>) -> Expr {
        Expr::Raw(s.into())
    }

    pub fn call(recv: Expr, name: impl Into<String>, args: Vec<Expr>) -> Expr {
        Expr::Call {
            recv: Some(Box::new(recv)),
            name: name.into(),
            args,
        }
    }

    pub fn static_call(name: impl Into<String>, args: Vec<Expr>) -> Expr {
        Expr::Call {
            recv: None,
            name: name.into(),
            args,
        }
    }

    pub fn bin(l: Expr, op: &'static str, r: Expr) -> Expr {
        Expr::Binary(Box::new(l), op, Box::new(r))
    }

    pub fn str_lit(s: &str) -> Expr {
        Expr::Raw(java_string_literal(s))
    }

    // Whether this expression may stand alone as a Java statement.
    pub fn is_statement_expr(&self) -> bool {
        matches!(
            self,
            Expr::Call { .. } | Expr::New { .. } | Expr::Assign(..)
        ) || matches!(self, Expr::Unary(op, _) if op.starts_with("++") || op.starts_with("--"))
    }
}

impl Stmt {
    pub fn new(line: u32, kind: StmtKind) -> Stmt {
        Stmt { line, kind }
    }
}

pub fn java_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c if (c as u32) > 0xFFFF => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04x}", unit));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

pub fn java_char_literal(c: char) -> String {
    match c {
        '\'' => "'\\''".into(),
        '\\' => "'\\\\'".into(),
        '\n' => "'\\n'".into(),
        '\r' => "'\\r'".into(),
        '\t' => "'\\t'".into(),
        '\0' => "'\\0'".into(),
        c if (c as u32) < 0x20 || (c as u32) > 0xFFFF => format!("'\\u{:04x}'", c as u32),
        c => format!("'{c}'"),
    }
}
