fn count(v: &[usize]) -> usize {
    let mut total = 0;
    for i in 0..v.len() {
        for j in i + 1..v.len() {
            total += v[i] * v[j];
        }
    }
    total
}
fn main() {
    let n: usize = 70_000;
    let v: Vec<usize> = (1..=n).collect();
    let mut ans = 0;
    let mut best = 0;
    for i in 0..v.len() {
        ans += v[i] * v[i];
        if v[i] > best { best = v[i]; }
    }
    let pairs = n * (n - 1) / 2;
    let s: usize = v.iter().sum();
    let sq = s * 3;
    let mut cnt = 0;
    for x in &v { if x % 7 == 0 { cnt += 1; } }
    println!("{} {} {} {} {} {cnt}", ans, best, pairs, s, sq);
    println!("{}", ans % 1_000_000_007);
    println!("{}", count(&v[..10]));
    let idx = v.len() / 2;
    println!("{}", v[idx]);
    let big = (pairs as u64) * 2;
    println!("{big}");
}
