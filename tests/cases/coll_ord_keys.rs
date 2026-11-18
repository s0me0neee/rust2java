// Ordered collections, sorts, heaps and min/max over keys Java can't compare by itself.
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Ver {
    major: u32,
    tags: Vec<String>,
}

fn main() {
    let a = 1;
    let b = 2;
    let mut paths: BTreeSet<Vec<i32>> = BTreeSet::new();
    paths.insert(vec![2, 1]);
    paths.insert(vec![1, 5, 0]);
    paths.insert(vec![1, 5]);
    paths.insert(vec![a, b]);
    println!("{:?} {} {}", paths, paths.len(), paths.contains(&vec![1, 5]));
    println!("{:?} {:?}", paths.first(), paths.last());

    let grid: BTreeSet<Vec<Vec<i32>>> = [vec![vec![1, 2]], vec![vec![1], vec![9]], vec![]]
        .into_iter()
        .collect();
    println!("{:?}", grid);

    let mut seen: BTreeMap<(i32, String), u32> = BTreeMap::new();
    seen.insert((2, "a".to_string()), 1);
    seen.insert((1, "z".to_string()), 2);
    *seen.entry((1, "b".to_string())).or_insert(0) += 3;
    println!("{:?}", seen);

    let from: BTreeMap<Vec<u8>, char> = BTreeMap::from([(vec![2], 'x'), (vec![1, 1], 'y')]);
    println!("{:?}", from);
    let points = BTreeSet::from([(3, 1), (1, 2), (1, 1)]);
    println!("{:?}", points);

    let mut rows = vec![vec![3], vec![1, 2], vec![1]];
    rows.sort();
    println!("{:?} {:?} {:?}", rows, rows.iter().max(), rows.iter().min());
    let mut pairs = vec![("b", vec![2]), ("a", vec![9]), ("b", vec![1])];
    pairs.sort_unstable();
    println!("{:?}", pairs);

    let mut heap = BinaryHeap::new();
    heap.push((5, vec![1]));
    heap.push((5, vec![0, 9]));
    heap.push((1, vec![7]));
    while let Some(top) = heap.pop() {
        print!("{:?} ", top);
    }
    println!();
    let mut dist = BinaryHeap::new();
    dist.push(Reverse((4000000000u64, 1usize)));
    dist.push(Reverse((7u64, 3usize)));
    dist.push(Reverse((7u64, 2usize)));
    while let Some(Reverse((d, node))) = dist.pop() {
        print!("{}:{} ", d, node);
    }
    println!();

    let opts: BTreeSet<Option<i32>> = [Some(2), None, Some(1)].into_iter().collect();
    let wide: BTreeSet<u32> = [4000000000u32, 1, 70000].into_iter().collect();
    let small: BTreeSet<u16> = [65535u16, 1, 300].into_iter().collect();
    println!("{:?} {:?} {:?}", opts, wide, small);
    println!("{:?}", [3000000000u32, 5].iter().max());

    let mut vers = BTreeSet::new();
    vers.insert(Ver { major: 2, tags: vec!["b".into()] });
    vers.insert(Ver { major: 2, tags: vec!["a".into(), "z".into()] });
    vers.insert(Ver { major: 1, tags: vec![] });
    println!("{:?}", vers.iter().map(|v| v.tags.len()).collect::<Vec<_>>());
}
