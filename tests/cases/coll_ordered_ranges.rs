use std::collections::{BTreeMap, BTreeSet, VecDeque};

fn main() {
    let set: BTreeSet<u64> = (0..50).map(|i| i * 7 % 101).collect();
    let mut acc = 0u64;
    for q in 0..40 {
        if let Some(x) = set.range(q * 3..).next() { acc += x; }
        if let Some(y) = set.range(..q * 3).next_back() { acc ^= y; }
    }
    let mid: Vec<_> = set.range(10..=20).collect();
    println!("{acc} {mid:?} {:?}", set.range(95..).next_back());
    let mut m = BTreeMap::new();
    for i in 0..300u64 { m.insert(i % 50, i); }
    let c: u64 = m.range(10..20).map(|(_, v)| *v).sum();
    println!("{c} {:?} {:?}", m.range(..5).next_back(), m.range(48..).next());
    let mut dq: VecDeque<i32> = VecDeque::new();
    for x in [5, 1, 4, 2, 3] {
        while dq.back().is_some_and(|&b| b < x) { dq.pop_back(); }
        dq.push_back(x);
    }
    println!("{} {} {:?}", dq[0], dq[dq.len() - 1], dq.get(0));
}
