// `map_while(Result::ok)` over stdin lines: every line is Ok, so the stream is unchanged.
use std::io;

fn main() {
    let lines: Vec<String> = io::stdin().lines().map_while(Result::ok).collect();
    println!("{} lines", lines.len());
    for line in io::stdin().lines().map_while(Result::ok) {
        println!("{}", line.trim());
    }
}
