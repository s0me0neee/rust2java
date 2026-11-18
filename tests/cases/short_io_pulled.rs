// Pulled stdin line iterator: `next()` already yields the plain String.
use std::io::{self, BufRead};

fn main() {
    let mut it = io::stdin().lock().lines();
    let first = it.next().unwrap().unwrap();
    let second = it.next().unwrap().unwrap();
    println!("{}|{}", first.trim(), second.trim().to_uppercase());
}
