use std::io;

fn main() {
    print!("line> ");
    let mut text = String::from("prefix:");
    let first = io::stdin().read_line(&mut text).unwrap();
    println!("{}:{:?}", first, text);
    text.clear();
    let last = io::stdin().read_line(&mut text).unwrap();
    println!("{}:{:?}", last, text);
    let eof = io::stdin().read_line(&mut text).unwrap();
    println!("{}:{:?}", eof, text);
}
