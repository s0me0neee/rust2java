// read_line into a buffer that is only read trimmed: `buf = STDIN.readLine();`, and
// `throws IOException` on the methods that read (not on one called from a closure).
use std::io;

fn read_num() -> i64 {
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    s.trim().parse().unwrap()
}

fn read_pair() -> (i64, i64) {
    let mut s = String::new();
    io::stdin().read_line(&mut s).unwrap();
    let v: Vec<i64> = s.split_whitespace().map(|x| x.parse().unwrap()).collect();
    (v[0], v[1])
}

fn solve() -> i64 {
    let (a, b) = read_pair();
    a + b
}

fn main() {
    let n = read_num();
    let xs: Vec<i64> = (0..n).map(|_| read_num()).collect();
    println!("{:?} {}", xs, solve());
}
