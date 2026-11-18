// Job scheduler: traits with default methods, generics with bounds, BinaryHeap with Reverse, sort_by_key.
use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap, HashSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Job {
    id: u32,
    priority: u8,
    duration: u32,
    deps: Vec<u32>,
}

impl Ord for Job {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority
            .cmp(&other.priority)
            .then_with(|| other.duration.cmp(&self.duration))
            .then(other.id.cmp(&self.id))
    }
}

impl PartialOrd for Job {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

trait Describe {
    fn name(&self) -> String;
    fn describe(&self) -> String {
        format!("<{}>", self.name())
    }
}

impl Describe for Job {
    fn name(&self) -> String {
        format!("job#{}", self.id)
    }
}

fn largest<T: PartialOrd + Copy>(xs: &[T]) -> Option<T> {
    let mut it = xs.iter();
    let mut best = *it.next()?;
    for &x in it {
        if x > best {
            best = x;
        }
    }
    Some(best)
}

fn topo(jobs: &[Job]) -> Option<Vec<u32>> {
    let mut indeg: BTreeMap<u32, usize> = jobs.iter().map(|j| (j.id, j.deps.len())).collect();
    let mut queue: VecDeque<u32> = indeg.iter().filter(|(_, d)| **d == 0).map(|(&id, _)| id).collect();
    let mut order = vec![];
    while let Some(id) = queue.pop_front() {
        order.push(id);
        for j in jobs.iter().filter(|j| j.deps.contains(&id)) {
            let d = indeg.get_mut(&j.id).unwrap();
            *d -= 1;
            if *d == 0 {
                queue.push_back(j.id);
            }
        }
    }
    if order.len() == jobs.len() { Some(order) } else { None }
}

fn main() {
    let jobs = vec![
        Job { id: 1, priority: 3, duration: 10, deps: vec![] },
        Job { id: 2, priority: 5, duration: 4, deps: vec![1] },
        Job { id: 3, priority: 5, duration: 2, deps: vec![1] },
        Job { id: 4, priority: 1, duration: 7, deps: vec![2, 3] },
        Job { id: 5, priority: 3, duration: 10, deps: vec![] },
    ];

    let mut heap: BinaryHeap<Job> = jobs.iter().cloned().collect();
    let mut by_prio = Vec::new();
    while let Some(j) = heap.pop() {
        by_prio.push(j.describe());
    }
    println!("{}", by_prio.join(" "));

    let mut short_first = BinaryHeap::new();
    for j in &jobs {
        short_first.push(Reverse((j.duration, j.id)));
    }
    let Reverse((d, id)) = short_first.peek().unwrap();
    println!("shortest: {} ({})", id, d);
    let firsts: Vec<_> = std::iter::from_fn(|| short_first.pop().map(|Reverse(p)| p)).take(3).collect();
    println!("{:?}", firsts);

    println!("topo: {:?}", topo(&jobs));
    let mut cyclic = jobs.clone();
    cyclic[0].deps.push(4);
    println!("cyclic topo: {:?}", topo(&cyclic));

    let durations: Vec<u32> = jobs.iter().map(|j| j.duration).collect();
    println!("largest duration {:?}, largest of none {:?}", largest(&durations), largest::<f64>(&[]));
    println!("largest float {:?}", largest(&[1.5, -2.0, 9.25]));

    let mut sorted = jobs.clone();
    sorted.sort_by_key(|j| (Reverse(j.priority), j.duration));
    println!("{:?}", sorted.iter().map(|j| j.id).collect::<Vec<_>>());
    sorted.sort();
    println!("{:?}", sorted.iter().map(|j| j.id).collect::<Vec<_>>());
    println!("max {:?}", jobs.iter().max().map(|j| j.id));
    println!("min by duration {:?}", jobs.iter().min_by_key(|j| j.duration).map(|j| j.name()));

    let total: u32 = jobs.iter().filter(|j| j.priority >= 3).map(|j| j.duration).sum();
    let deps: HashSet<u32> = jobs.iter().flat_map(|j| j.deps.iter().copied()).collect();
    let mut deps: Vec<_> = deps.into_iter().collect();
    deps.sort_unstable();
    println!("total {} deps {:?}", total, deps);
    let leaf = jobs.iter().position(|j| !deps.contains(&j.id));
    println!("first leaf idx {:?}", leaf);
    println!("{}", jobs[0] < jobs[4]);
    println!("{:?}", jobs[1].cmp(&jobs[2]));
}
