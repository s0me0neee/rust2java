use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

struct Dsu { p: Vec<usize>, sz: Vec<usize> }
impl Dsu {
    fn new(n: usize) -> Self { Dsu { p: (0..n).collect(), sz: vec![1; n] } }
    fn find(&mut self, x: usize) -> usize { if self.p[x] != x { let r = self.find(self.p[x]); self.p[x] = r; } self.p[x] }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut a, mut b) = (self.find(a), self.find(b));
        if a == b { return false; }
        if self.sz[a] < self.sz[b] { std::mem::swap(&mut a, &mut b); }
        self.p[b] = a; self.sz[a] += self.sz[b]; true
    }
}
struct Seg { n: usize, t: Vec<i64> }
impl Seg {
    fn new(a: &[i64]) -> Self { let n = a.len(); let mut t = vec![0; 2 * n]; t[n..].copy_from_slice(a); for i in (1..n).rev() { t[i] = t[2 * i] + t[2 * i + 1]; } Seg { n, t } }
    fn update(&mut self, mut i: usize, v: i64) { i += self.n; self.t[i] = v; while i > 1 { i /= 2; self.t[i] = self.t[2 * i] + self.t[2 * i + 1]; } }
    fn query(&self, mut l: usize, mut r: usize) -> i64 { let mut s = 0; l += self.n; r += self.n; while l < r { if l & 1 == 1 { s += self.t[l]; l += 1; } if r & 1 == 1 { r -= 1; s += self.t[r]; } l >>= 1; r >>= 1; } s }
}
fn bfs(grid: &[&str]) -> Option<usize> {
    let g: Vec<Vec<u8>> = grid.iter().map(|r| r.bytes().collect()).collect();
    let (h, w) = (g.len(), g[0].len());
    let mut dist = vec![vec![usize::MAX; w]; h];
    let mut q = VecDeque::new();
    dist[0][0] = 0; q.push_back((0usize, 0usize));
    while let Some((r, c)) = q.pop_front() {
        for (dr, dc) in [(0i32, 1i32), (1, 0), (0, -1), (-1, 0)] {
            let (nr, nc) = (r as i32 + dr, c as i32 + dc);
            if nr < 0 || nc < 0 || nr >= h as i32 || nc >= w as i32 { continue; }
            let (nr, nc) = (nr as usize, nc as usize);
            if g[nr][nc] == b'#' || dist[nr][nc] != usize::MAX { continue; }
            dist[nr][nc] = dist[r][c] + 1;
            q.push_back((nr, nc));
        }
    }
    let d = dist[h - 1][w - 1];
    if d == usize::MAX { None } else { Some(d) }
}
fn dijkstra(n: usize, edges: &[(usize, usize, u64)], s: usize) -> Vec<Option<u64>> {
    let mut adj = vec![vec![]; n];
    for &(a, b, w) in edges { adj[a].push((b, w)); adj[b].push((a, w)); }
    let mut dist = vec![None; n];
    let mut pq = BinaryHeap::new();
    pq.push(Reverse((0u64, s)));
    while let Some(Reverse((d, u))) = pq.pop() {
        if dist[u].is_some() { continue; }
        dist[u] = Some(d);
        for &(v, w) in &adj[u] { if dist[v].is_none() { pq.push(Reverse((d + w, v))); } }
    }
    dist
}
fn kmp(text: &str, pat: &str) -> Vec<usize> {
    let (t, p) = (text.as_bytes(), pat.as_bytes());
    let mut f = vec![0; p.len()];
    let mut k = 0;
    for i in 1..p.len() { while k > 0 && p[i] != p[k] { k = f[k - 1]; } if p[i] == p[k] { k += 1; } f[i] = k; }
    let mut res = vec![]; k = 0;
    for (i, &c) in t.iter().enumerate() { while k > 0 && c != p[k] { k = f[k - 1]; } if c == p[k] { k += 1; } if k == p.len() { res.push(i + 1 - k); k = f[k - 1]; } }
    res
}
fn modpow(mut b: u64, mut e: u64, m: u64) -> u64 { let mut r = 1; b %= m; while e > 0 { if e & 1 == 1 { r = r * b % m; } b = b * b % m; e >>= 1; } r }
fn gcd(a: u64, b: u64) -> u64 { if b == 0 { a } else { gcd(b, a % b) } }
fn perms(v: &mut Vec<i32>, k: usize, out: &mut Vec<String>) {
    if k == v.len() { out.push(v.iter().map(|x| x.to_string()).collect()); return; }
    for i in k..v.len() { v.swap(k, i); perms(v, k + 1, out); v.swap(k, i); }
}
fn main() {
    let mut d = Dsu::new(6);
    println!("{} {} {} {}", d.union(0, 1), d.union(1, 2), d.union(0, 2), d.find(2) == d.find(0));
    let mut s = Seg::new(&[5, 3, 7, 9, 6, 4, 1, 2]);
    println!("{} {}", s.query(0, 8), s.query(2, 5));
    s.update(3, -1);
    println!("{}", s.query(1, 4));
    println!("{:?} {:?}", bfs(&["..#.", "#...", "..#."]), bfs(&[".#", "#."]));
    println!("{:?}", dijkstra(5, &[(0, 1, 4), (0, 2, 1), (2, 1, 2), (1, 3, 5)], 0));
    println!("{:?}", kmp("abababcabab", "abab"));
    println!("{} {} {}", modpow(2, 1_000_000, 1_000_000_007), gcd(84, 36), 84 / gcd(84, 36) * 36);
    let sieve = { let n = 50; let mut is = vec![true; n + 1]; is[0] = false; is[1] = false; let mut i = 2; while i * i <= n { if is[i] { let mut j = i * i; while j <= n { is[j] = false; j += i; } } i += 1; } (0..=n).filter(|&i| is[i]).collect::<Vec<_>>() };
    println!("{:?}", sieve);
    let mut out = vec![]; perms(&mut vec![1, 2, 3], 0, &mut out);
    println!("{}", out.join(","));
    let w = [3, 4, 5]; let v = [30, 50, 60]; let cap = 8;
    let best = (0u32..1 << w.len()).filter(|m| (0..w.len()).filter(|i| m >> i & 1 == 1).map(|i| w[i]).sum::<i32>() <= cap).map(|m| (0..w.len()).filter(|i| m >> i & 1 == 1).map(|i| v[i]).sum::<i32>()).max();
    println!("{:?}", best);
}
