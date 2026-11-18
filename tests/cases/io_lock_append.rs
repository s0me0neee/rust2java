use std::io::{self, BufRead, Read};

fn main() {
    let stdin = io::stdin();
    let mut lock = stdin.lock();
    let mut first = String::from("first:");
    let n = lock.read_line(&mut first).unwrap();
    println!("{}:{:?}", n, first);
    let mut rest = String::from("rest:");
    let n = lock.read_to_string(&mut rest).unwrap();
    println!("{}:{:?}", n, rest);
    let n = lock.read_line(&mut first).unwrap();
    println!("eof:{}:{:?}", n, first);
}
