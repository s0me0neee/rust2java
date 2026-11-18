// Line-oriented stdin loop: the unwrapped `lines()` item is a plain String.
use std::io::{self, BufRead};

fn main() {
    let mut total = 0;
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let n: i64 = line.parse().unwrap();
        total += n;
        println!("{n} -> {total}");
    }
    println!("total {total}");
}
