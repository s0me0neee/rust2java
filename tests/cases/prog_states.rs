// State machines: match on (state, event) tuples of enums, enums with data, impl blocks returning Self,
// matches! with guards, nested Option<Result>, and char-level text scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Light {
    Red,
    Green,
    Yellow,
    Off,
}

#[derive(Debug, Clone, Copy)]
enum Event {
    Tick,
    Fault,
    Reset,
}

impl Light {
    fn next(self, e: Event) -> Light {
        match (self, e) {
            (_, Event::Fault) => Light::Off,
            (Light::Off, Event::Reset) => Light::Red,
            (Light::Off, _) => Light::Off,
            (Light::Red, Event::Tick) => Light::Green,
            (Light::Green, Event::Tick) => Light::Yellow,
            (Light::Yellow, Event::Tick) => Light::Red,
            (s, Event::Reset) => s,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Hand {
    Rock,
    Paper,
    Scissors,
}

#[derive(Debug)]
enum Outcome {
    Win(Hand),
    Draw,
}

fn play(a: Hand, b: Hand) -> Outcome {
    use Hand::*;
    match (a, b) {
        (x, y) if x == y => Outcome::Draw,
        (Rock, Scissors) | (Paper, Rock) | (Scissors, Paper) => Outcome::Win(a),
        _ => Outcome::Win(b),
    }
}

#[derive(Debug)]
enum Token {
    Num(i64),
    Word(String),
    Sym(char),
}

fn scan(s: &str) -> Vec<Token> {
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = vec![];
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' => i += 1,
            '0'..='9' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                let text: String = chars[start..i].iter().collect();
                out.push(Token::Num(text.parse().unwrap()));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut w = String::new();
                while let Some(&ch) = chars.get(i) {
                    if !(ch.is_alphanumeric() || ch == '_') {
                        break;
                    }
                    w.push(ch);
                    i += 1;
                }
                out.push(Token::Word(w));
            }
            other => {
                out.push(Token::Sym(other));
                i += 1;
            }
        }
    }
    out
}

fn classify(t: &Token) -> &'static str {
    match t {
        Token::Num(n) if *n < 0 => "neg",
        Token::Num(0) => "zero",
        Token::Num(1..=9) => "digit",
        Token::Num(_) => "number",
        Token::Word(w) if w.starts_with('_') => "private",
        Token::Word(w) if matches!(w.as_str(), "let" | "fn" | "if") => "keyword",
        Token::Word(_) => "ident",
        Token::Sym('(' | ')' | '{' | '}') => "bracket",
        Token::Sym(_) => "symbol",
    }
}

fn lookup(id: u32) -> Option<Result<u32, String>> {
    match id {
        0 => None,
        n if n % 7 == 0 => Some(Err(format!("{} is cursed", n))),
        n => Some(Ok(n * 3)),
    }
}

fn main() {
    let events = [Event::Tick, Event::Tick, Event::Reset, Event::Tick, Event::Fault, Event::Tick, Event::Reset, Event::Tick];
    let mut s = Light::Red;
    let mut trace = vec![format!("{:?}", s)];
    for e in events {
        s = s.next(e);
        trace.push(format!("{:?}", s));
    }
    println!("{}", trace.join(" > "));
    let reds = trace.iter().filter(|t| *t == "Red").count();
    println!("reds {} final_off {}", reds, s == Light::Off);

    let hands = [Hand::Rock, Hand::Paper, Hand::Scissors];
    let mut score = [0i32; 3];
    for &a in &hands {
        for &b in &hands {
            match play(a, b) {
                Outcome::Win(h) if h == a => score[0] += 1,
                Outcome::Win(_) => score[1] += 1,
                Outcome::Draw => score[2] += 1,
            }
        }
    }
    println!("{:?} {:?}", score, play(Hand::Scissors, Hand::Rock));

    let toks = scan("let _x1 = foo(42, 7) + 0 { y_2 }");
    let kinds: Vec<&str> = toks.iter().map(classify).collect();
    println!("{:?}", kinds);
    let words: Vec<&String> = toks.iter().filter_map(|t| if let Token::Word(w) = t { Some(w) } else { None }).collect();
    println!("{:?} nums={}", words, toks.iter().filter(|t| matches!(t, Token::Num(n) if *n > 5)).count());
    println!("{:?}", toks.last());

    for id in [0, 4, 14, 5] {
        let msg = match lookup(id) {
            None => "missing".to_string(),
            Some(Ok(v)) if v > 12 => format!("big {}", v),
            Some(Ok(v)) => format!("ok {}", v),
            Some(Err(e)) => format!("err: {}", e),
        };
        println!("{} -> {}", id, msg);
    }
    let found: Result<Vec<u32>, String> = [1, 2, 3].iter().map(|&i| lookup(i).unwrap()).collect();
    let failed: Result<Vec<u32>, String> = [1, 7, 3].iter().map(|&i| lookup(i).unwrap()).collect();
    println!("{:?} {:?}", found, failed);
    let transposed: Result<Option<u32>, String> = lookup(2).transpose();
    println!("{:?} {:?}", transposed, lookup(0).transpose());
}
