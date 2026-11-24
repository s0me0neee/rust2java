#[derive(Debug, PartialEq, Clone)]
enum Token {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
}

fn tokenize(src: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                tokens.push(Token::Num(text.parse().map_err(|_| format!("bad number '{text}'"))?));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            '+' | '-' | '*' | '/' | '^' => {
                tokens.push(Token::Op(c));
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
            _ => return Err(format!("unexpected '{c}' at {i}")),
        }
    }
    Ok(tokens)
}

fn to_rpn(tokens: &[Token]) -> Vec<Token> {
    let prec = |op: char| match op {
        '^' => 3,
        '*' | '/' => 2,
        _ => 1,
    };
    let mut out = Vec::new();
    let mut ops: Vec<Token> = Vec::new();
    for t in tokens {
        match t {
            Token::Num(_) | Token::Ident(_) => out.push(t.clone()),
            Token::Op(o) => {
                while let Some(Token::Op(top)) = ops.last() {
                    if prec(*top) > prec(*o) || (prec(*top) == prec(*o) && *o != '^') {
                        out.push(ops.pop().unwrap());
                    } else {
                        break;
                    }
                }
                ops.push(t.clone());
            }
            Token::LParen => ops.push(Token::LParen),
            Token::RParen => {
                while let Some(top) = ops.pop() {
                    if top == Token::LParen {
                        break;
                    }
                    out.push(top);
                }
            }
        }
    }
    while let Some(op) = ops.pop() {
        out.push(op);
    }
    out
}

fn main() {
    for src in ["3 + 4 * x", "(1.5 + rate) ^ 2 ^ 3 / y_1", "2 $ 3", "1..2 + 1"] {
        match tokenize(src) {
            Ok(toks) => {
                let rpn: Vec<String> = to_rpn(&toks)
                    .iter()
                    .map(|t| match t {
                        Token::Num(n) => format!("{n}"),
                        Token::Ident(s) => s.clone(),
                        Token::Op(c) => c.to_string(),
                        other => format!("{other:?}"),
                    })
                    .collect();
                println!("{src} => {}", rpn.join(" "));
            }
            Err(e) => println!("{src} => error: {e}"),
        }
    }
}
