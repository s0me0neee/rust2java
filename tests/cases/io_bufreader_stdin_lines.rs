use std::io::{self, BufRead, BufReader};

fn main() {
    let reader = BufReader::new(io::stdin());
    for line in reader.lines() {
        println!("row={}", line.unwrap());
    }
}
