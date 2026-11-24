use std::io::{self, Read, Write};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut it = input.split_ascii_whitespace().map(|x| x.parse::<i64>().unwrap());
    let t = it.next().unwrap() as usize;
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());
    for _ in 0..t {
        let n = it.next().unwrap() as usize;
        let target = it.next().unwrap();
        let mut a: Vec<(i64, usize)> = (0..n).map(|i| (it.next().unwrap(), i + 1)).collect();
        a.sort_unstable();
        let (mut l, mut r) = (0usize, n.saturating_sub(1));
        let mut ans = None;
        while l < r {
            let s = a[l].0 + a[r].0;
            if s == target {
                ans = Some((a[l].1.min(a[r].1), a[l].1.max(a[r].1)));
                break;
            } else if s < target {
                l += 1;
            } else {
                r -= 1;
            }
        }
        match ans {
            Some((x, y)) => writeln!(out, "{} {}", x, y).unwrap(),
            None => writeln!(out, "IMPOSSIBLE").unwrap(),
        }
    }
}
