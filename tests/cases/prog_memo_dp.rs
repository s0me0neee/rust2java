use std::collections::HashMap;
fn ways(n: u64, memo: &mut HashMap<u64, u64>) -> u64 {
    if n <= 1 { return 1; }
    if let Some(&v) = memo.get(&n) { return v; }
    let r = (ways(n - 1, memo) + ways(n - 2, memo)) % 1_000_000_007;
    memo.insert(n, r);
    r
}
fn lcs(a: &str, b: &str) -> String {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 1..=a.len() { for j in 1..=b.len() {
        dp[i][j] = if a[i-1] == b[j-1] { dp[i-1][j-1] + 1 } else { dp[i-1][j].max(dp[i][j-1]) };
    } }
    let (mut i, mut j) = (a.len(), b.len());
    let mut out = Vec::new();
    while i > 0 && j > 0 {
        if a[i-1] == b[j-1] { out.push(a[i-1]); i -= 1; j -= 1; }
        else if dp[i-1][j] >= dp[i][j-1] { i -= 1 } else { j -= 1 }
    }
    out.iter().rev().collect()
}
fn main() {
    let mut memo = HashMap::new();
    println!("{} {}", ways(90, &mut memo), memo.len());
    println!("{}", lcs("AGGTAB", "GXTXAYB"));
    let primes: Vec<u32> = (2..60).filter(|n| (2..*n).take_while(|d| d * d <= *n).all(|d| n % d != 0)).collect();
    println!("{primes:?}");
    let total: u64 = primes.iter().map(|&p| p as u64).product::<u64>() % 1_000_000_007;
    println!("{total}");
    let mut counter = 0;
    let mut inc = |by: i32| { counter += by; counter };
    inc(2); inc(5);
    println!("{}", inc(0));
    let words = ["apple", "bob", "kayak", "zed"];
    let (pal, non): (Vec<&str>, Vec<&str>) = words.iter().partition(|w| w.chars().rev().collect::<String>() == **w);
    println!("{pal:?} {non:?}");
}
