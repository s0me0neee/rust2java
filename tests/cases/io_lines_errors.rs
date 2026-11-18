// Exact byte-level line splitting: the byte read below keeps stdin out of line mode.
use std::io::{self, BufRead, Read};

fn main() {
    for line in io::stdin().lock().lines().flatten() {
        println!("{:?}", line);
    }
    let _ = io::stdin().bytes().count();
}
