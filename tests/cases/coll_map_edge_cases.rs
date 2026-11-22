use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
fn main() {
    let mut m: BTreeMap<String, Vec<i32>> = BTreeMap::new();
    for (k, v) in [("b", 2), ("a", 1), ("b", 3), ("c", 0)] { m.entry(k.to_string()).or_default().push(v); }
    println!("{:?}", m);
    println!("{:?} {:?}", m.get("b"), m.get("z"));
    println!("{:?} {:?}", m.keys().collect::<Vec<_>>(), m.values().map(|v| v.len()).sum::<usize>());
    println!("{:?} {:?}", m.first_key_value(), m.last_key_value().map(|(k, _)| k));
    if let Some(v) = m.get_mut("a") { v.push(10); }
    let removed = m.remove("c");
    println!("{:?} {:?} {}", removed, m.contains_key("c"), m.len());
    let mut counts: HashMap<char, usize> = HashMap::new();
    for c in "mississippi".chars() { *counts.entry(c).or_insert(0) += 1; }
    let mut cv: Vec<_> = counts.into_iter().collect();
    cv.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("{:?}", cv);
    let a: HashSet<i32> = [1, 2, 3, 4].into_iter().collect();
    let b: HashSet<i32> = [3, 4, 5].into_iter().collect();
    let mut i: Vec<_> = a.intersection(&b).copied().collect(); i.sort();
    let mut u: Vec<_> = a.union(&b).copied().collect(); u.sort();
    let mut d: Vec<_> = a.difference(&b).copied().collect(); d.sort();
    let mut s: Vec<_> = a.symmetric_difference(&b).copied().collect(); s.sort();
    println!("{:?} {:?} {:?} {:?} {} {}", i, u, d, s, a.is_subset(&b), a.is_disjoint(&b));
    let mut bs: BTreeSet<i32> = (1..10).collect();
    bs.retain(|x| x % 3 != 0);
    let pl = bs.pop_last();
    println!("{:?} {:?} {:?} {:?}", bs, bs.first(), pl, bs.iter().rev().take(2).collect::<Vec<_>>());
    let mut ins = HashSet::new();
    println!("{} {} {}", ins.insert(5), ins.insert(5), ins.remove(&5));
    let mut om = BTreeMap::new();
    println!("{:?} {:?}", om.insert(1, "a"), om.insert(1, "b"));
    let inv: BTreeMap<&str, i32> = om.iter().map(|(k, v)| (*v, *k)).collect();
    println!("{:?} {:?}", inv, om.iter().next());
    let mut hm: HashMap<(i32, i32), String> = HashMap::new();
    hm.insert((1, 2), "x".into());
    println!("{:?} {:?}", hm.get(&(1, 2)), hm.get(&(2, 1)));
    let total: i32 = om.keys().sum();
    let mut e = BTreeMap::new();
    e.entry("k").or_insert_with(Vec::new).push(1);
    *e.entry("k").or_insert_with(Vec::new) = vec![9];
    let zl = e.entry("z").or_insert(vec![0]).len();
    println!("{total} {:?} {:?}", e, zl);
    let mut bm: BTreeMap<i32, i32> = (0..6).map(|i| (i, i * i)).collect();
    bm.retain(|k, v| k % 2 == 0 && *v < 20);
    let pf = bm.pop_first();
    println!("{:?} {:?}", bm, pf);
}
