use std::cmp::Reverse;
use std::collections::BinaryHeap;

fn dijkstra(adj: &[Vec<(usize, u64)>], src: usize) -> Vec<Option<u64>> {
    let mut dist: Vec<Option<u64>> = vec![None; adj.len()];
    let mut heap = BinaryHeap::new();
    dist[src] = Some(0);
    heap.push(Reverse((0u64, src)));
    while let Some(Reverse((d, u))) = heap.pop() {
        if dist[u].is_some_and(|best| d > best) {
            continue;
        }
        for &(v, w) in &adj[u] {
            let nd = d + w;
            if dist[v].map_or(true, |cur| nd < cur) {
                dist[v] = Some(nd);
                heap.push(Reverse((nd, v)));
            }
        }
    }
    dist
}

fn main() {
    let n = 6;
    let mut adj = vec![Vec::new(); n];
    for &(a, b, w) in &[(0, 1, 7u64), (0, 2, 9), (0, 5, 14), (1, 2, 10), (1, 3, 15), (2, 3, 11), (2, 5, 2), (3, 4, 6)] {
        adj[a].push((b, w));
        adj[b].push((a, w));
    }
    let d = dijkstra(&adj, 0);
    let text: Vec<String> = d.iter().map(|x| x.map_or("inf".to_string(), |v| v.to_string())).collect();
    println!("{}", text.join(" "));
    adj.push(Vec::new());
    println!("{:?}", dijkstra(&adj, 4).last());
}
