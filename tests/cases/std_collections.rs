// Collections: HashMap/BTreeMap/HashSet/BTreeSet/VecDeque/BinaryHeap (§11.1).
use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, VecDeque};

fn main() {
    let mut m: HashMap<&str, i32> = HashMap::new();
    m.insert("b", 2);
    m.insert("a", 1);
    m.insert("c", 3);
    println!("{} {} {}", m.len(), m.contains_key("a"), m.contains_key("z"));
    println!("{:?} {:?}", m.get("a"), m.get("z"));
    println!("{:?}", m.remove("b"));
    println!("{}", m.len());
    let mut keys: Vec<&str> = m.keys().copied().collect();
    keys.sort();
    println!("{:?}", keys);
    let mut vals: Vec<i32> = m.values().copied().collect();
    vals.sort();
    println!("{:?}", vals);
    *m.entry("a").or_insert(0) += 10;
    *m.entry("z").or_insert(0) += 5;
    println!("{:?} {:?}", m.get("a"), m.get("z"));

    let mut counts: BTreeMap<String, i32> = BTreeMap::new();
    for w in ["x", "y", "x", "z", "x"] {
        *counts.entry(w.to_string()).or_insert(0) += 1;
    }
    for (k, v) in &counts {
        println!("{k}={v}");
    }

    let mut groups: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
    groups.entry(1).or_insert_with(Vec::new).push(10);
    groups.entry(1).or_insert_with(Vec::new).push(11);
    groups.entry(2).or_default().push(20);
    println!("{:?} {:?}", groups.get(&1), groups.get(&2));
    println!("{:?}", groups.len());

    let mut s: HashSet<i32> = HashSet::new();
    println!("{} {}", s.insert(1), s.insert(1));
    s.insert(2);
    println!("{} {}", s.contains(&2), s.len());
    let other: HashSet<i32> = [2, 3].into_iter().collect();
    let mut inter: Vec<i32> = s.intersection(&other).copied().collect();
    inter.sort();
    let mut uni: Vec<i32> = s.union(&other).copied().collect();
    uni.sort();
    let mut diff: Vec<i32> = s.difference(&other).copied().collect();
    diff.sort();
    println!("{:?} {:?} {:?}", inter, uni, diff);
    println!("{} {}", s.is_subset(&other), s.is_superset(&other));
    println!("{:?}", s.remove(&1));
    println!("{}", s.len());

    let mut bs: BTreeSet<i32> = BTreeSet::new();
    bs.insert(3);
    bs.insert(1);
    bs.insert(2);
    bs.insert(3);
    let ordered: Vec<i32> = bs.iter().copied().collect();
    println!("{:?} {}", ordered, bs.len());
    println!("{} {}", bs.contains(&2), bs.remove(&2));

    let mut dq: VecDeque<i32> = VecDeque::new();
    dq.push_back(1);
    dq.push_back(2);
    dq.push_front(0);
    println!("{:?} {:?} {}", dq.front(), dq.back(), dq.len());
    println!("{:?} {:?}", dq.pop_front(), dq.pop_back());
    println!("{}", dq.len());
    let vd: VecDeque<i32> = vec![1, 2, 3].into();
    println!("{} {}", vd.len(), vd.front().is_some());

    let mut h: BinaryHeap<i32> = BinaryHeap::new();
    h.push(3);
    h.push(9);
    h.push(1);
    println!("{:?} {}", h.peek(), h.len());
    println!("{:?} {:?} {:?}", h.pop(), h.pop(), h.pop());
    println!("{}", h.pop().is_none());

    let mut rh: BinaryHeap<std::cmp::Reverse<i32>> = BinaryHeap::new();
    rh.push(std::cmp::Reverse(3));
    rh.push(std::cmp::Reverse(9));
    println!("{} {}", rh.pop().is_some(), rh.len());
}
