// stdio (§12): stdin read_line / read_to_string, fs read/write, env::args.
use std::io::{self, Read};

fn main() {
    let mut line = String::new();
    let n = io::stdin().read_line(&mut line).unwrap();
    println!("{} {}", n, line.trim());

    let mut rest = String::new();
    io::stdin().lock().read_to_string(&mut rest).unwrap();
    print!("rest={}", rest);

    std::fs::write("/tmp/rust2java_std_io.txt", "file contents\n").unwrap();
    let content = std::fs::read_to_string("/tmp/rust2java_std_io.txt").unwrap();
    print!("{content}");
    println!("len={}", content.len());

    let args: Vec<String> = std::env::args().skip(1).collect();
    println!("args {}", args.len());
    let which = std::env::var("PATH");
    println!("{}", which.is_ok());
}
