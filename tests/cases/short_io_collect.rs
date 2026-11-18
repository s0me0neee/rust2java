// `lines().map(unwrap).collect()` over stdin: plain Strings, one `toList()`.
use std::io::{self, BufRead};

fn main() {
    let lines: Vec<String> = io::stdin().lines().map(|l| l.unwrap()).collect();
    let words: usize = lines.iter().map(|l| l.split_whitespace().count()).sum();
    println!("{} lines, {} words", lines.len(), words);
    for (i, line) in lines.iter().enumerate() {
        println!("{i}: {line}");
    }
}
