// Dynamic programming: edit distance, LCS reconstruction, knapsack, coin change, LIS with binary search.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut dp = vec![vec![0usize; b.len() + 1]; a.len() + 1];
    for i in 0..=a.len() {
        dp[i][0] = i;
    }
    for j in 0..=b.len() {
        dp[0][j] = j;
    }
    for i in 1..=a.len() {
        for j in 1..=b.len() {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1).min(dp[i][j - 1] + 1).min(dp[i - 1][j - 1] + cost);
        }
    }
    dp[a.len()][b.len()]
}

fn lcs(a: &[u8], b: &[u8]) -> String {
    let (n, m) = (a.len(), b.len());
    let mut dp = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if a[i] == b[j] { dp[i + 1][j + 1] + 1 } else { dp[i + 1][j].max(dp[i][j + 1]) };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut out = String::new();
    while i < n && j < m {
        if a[i] == b[j] {
            out.push(a[i] as char);
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

fn knapsack(items: &[(usize, u64)], cap: usize) -> (u64, Vec<usize>) {
    let n = items.len();
    let mut best = vec![vec![0u64; cap + 1]; n + 1];
    for (i, &(w, v)) in items.iter().enumerate() {
        for c in 0..=cap {
            best[i + 1][c] = best[i][c];
            if c >= w && best[i][c - w] + v > best[i + 1][c] {
                best[i + 1][c] = best[i][c - w] + v;
            }
        }
    }
    let mut chosen = Vec::new();
    let mut c = cap;
    for i in (1..=n).rev() {
        if best[i][c] != best[i - 1][c] {
            chosen.push(i - 1);
            c -= items[i - 1].0;
        }
    }
    chosen.reverse();
    (best[n][cap], chosen)
}

fn coin_change(coins: &[usize], target: usize) -> Option<usize> {
    let mut dp = vec![usize::MAX; target + 1];
    dp[0] = 0;
    for t in 1..=target {
        for &c in coins {
            if c <= t && dp[t - c] != usize::MAX {
                dp[t] = dp[t].min(dp[t - c] + 1);
            }
        }
    }
    if dp[target] == usize::MAX { None } else { Some(dp[target]) }
}

fn lis(xs: &[i64]) -> usize {
    let mut tails: Vec<i64> = Vec::new();
    for &x in xs {
        match tails.binary_search(&x) {
            Ok(_) => {}
            Err(pos) if pos == tails.len() => tails.push(x),
            Err(pos) => tails[pos] = x,
        }
    }
    tails.len()
}

fn main() {
    for (a, b) in [("kitten", "sitting"), ("flaw", "lawn"), ("", "abc"), ("same", "same")] {
        println!("{a:?} -> {b:?}: {}", edit_distance(a, b));
    }
    println!("lcs={}", lcs(b"AGGTAB", b"GXTXAYB"));
    let items = [(12, 4), (2, 2), (1, 1), (1, 2), (4, 10)];
    let (value, chosen) = knapsack(&items, 15);
    println!("knapsack value={} items={:?}", value, chosen);
    println!("{:?} {:?} {:?}", coin_change(&[1, 5, 10, 25], 63), coin_change(&[2], 3), coin_change(&[3, 7], 0));
    println!("lis={}", lis(&[10, 9, 2, 5, 3, 7, 101, 18, -4, 200]));
    let tri: Vec<Vec<u64>> = (0..6).fold(Vec::new(), |mut rows: Vec<Vec<u64>>, i| {
        let row = (0..=i).map(|j| if j == 0 || j == i { 1 } else { rows[i - 1][j - 1] + rows[i - 1][j] }).collect();
        rows.push(row);
        rows
    });
    for row in &tri {
        let s: Vec<String> = row.iter().map(|v| v.to_string()).collect();
        println!("{:^20}", s.join(" "));
    }
    let fib: Vec<u64> = (0..90).scan((0u64, 1u64), |st, _| {
        let r = st.0;
        *st = (st.1, st.0 + st.1);
        Some(r)
    }).collect();
    println!("{} {}", fib[89], fib.iter().filter(|&&f| f % 2 == 0).count());
}
