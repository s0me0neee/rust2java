use std::env;
use std::io::{self, BufRead, Read, Write};
fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{}", args.len());
    let stdin = io::stdin();
    let mut first = String::new();
    stdin.lock().read_line(&mut first).unwrap();
    let n: usize = first.trim().parse().unwrap();
    let mut rows = Vec::new();
    for _ in 0..n {
        let mut line = String::new();
        io::stdin().read_line(&mut line).expect("read");
        let nums: Vec<i64> = line.split_whitespace().map(|x| x.parse().unwrap()).collect();
        rows.push(nums);
    }
    println!("{:?}", rows);
    let mut rest = String::new();
    io::stdin().read_to_string(&mut rest).unwrap();
    let words: Vec<&str> = rest.lines().filter(|l| !l.is_empty()).collect();
    println!("{:?}", words);
    let out = io::stdout();
    let mut lock = out.lock();
    writeln!(lock, "locked {}", 1).unwrap();
    let mut buf = io::BufWriter::new(io::stdout());
    write!(buf, "a").unwrap();
    writeln!(buf, "b").unwrap();
    buf.flush().unwrap();
    eprintln!("to stderr");
    print!("no newline");
    io::stdout().flush().unwrap();
    println!();
    if env::var("SURELY_NOT_SET_VAR").is_err() {
        println!("unset");
    }
    std::process::exit(if rows.is_empty() { 1 } else { 0 });
}
