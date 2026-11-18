// `lines().flatten()` keeps only Ok lines; stdin lines never fail, so it is just the stream.
use std::io;

fn main() {
    for line in io::stdin().lines().flatten() {
        println!("<{}>", line.trim());
    }
    let count = io::stdin().lines().flatten().count();
    println!("count={count}");
}
