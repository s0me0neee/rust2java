use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    let first = lines.next().unwrap().unwrap();
    let mut nq = first.split_whitespace().map(|x| x.parse::<usize>().unwrap());
    let (n, q) = (nq.next().unwrap(), nq.next().unwrap());
    let a: Vec<i64> = lines
        .next()
        .unwrap()
        .unwrap()
        .split_whitespace()
        .map(|x| x.parse().unwrap())
        .collect();
    assert_eq!(a.len(), n);
    let mut pre = vec![0i64; n + 1];
    for i in 0..n {
        pre[i + 1] = pre[i] + a[i];
    }
    let out = io::stdout();
    let mut out = out.lock();
    for _ in 0..q {
        let line = lines.next().unwrap().unwrap();
        let v: Vec<usize> = line.split(' ').map(|x| x.parse().unwrap()).collect();
        let (l, r) = (v[0], v[1]);
        writeln!(out, "{}", pre[r] - pre[l - 1]).unwrap();
    }
    let best = (1..=n).flat_map(|l| (l..=n).map(move |r| (l, r))).max_by_key(|&(l, r)| pre[r] - pre[l - 1]);
    writeln!(out, "best {:?}", best).unwrap();
}
