// User macro_rules! expanded in place, plus the std gaps found in competitive-programming code.
macro_rules! read_token {
    () => {{
        use std::io::{self, Read};
        let mut token = Vec::new();
        let mut buf = [0u8; 1];
        loop {
            let n = io::stdin().read(&mut buf).unwrap();
            if n == 0 || (buf[0].is_ascii_whitespace() && !token.is_empty()) {
                break;
            }
            if !buf[0].is_ascii_whitespace() {
                token.push(buf[0]);
            }
        }
        String::from_utf8(token).unwrap()
    }};
    ($t:ty) => {{
        let s = read_token!();
        s.parse::<$t>().unwrap()
    }};
}

macro_rules! square {
    ($x:expr) => {
        $x * $x
    };
}

macro_rules! pair {
    ($a:ident, $b:ident = $v:expr) => {
        let $a = $v;
        let $b = $v + 1;
    };
}

fn main() {
    let n: usize = read_token!(usize);
    let word: String = read_token!(String);
    let mut total = 0.0f64;
    for _ in 0..n {
        let x = read_token!(u32);
        total = f64::max(total, x as f64 / 2.0);
        println!("{} {}", x, x.is_multiple_of(3));
    }
    println!("{} {} {}", word, total, square!(n + 1));
    pair!(a, b = 10);
    println!("{} {}", a, b);
    let digits: u32 = word.chars().filter(|c| c.is_ascii_digit()).map(|c| char::to_digit(c, 10).unwrap()).sum();
    let chars: Vec<char> = "ééab".chars().collect();
    println!("{} {} {}", digits, chars[0] == chars[1], chars[2] == chars[3]);
}

#[test]
fn only_in_tests() {
    let unused = read_token!();
    assert!(unused.is_empty());
}
