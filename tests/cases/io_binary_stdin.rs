// read_line then raw bytes from the same stdin (non-UTF-8 bytes only reach the byte reads).
use std::io::{self, BufRead, Read};

fn main() {
    let mut text = String::from("kept");
    let invalid = io::stdin().lock().read_line(&mut text);
    println!("invalid={} text={}", invalid.is_err(), text);
    let bytes: Vec<u8> = io::stdin().bytes().take(2).map(|b| b.unwrap()).collect();
    println!("bytes={:?}", bytes);
    let mut rest = vec![42u8];
    let n = io::stdin().read_to_end(&mut rest).unwrap();
    println!("n={} rest={:?}", n, rest);
}
