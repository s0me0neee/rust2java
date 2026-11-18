// Dijkstra over a small weighted graph: a BinaryHeap of Reverse((dist, node)),
// a BTreeMap adjacency list, distance and parent tables, and path rebuilding.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, HashMap};

const INF: i64 = i64::MAX / 4;

#[derive(Debug, Clone)]
struct Edge {
    to: usize,
    weight: i64,
}

#[derive(Debug, Clone)]
struct Graph {
    nodes: Vec<String>,
    edges: BTreeMap<usize, Vec<Edge>>,
}

impl Graph {
    fn new(names: &[&str]) -> Graph {
        Graph {
            nodes: names.iter().map(|name| name.to_string()).collect(),
            edges: BTreeMap::new(),
        }
    }

    fn index_of(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|node| node == name)
    }

    fn connect(&mut self, from: &str, to: &str, weight: i64) {
        let a = self.index_of(from).unwrap();
        let b = self.index_of(to).unwrap();
        self.edges.entry(a).or_default().push(Edge { to: b, weight });
        self.edges.entry(b).or_default().push(Edge { to: a, weight });
    }

    fn neighbors(&self, node: usize) -> &[Edge] {
        self.edges.get(&node).map(|edges| edges.as_slice()).unwrap_or(&[])
    }

    fn dijkstra(&self, source: &str) -> (Vec<i64>, Vec<Option<usize>>) {
        let start = self.index_of(source).expect("unknown source");
        let mut dist = vec![INF; self.nodes.len()];
        let mut parent: Vec<Option<usize>> = vec![None; self.nodes.len()];
        let mut heap: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
        dist[start] = 0;
        heap.push(Reverse((0, start)));
        let mut settled = 0;
        while let Some(Reverse((d, node))) = heap.pop() {
            if d > dist[node] {
                continue;
            }
            settled += 1;
            for edge in self.neighbors(node) {
                let candidate = d + edge.weight;
                if candidate < dist[edge.to] {
                    dist[edge.to] = candidate;
                    parent[edge.to] = Some(node);
                    heap.push(Reverse((candidate, edge.to)));
                }
            }
        }
        println!("settled {} of {} nodes", settled, self.nodes.len());
        (dist, parent)
    }

    fn path_to(&self, parent: &[Option<usize>], target: usize) -> Vec<String> {
        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(node) = current {
            path.push(self.nodes[node].clone());
            current = parent[node];
        }
        path.reverse();
        path
    }

    fn report(&self, dist: &[i64]) {
        for (node, name) in self.nodes.iter().enumerate() {
            if dist[node] >= INF {
                println!("{:<8} unreachable", name);
            } else {
                println!("{:<8} {:>4}", name, dist[node]);
            }
        }
        let reachable: Vec<usize> = (0..dist.len()).filter(|i| dist[*i] < INF).collect();
        let total: i64 = reachable.iter().map(|i| dist[*i]).sum();
        let average = total as f64 / reachable.len() as f64;
        println!("reachable: {}, average {:.3}", reachable.len(), average);
        let farthest = reachable.iter().max_by_key(|i| dist[**i]);
        println!("farthest: {:?}", farthest.map(|i| &self.nodes[*i]));
    }
}

fn mst_weight(graph: &Graph) -> i64 {
    let mut in_tree = vec![false; graph.nodes.len()];
    let mut best = vec![INF; graph.nodes.len()];
    best[0] = 0;
    let mut heap: BinaryHeap<Reverse<(i64, usize)>> = BinaryHeap::new();
    heap.push(Reverse((0, 0)));
    let mut total = 0;
    let mut chosen = Vec::new();
    while let Some(Reverse((weight, node))) = heap.pop() {
        if in_tree[node] || weight > best[node] {
            continue;
        }
        in_tree[node] = true;
        total += weight;
        chosen.push(graph.nodes[node].clone());
        for edge in graph.neighbors(node) {
            if !in_tree[edge.to] && edge.weight < best[edge.to] {
                best[edge.to] = edge.weight;
                heap.push(Reverse((edge.weight, edge.to)));
            }
        }
    }
    println!("mst nodes (in order): {:?}", chosen);
    total
}

fn main() {
    let mut graph = Graph::new(&["a", "b", "c", "d", "e", "f", "g", "h"]);
    let links = [
        ("a", "b", 4),
        ("a", "c", 2),
        ("b", "c", 5),
        ("b", "d", 10),
        ("c", "e", 3),
        ("d", "e", 4),
        ("d", "f", 11),
        ("e", "f", 5),
        ("f", "g", 2),
        ("g", "h", 3),
    ];
    for (from, to, weight) in links {
        graph.connect(from, to, weight);
    }
    println!("nodes: {}, directed links: {}", graph.nodes.len(), links.len());
    let (dist, parent) = graph.dijkstra("a");
    graph.report(&dist);
    for target in graph.nodes.iter() {
        if let Some(index) = graph.index_of(target) {
            if dist[index] < INF {
                let path = graph.path_to(&parent, index);
                println!("{}: {}", target, path.join(" -> "));
            }
        }
    }
    let mut by_distance: HashMap<i64, Vec<String>> = HashMap::new();
    for (index, name) in graph.nodes.iter().enumerate() {
        if dist[index] < INF {
            by_distance
                .entry(dist[index])
                .or_default()
                .push(name.clone());
        }
    }
    let mut rows: Vec<(i64, String)> = by_distance
        .into_iter()
        .map(|(distance, names)| (distance, names.join(", ")))
        .collect();
    rows.sort();
    for (distance, names) in rows {
        println!("{:>4}: {}", distance, names);
    }
    println!("mst weight: {}", mst_weight(&graph));
    let missing = graph.dijkstra("h");
    println!("from h: {} nodes reached", missing.0.iter().filter(|d| **d < INF).count());
}
