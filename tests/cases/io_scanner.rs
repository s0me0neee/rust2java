use std::io::{self, Read};

fn main() {
    print!("numbers> ");
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut tokens = input.split_ascii_whitespace();
    let n: usize = tokens.next().unwrap().parse().unwrap();
    let mut total: i64 = 0;
    for _ in 0..n {
        let value: i64 = tokens.next().unwrap().parse().unwrap();
        total += value;
    }
    println!("sum={}", total);
}
