// Tokenizer + recursive-descent parser + evaluator with variables.
use std::collections::HashMap;
use std::fmt;
use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Assign,
}

#[derive(Debug)]
enum Ast {
    Num(f64),
    Var(String),
    Bin(Box<Ast>, char, Box<Ast>),
    Neg(Box<Ast>),
    Let(String, Box<Ast>),
}

#[derive(Debug)]
enum EvalError {
    Unknown(String),
    DivZero,
    Syntax(String),
}

impl fmt::Display for EvalError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            EvalError::Unknown(v) => write!(f, "unknown variable '{}'", v),
            EvalError::DivZero => write!(f, "division by zero"),
            EvalError::Syntax(m) => write!(f, "syntax error: {}", m),
        }
    }
}

struct Lexer<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Lexer<'a> {
    fn new(s: &'a str) -> Self {
        Lexer { chars: s.chars().peekable() }
    }

    fn tokens(&mut self) -> Result<Vec<Tok>, EvalError> {
        let mut out = Vec::new();
        while let Some(&c) = self.chars.peek() {
            if c.is_whitespace() {
                self.chars.next();
            } else if c.is_ascii_digit() || c == '.' {
                let mut s = String::new();
                while let Some(&d) = self.chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        s.push(d);
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = s.parse().map_err(|_| EvalError::Syntax(format!("bad number {}", s)))?;
                out.push(Tok::Num(n));
            } else if c.is_alphabetic() {
                let mut s = String::new();
                while let Some(&d) = self.chars.peek() {
                    if d.is_alphanumeric() || d == '_' {
                        s.push(d);
                        self.chars.next();
                    } else {
                        break;
                    }
                }
                out.push(Tok::Ident(s));
            } else {
                self.chars.next();
                out.push(match c {
                    '(' => Tok::LParen,
                    ')' => Tok::RParen,
                    '=' => Tok::Assign,
                    '+' | '-' | '*' | '/' | '%' => Tok::Op(c),
                    other => return Err(EvalError::Syntax(format!("unexpected '{}'", other))),
                });
            }
        }
        Ok(out)
    }
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        self.pos += 1;
        t
    }

    fn statement(&mut self) -> Result<Ast, EvalError> {
        if let (Some(Tok::Ident(name)), Some(Tok::Assign)) = (self.toks.get(0), self.toks.get(1)) {
            let name = name.clone();
            self.pos = 2;
            let e = self.expr()?;
            return Ok(Ast::Let(name, Box::new(e)));
        }
        self.expr()
    }

    fn expr(&mut self) -> Result<Ast, EvalError> {
        let mut lhs = self.term()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if op != '+' && op != '-' {
                break;
            }
            self.next();
            let rhs = self.term()?;
            lhs = Ast::Bin(Box::new(lhs), op, Box::new(rhs));
        }
        Ok(lhs)
    }

    fn term(&mut self) -> Result<Ast, EvalError> {
        let mut lhs = self.factor()?;
        loop {
            match self.peek() {
                Some(Tok::Op(op)) if matches!(op, '*' | '/' | '%') => {
                    let op = *op;
                    self.next();
                    let rhs = self.factor()?;
                    lhs = Ast::Bin(Box::new(lhs), op, Box::new(rhs));
                }
                _ => break,
            }
        }
        Ok(lhs)
    }

    fn factor(&mut self) -> Result<Ast, EvalError> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Ast::Num(n)),
            Some(Tok::Ident(s)) => Ok(Ast::Var(s)),
            Some(Tok::Op('-')) => Ok(Ast::Neg(Box::new(self.factor()?))),
            Some(Tok::LParen) => {
                let e = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    t => Err(EvalError::Syntax(format!("expected ')', got {:?}", t))),
                }
            }
            t => Err(EvalError::Syntax(format!("unexpected token {:?}", t))),
        }
    }
}

fn eval(ast: &Ast, env: &mut HashMap<String, f64>) -> Result<f64, EvalError> {
    Ok(match ast {
        Ast::Num(n) => *n,
        Ast::Var(v) => *env.get(v).ok_or_else(|| EvalError::Unknown(v.clone()))?,
        Ast::Neg(e) => -eval(e, env)?,
        Ast::Bin(a, op, b) => {
            let x = eval(a, env)?;
            let y = eval(b, env)?;
            match op {
                '+' => x + y,
                '-' => x - y,
                '*' => x * y,
                '/' if y == 0.0 => return Err(EvalError::DivZero),
                '/' => x / y,
                '%' => x % y,
                _ => unreachable!(),
            }
        }
        Ast::Let(name, e) => {
            let v = eval(e, env)?;
            env.insert(name.clone(), v);
            v
        }
    })
}

fn main() {
    let lines = [
        "x = 3",
        "y = x * (2 + 4.5)",
        "y / 2 - -1",
        "z + 1",
        "10 / (x - 3)",
        "7 % 4 + 2 * 3",
        "(1 + 2",
        "4 $ 4",
    ];
    let mut env = HashMap::new();
    for (i, line) in lines.iter().enumerate() {
        let result = Lexer::new(line).tokens().and_then(|toks| {
            let mut p = Parser { toks, pos: 0 };
            p.statement()
        });
        match result.and_then(|ast| eval(&ast, &mut env)) {
            Ok(v) => println!("[{}] {} => {}", i + 1, line, v),
            Err(e) => println!("[{}] {} => error: {}", i + 1, line, e),
        }
    }
    let mut vars: Vec<_> = env.into_iter().collect();
    vars.sort_by(|a, b| a.0.cmp(&b.0));
    println!("{:?}", vars);
    let ast = Parser { toks: Lexer::new("-(a+1)").tokens().unwrap(), pos: 0 }.expr().unwrap();
    println!("{:?}", ast);
}
