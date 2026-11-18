// `?` over stdin lines inside an `io::Result` function: a line cannot fail.
use std::io::{self, BufRead};

fn run() -> io::Result<()> {
    for line in io::stdin().lock().lines() {
        let line = line?;
        println!("{}", line.trim().to_uppercase());
    }
    Ok(())
}

fn main() {
    if run().is_err() {
        println!("failed");
    }
}
