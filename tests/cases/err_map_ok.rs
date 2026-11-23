// `.map(f)` on a Result that simple mode throws: a variant constructor and a closure.
#[derive(Debug)]
enum Tok {
    Num(i64),
    Word(String),
}

fn num(s: &str) -> Result<Tok, std::num::ParseIntError> {
    s.parse::<i64>().map(Tok::Num)
}

fn double(s: &str) -> Result<i64, std::num::ParseIntError> {
    s.trim().parse::<i64>().map(|n| n * 2)
}

fn main() {
    for s in ["42", "x"] {
        match num(s) {
            Ok(Tok::Num(n)) if n > 40 => println!("big {n}"),
            Ok(Tok::Num(n)) => println!("num {n}"),
            Ok(Tok::Word(w)) => println!("word {w}"),
            Err(_) => println!("not a number: {s}"),
        }
    }
    match double(" 21 ") {
        Ok(n) => println!("{n}"),
        Err(_) => println!("bad"),
    }
}
