use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashSet, VecDeque};

#[derive(Debug, PartialEq, Eq)]
struct Job { prio: u8, id: usize, cost: u32 }
impl Ord for Job {
    fn cmp(&self, o: &Self) -> Ordering { self.prio.cmp(&o.prio).then(o.id.cmp(&self.id)) }
}
impl PartialOrd for Job { fn partial_cmp(&self, o: &Self) -> Option<Ordering> { Some(self.cmp(o)) } }

fn topo(n: usize, edges: &[(usize, usize)]) -> Option<Vec<usize>> {
    let mut indeg = vec![0; n];
    let mut adj = vec![Vec::new(); n];
    for &(a, b) in edges { adj[a].push(b); indeg[b] += 1; }
    let mut q: VecDeque<usize> = (0..n).filter(|&i| indeg[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    while let Some(u) = q.pop_front() {
        order.push(u);
        for &v in &adj[u] { indeg[v] -= 1; if indeg[v] == 0 { q.push_back(v); } }
    }
    if order.len() == n { Some(order) } else { None }
}

fn main() {
    let mut heap = BinaryHeap::new();
    for (id, (prio, cost)) in [(2u8, 10u32), (5, 3), (2, 7), (9, 1), (5, 8)].into_iter().enumerate() {
        heap.push(Job { prio, id, cost });
    }
    let mut t = 0u32;
    while let Some(Job { prio, id, cost }) = heap.pop() {
        t += cost;
        println!("t={t:>3} job#{id} prio={prio}");
    }
    println!("{:?}", topo(4, &[(0, 1), (1, 2), (0, 3), (3, 2)]));
    println!("{:?}", topo(3, &[(0, 1), (1, 2), (2, 0)]));
    let seen: HashSet<u32> = [3, 1, 4, 1, 5, 9, 2, 6].into_iter().collect();
    let mut s: Vec<_> = seen.into_iter().collect();
    s.sort_unstable_by(|a, b| b.cmp(a));
    println!("{s:?}");
    let windows: Vec<i32> = [1, 3, 2, 5, 4].windows(2).map(|w| w[1] - w[0]).collect();
    let chunks: Vec<Vec<i32>> = (1..=7).collect::<Vec<_>>().chunks(3).map(|c| c.to_vec()).collect();
    println!("{windows:?} {chunks:?}");
    let pos = s.binary_search_by(|x| 4.cmp(x));
    println!("{pos:?} {:?}", s.iter().step_by(2).collect::<Vec<_>>());
}
