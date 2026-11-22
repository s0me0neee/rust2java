fn dfs(u: usize, adj: &Vec<Vec<usize>>, depth: &mut Vec<usize>, d: usize) {
    depth[u] = d;
    for &v in &adj[u] { dfs(v, adj, depth, d + 1); }
}
fn sum_to(n: u64) -> u64 { if n == 0 { 0 } else { n + sum_to(n - 1) } }
fn main() {
    let n = 100_000;
    let mut adj = vec![Vec::new(); n];
    for i in 0..n - 1 { adj[i].push(i + 1); }
    let mut depth = vec![0; n];
    dfs(0, &adj, &mut depth, 0);
    println!("{} {}", depth[n - 1], sum_to(100_000));
}
