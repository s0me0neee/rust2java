// A tokenizer and recursive-descent expression calculator: a boxed expression
// tree, precedence climbing, variable assignment and a step-by-step trace.
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Num(i64),
    Ident(String),
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    LParen,
    RParen,
    Assign,
    Semi,
}

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Token::Num(n) => write!(f, "{}", n),
            Token::Ident(name) => write!(f, "{}", name),
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::Slash => write!(f, "/"),
            Token::Percent => write!(f, "%"),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Assign => write!(f, "="),
            Token::Semi => write!(f, ";"),
        }
    }
}

fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => i += 1,
            '0'..='9' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                let value = text
                    .parse::<i64>()
                    .map_err(|_| format!("number too large: {}", text))?;
                tokens.push(Token::Num(value));
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            '+' => {
                tokens.push(Token::Plus);
                i += 1;
            }
            '-' => {
                tokens.push(Token::Minus);
                i += 1;
            }
            '*' => {
                tokens.push(Token::Star);
                i += 1;
            }
            '/' => {
                tokens.push(Token::Slash);
                i += 1;
            }
            '%' => {
                tokens.push(Token::Percent);
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '=' => {
                tokens.push(Token::Assign);
                i += 1;
            }
            ';' => {
                tokens.push(Token::Semi);
                i += 1;
            }
            other => return Err(format!("unexpected character {:?} at {}", other, i)),
        }
    }
    Ok(tokens)
}

#[derive(Debug, PartialEq)]
enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
}

#[derive(Debug)]
enum Expr {
    Num(i64),
    Var(String),
    UnaryMinus(Box<Expr>),
    Binary(Box<Expr>, Op, Box<Expr>),
    Assign(String, Box<Expr>),
}

impl Expr {
    fn eval(&self, env: &mut HashMap<String, i64>) -> Result<i64, String> {
        match self {
            Expr::Num(n) => Ok(*n),
            Expr::Var(name) => env
                .get(name)
                .copied()
                .ok_or_else(|| format!("undefined variable `{}`", name)),
            Expr::UnaryMinus(inner) => Ok(-inner.eval(env)?),
            Expr::Binary(left, op, right) => {
                let a = left.eval(env)?;
                let b = right.eval(env)?;
                match op {
                    Op::Add => Ok(a + b),
                    Op::Sub => Ok(a - b),
                    Op::Mul => Ok(a * b),
                    Op::Div => {
                        if b == 0 {
                            Err("division by zero".to_string())
                        } else {
                            Ok(a / b)
                        }
                    }
                    Op::Rem => {
                        if b == 0 {
                            Err("division by zero".to_string())
                        } else {
                            Ok(a % b)
                        }
                    }
                }
            }
            Expr::Assign(name, value) => {
                let v = value.eval(env)?;
                env.insert(name.clone(), v);
                Ok(v)
            }
        }
    }

    fn depth(&self) -> usize {
        match self {
            Expr::Num(_) | Expr::Var(_) => 1,
            Expr::UnaryMinus(inner) => 1 + inner.depth(),
            Expr::Binary(left, _, right) => 1 + left.depth().max(right.depth()),
            Expr::Assign(_, value) => 1 + value.depth(),
        }
    }

    fn size(&self) -> usize {
        match self {
            Expr::Num(_) | Expr::Var(_) => 1,
            Expr::UnaryMinus(inner) => 1 + inner.size(),
            Expr::Binary(left, _, right) => 1 + left.size() + right.size(),
            Expr::Assign(_, value) => 1 + value.size(),
        }
    }
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Parser {
        Parser { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn expect(&mut self, want: &Token) -> Result<(), String> {
        match self.advance() {
            Some(token) if token == *want => Ok(()),
            Some(token) => Err(format!("expected {:?}, found {:?}", want, token)),
            None => Err(format!("expected {:?}, found end of input", want)),
        }
    }

    fn statement(&mut self) -> Result<Expr, String> {
        let expr = self.expression(0)?;
        if matches!(self.peek(), Some(Token::Semi)) {
            self.advance();
        }
        Ok(expr)
    }

    fn expression(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut left = self.unary()?;
        while let Some(op) = self.peek_op() {
            let prec = precedence(&op);
            if prec < min_prec {
                break;
            }
            self.advance();
            let right = self.expression(prec + 1)?;
            left = Expr::Binary(Box::new(left), op, Box::new(right));
        }
        Ok(left)
    }

    fn peek_op(&self) -> Option<Op> {
        match self.peek() {
            Some(Token::Plus) => Some(Op::Add),
            Some(Token::Minus) => Some(Op::Sub),
            Some(Token::Star) => Some(Op::Mul),
            Some(Token::Slash) => Some(Op::Div),
            Some(Token::Percent) => Some(Op::Rem),
            _ => None,
        }
    }

    fn unary(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Token::Minus) => {
                self.advance();
                Ok(Expr::UnaryMinus(Box::new(self.unary()?)))
            }
            Some(Token::Num(_)) => match self.advance() {
                Some(Token::Num(n)) => Ok(Expr::Num(n)),
                _ => unreachable!(),
            },
            Some(Token::Ident(_)) => match self.advance() {
                Some(Token::Ident(name)) => {
                    if matches!(self.peek(), Some(Token::Assign)) {
                        self.advance();
                        let value = self.expression(0)?;
                        Ok(Expr::Assign(name, Box::new(value)))
                    } else {
                        Ok(Expr::Var(name))
                    }
                }
                _ => unreachable!(),
            },
            Some(Token::LParen) => {
                self.advance();
                let inner = self.expression(0)?;
                self.expect(&Token::RParen)?;
                Ok(inner)
            }
            Some(token) => Err(format!("unexpected token {:?}", token)),
            None => Err("unexpected end of input".to_string()),
        }
    }
}

fn precedence(op: &Op) -> u8 {
    match op {
        Op::Add | Op::Sub => 1,
        Op::Mul | Op::Div | Op::Rem => 2,
    }
}

fn run_line(line: &str, env: &mut HashMap<String, i64>) {
    let tokens = match tokenize(line) {
        Ok(tokens) => tokens,
        Err(message) => {
            println!("{}: token error: {}", line, message);
            return;
        }
    };
    let mut parser = Parser::new(tokens);
    match parser.statement() {
        Ok(expr) => match expr.eval(env) {
            Ok(value) => println!("{} = {} (depth {}, nodes {})", line, value, expr.depth(), expr.size()),
            Err(message) => println!("{}: eval error: {}", line, message),
        },
        Err(message) => println!("{}: parse error: {}", line, message),
    }
}

fn main() {
    let program = [
        "1 + 2 * 3",
        "(1 + 2) * 3",
        "x = 10;",
        "y = x * x - 4",
        "z = (x + y) / 2 % 7",
        "total = x + y + z;",
        "total",
        "missing + 1",
        "1 / 0",
        "2 % 0",
        "-(-5) + 3",
        "1 + (2 * (3 + 4))",
        "12 $ 4",
        "999999999999999999999999",
        "(((7)))",
        "x = y = 42;",
        "x + y",
    ];
    let mut env: HashMap<String, i64> = HashMap::new();
    for line in program {
        run_line(line, &mut env);
    }
    println!("---");
    let mut names: Vec<&String> = env.keys().collect();
    names.sort();
    for name in names {
        println!("{} = {}", name, env[name]);
    }
    let sum: i64 = env.values().sum();
    println!("sum of variables: {}", sum);
}
