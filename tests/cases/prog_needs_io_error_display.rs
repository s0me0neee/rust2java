// Display of `std::io::Error` is not lowered yet.
use std::fs;

fn main() {
    match fs::read_to_string("r2j-no-such-file.txt") {
        Ok(text) => println!("{}", text),
        Err(error) => println!("error: {}", error),
    }
}
