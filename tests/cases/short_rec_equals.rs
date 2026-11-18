// Derived equality/hash: data-enum records use their implicit component-wise equals/hashCode.
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Token {
    Num(i32),
    Word(String),
    Pair { line: i32, text: String },
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
enum Amount {
    Value(f64),
    Zero,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct Label {
    name: String,
    point: Point,
    token: Token,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    id: i32,
    tag: char,
}

fn main() {
    let toks = [
        Token::Num(1),
        Token::Num(1),
        Token::Word(String::from("a")),
        Token::Word(String::from("a")),
        Token::Eof,
        Token::Eof,
    ];
    println!(
        "{} {} {}",
        toks[0] == toks[1],
        toks[2] == toks[3],
        toks[4] == toks[5]
    );
    let p1 = Token::Pair { line: 1, text: String::from("x") };
    let p2 = Token::Pair { line: 1, text: String::from("x") };
    let p3 = Token::Pair { line: 2, text: String::from("x") };
    println!("{} {} {}", p1 == p2, p1 != p3, p1 == Token::Eof);

    let mut set = HashSet::new();
    set.insert(Token::Num(7));
    set.insert(Token::Num(7));
    set.insert(Token::Word(String::from("w")));
    println!("{} {}", set.len(), set.contains(&Token::Num(7)));

    let zero = Amount::Value(0.0);
    let neg = Amount::Value(-0.0);
    let nan = Amount::Value(f64::NAN);
    println!(
        "{} {} {}",
        zero == neg,
        nan == nan,
        Amount::Zero == Amount::Zero
    );

    let l1 = Label {
        name: String::from("n"),
        point: Point { x: 1, y: 2 },
        token: Token::Num(3),
    };
    let l2 = l1.clone();
    let l3 = Label {
        name: String::from("n"),
        point: Point { x: 1, y: 3 },
        token: Token::Num(3),
    };
    println!("{} {} {}", l1 == l2, l1 == l3, l1 != l3);

    let k1 = Key { id: 1, tag: 'a' };
    let k2 = Key { id: 1, tag: 'a' };
    let mut keys = HashSet::new();
    keys.insert(k1.clone());
    println!("{} {}", k1 == k2, keys.contains(&k2));
}
