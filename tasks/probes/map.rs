use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque, BinaryHeap};
use std::cmp::Reverse;
fn main() {
    let mut m: HashMap<String, i32> = HashMap::new();
    m.insert("a".to_string(), 1);
    m.insert("b".to_string(), 2);
    *m.entry("a".to_string()).or_insert(0) += 5;
    m.entry("c".to_string()).or_default();
    *m.entry("d".to_string()).or_insert_with(|| 9) -= 1;
    println!("{:?} {:?}", m.get("a"), m.get("z"));
    println!("{} {}", m.contains_key("b"), m.len());
    if let Some(v) = m.get_mut("b") {
        *v *= 10;
    }
    println!("{}", m["b"]);
    let removed = m.remove("c");
    println!("{:?}", removed);
    let mut keys: Vec<_> = m.keys().cloned().collect();
    keys.sort();
    println!("{:?}", keys);
    let mut vals: Vec<i32> = m.values().copied().collect();
    vals.sort();
    println!("{:?}", vals);
    let mut pairs: Vec<(&String, &i32)> = m.iter().collect();
    pairs.sort();
    println!("{:?}", pairs);
    let mut counts = BTreeMap::new();
    for w in "the cat the dog the end".split(' ') {
        *counts.entry(w).or_insert(0) += 1;
    }
    println!("{:?}", counts);
    for (k, v) in &counts {
        println!("{} -> {}", k, v);
    }
    println!("{:?}", counts.iter().next());
    println!("{:?}", counts.keys().last());
    println!("{:?}", counts.first_key_value());
    let mut bt: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    bt.entry(1).or_insert_with(Vec::new).push(5);
    bt.entry(1).or_default().push(6);
    println!("{:?}", bt);
    println!("{:?}", bt.range(0..5).count());
    let mut hs: HashSet<i32> = HashSet::new();
    println!("{} {}", hs.insert(3), hs.insert(3));
    hs.extend([4, 5]);
    println!("{} {}", hs.contains(&4), hs.len());
    let other: HashSet<i32> = [5, 6].into_iter().collect();
    let mut inter: Vec<_> = hs.intersection(&other).collect();
    inter.sort();
    println!("{:?}", inter);
    let mut uni: Vec<_> = hs.union(&other).copied().collect();
    uni.sort();
    println!("{:?}", uni);
    let bs: BTreeSet<char> = "hello".chars().collect();
    println!("{:?}", bs);
    println!("{:?} {:?}", bs.first(), bs.iter().next_back());
    let mut q = VecDeque::new();
    q.push_back(1);
    q.push_back(2);
    q.push_front(0);
    println!("{:?} {:?} {:?}", q, q.front(), q.back());
    println!("{:?} {:?}", q.pop_front(), q.pop_back());
    println!("{} {}", q.len(), q[0]);
    let mut heap = BinaryHeap::new();
    heap.push(3);
    heap.push(7);
    heap.push(1);
    println!("{:?} {}", heap.peek(), heap.len());
    println!("{:?}", heap.pop());
    let mut minh = BinaryHeap::new();
    minh.push(Reverse(5));
    minh.push(Reverse(2));
    if let Some(Reverse(x)) = minh.pop() {
        println!("{}", x);
    }
    let mut pq: BinaryHeap<(i32, usize)> = BinaryHeap::new();
    pq.push((2, 1));
    pq.push((9, 0));
    println!("{:?}", pq.pop());
    let sorted_heap = heap.into_sorted_vec();
    println!("{:?}", sorted_heap);
    let hm2: HashMap<&str, i32> = vec![("x", 1), ("y", 2)].into_iter().collect();
    let mut hk: Vec<_> = hm2.into_iter().collect();
    hk.sort();
    println!("{:?}", hk);
    let mut grid: HashMap<(i32, i32), char> = HashMap::new();
    grid.insert((0, 1), '#');
    println!("{:?}", grid.get(&(0, 1)));
    println!("{}", grid.get(&(5, 5)).copied().unwrap_or('.'));
    let mut seen = HashSet::new();
    let dupes: Vec<i32> = vec![1, 2, 1, 3, 2].into_iter().filter(|x| !seen.insert(*x)).collect();
    println!("{:?}", dupes);
    m.retain(|_, v| *v > 5);
    println!("{}", m.len());
}
