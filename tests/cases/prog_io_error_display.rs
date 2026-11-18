// Display of `std::io::Error`: Rust's text for the common file errors.
use std::fs;

fn main() {
    match fs::read_to_string("r2j-no-such-file.txt") {
        Ok(text) => println!("{}", text),
        Err(error) => println!("error: {}", error),
    }
}
