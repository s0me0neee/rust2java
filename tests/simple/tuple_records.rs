// Tuples the program builds: an aliased grid position, weighted edges, a set of points; map items stay entries.
use std::collections::{HashMap, HashSet, VecDeque};

type Pos = (usize, usize);

fn neighbors(p: Pos, rows: usize, cols: usize) -> Vec<Pos> {
    let (r, c) = p;
    let mut out = Vec::new();
    if r > 0 { out.push((r - 1, c)); }
    if c > 0 { out.push((r, c - 1)); }
    if r + 1 < rows { out.push((r + 1, c)); }
    if c + 1 < cols { out.push((r, c + 1)); }
    out
}

fn main() {
    let grid = ["..#", "...", "#.."];
    let start: Pos = (0, 0);
    let mut dist: HashMap<Pos, u32> = HashMap::new();
    let mut q = VecDeque::new();
    dist.insert(start, 0);
    q.push_back(start);
    while let Some(cur) = q.pop_front() {
        for n in neighbors(cur, 3, 3) {
            let (r, c) = n;
            if grid[r].as_bytes()[c] == b'#' || dist.contains_key(&n) { continue; }
            dist.insert(n, dist[&cur] + 1);
            q.push_back(n);
        }
    }
    println!("{}", dist[&(2, 2)]);

    let mut edges: Vec<(usize, usize, i64)> = vec![(0, 1, 5), (1, 2, 3), (0, 2, 9)];
    edges.sort_by_key(|&(_, _, w)| w);
    for (from, to, weight) in &edges {
        println!("{from}->{to} {weight}");
    }
    let total: i64 = edges.iter().map(|e| e.2).sum();
    let heavy = edges.iter().filter(|(_, _, w)| *w > 4).count();
    println!("{total} {heavy} {:?}", edges[0]);

    let mut seen: HashSet<(i32, i32)> = HashSet::new();
    seen.insert((1, 2));
    println!("{}", seen.contains(&(1, 2)));

    let scores: HashMap<&str, i32> = [("a", 1), ("b", 2)].into_iter().collect();
    let mut keys: Vec<_> = scores.iter().map(|(k, v)| format!("{k}{v}")).collect();
    keys.sort();
    println!("{:?}", keys);
    for (i, e) in edges.iter().enumerate() {
        if i == 0 { println!("{} {}", e.0, e.1); }
    }
}
