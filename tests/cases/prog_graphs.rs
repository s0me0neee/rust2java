// Graph algorithms: Dijkstra with BinaryHeap<Reverse<(u64, usize)>>, adjacency Vec<Vec<(usize, u64)>>,
// topological sort with in-degree VecDeque, union-find with path compression, peek/pop/into_sorted_vec.
use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

struct Graph {
    adj: Vec<Vec<(usize, u64)>>,
}

impl Graph {
    fn new(n: usize) -> Self {
        Graph { adj: vec![Vec::new(); n] }
    }
    fn add(&mut self, a: usize, b: usize, w: u64) {
        self.adj[a].push((b, w));
    }
    fn dijkstra(&self, src: usize) -> (Vec<Option<u64>>, Vec<usize>) {
        let n = self.adj.len();
        let mut dist = vec![None; n];
        let mut parent = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();
        dist[src] = Some(0);
        heap.push(Reverse((0u64, src)));
        while let Some(Reverse((d, u))) = heap.pop() {
            if dist[u].is_some_and(|best| d > best) {
                continue;
            }
            for &(v, w) in &self.adj[u] {
                let nd = d + w;
                if dist[v].map_or(true, |cur| nd < cur) {
                    dist[v] = Some(nd);
                    parent[v] = u;
                    heap.push(Reverse((nd, v)));
                }
            }
        }
        (dist, parent)
    }
    fn topo(&self) -> Option<Vec<usize>> {
        let n = self.adj.len();
        let mut indeg = vec![0; n];
        for edges in &self.adj {
            for &(v, _) in edges {
                indeg[v] += 1;
            }
        }
        let mut q: VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(u) = q.pop_front() {
            order.push(u);
            for &(v, _) in &self.adj[u] {
                indeg[v] -= 1;
                if indeg[v] == 0 {
                    q.push_back(v);
                }
            }
        }
        if order.len() == n { Some(order) } else { None }
    }
}

struct Dsu {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl Dsu {
    fn new(n: usize) -> Self {
        Dsu { parent: (0..n).collect(), size: vec![1; n] }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            let root = self.find(self.parent[x]);
            self.parent[x] = root;
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let (mut ra, mut rb) = (self.find(a), self.find(b));
        if ra == rb {
            return false;
        }
        if self.size[ra] < self.size[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        self.size[ra] += self.size[rb];
        true
    }
}

fn path(parent: &[usize], mut t: usize) -> Vec<usize> {
    let mut p = vec![t];
    while parent[t] != usize::MAX {
        t = parent[t];
        p.push(t);
    }
    p.into_iter().rev().collect()
}

fn main() {
    let edges = [(0, 1, 4), (0, 2, 1), (2, 1, 2), (1, 3, 1), (2, 3, 5), (3, 4, 3)];
    let mut g = Graph::new(6);
    for &(a, b, w) in edges.iter() {
        g.add(a, b, w);
    }
    let (dist, parent) = g.dijkstra(0);
    for (i, d) in dist.iter().enumerate() {
        match d {
            Some(d) => println!("{} dist {} via {:?}", i, d, path(&parent, i)),
            None => println!("{} unreachable", i),
        }
    }
    println!("topo {:?}", g.topo());
    g.add(4, 0, 1);
    println!("topo after cycle {:?}", g.topo());

    let mut mst_edges: Vec<(u64, usize, usize)> = edges.iter().map(|&(a, b, w)| (w, a, b)).collect();
    mst_edges.sort_unstable();
    let mut dsu = Dsu::new(6);
    let mut cost = 0;
    let mut used = vec![];
    for (w, a, b) in mst_edges {
        if dsu.union(a, b) {
            cost += w;
            used.push((a, b));
        }
    }
    println!("mst {} {:?}", cost, used);
    let roots: std::collections::BTreeSet<usize> = (0..6).map(|i| dsu.find(i)).collect();
    println!("components {}", roots.len());

    let mut heap: BinaryHeap<(u32, &str)> = BinaryHeap::new();
    for (p, name) in [(3, "low"), (9, "urgent"), (5, "mid"), (9, "also")] {
        heap.push((p, name));
    }
    println!("peek {:?} len {}", heap.peek(), heap.len());
    if let Some((_, name)) = heap.pop() {
        heap.push((1, name));
    }
    let first = heap.pop().unwrap();
    println!("pop {:?}", first);
    println!("sorted {:?}", heap.into_sorted_vec());
    let mut mins: BinaryHeap<Reverse<i32>> = vec![5, 1, 8, 3].into_iter().map(Reverse).collect();
    mins.push(Reverse(0));
    let drained: Vec<i32> = std::iter::from_fn(|| mins.pop().map(|Reverse(x)| x)).collect();
    println!("{:?}", drained);
}
