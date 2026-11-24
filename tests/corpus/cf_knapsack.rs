fn knapsack(items: &[(usize, u64)], cap: usize) -> (u64, Vec<usize>) {
    let n = items.len();
    let mut dp = vec![vec![0u64; cap + 1]; n + 1];
    for i in 1..=n {
        let (w, v) = items[i - 1];
        for c in 0..=cap {
            dp[i][c] = dp[i - 1][c];
            if c >= w {
                dp[i][c] = dp[i][c].max(dp[i - 1][c - w] + v);
            }
        }
    }
    let mut chosen = Vec::new();
    let mut c = cap;
    for i in (1..=n).rev() {
        if dp[i][c] != dp[i - 1][c] {
            chosen.push(i - 1);
            c -= items[i - 1].0;
        }
    }
    chosen.reverse();
    (dp[n][cap], chosen)
}

fn lcs(a: &str, b: &str) -> String {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, String::new());
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(a[i]);
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn main() {
    let items = [(12, 4), (2, 2), (1, 1), (1, 2), (4, 10)];
    let (best, chosen) = knapsack(&items, 15);
    println!("{best} {chosen:?}");
    println!("{}", lcs("AGGTAB", "GXTXAYB"));
    println!("{:?}", lcs("abc", "def"));
}
