use std::io::{self, Read};

fn main() {
    let mut count = 0;
    for byte in io::stdin().bytes() {
        println!("{}", byte.unwrap());
        count += 1;
    }
    println!("bytes={}", count);
}
