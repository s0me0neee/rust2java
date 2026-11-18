// entry().and_modify(..) with and without or_insert*, X::from_iter, slice.to_owned().
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};

fn main() {
    let mut counts: BTreeMap<char, u32> = BTreeMap::new();
    for c in "banana".chars() {
        counts.entry(c).and_modify(|n| *n += 1).or_insert(1);
    }
    println!("{:?}", counts);

    let mut scores: HashMap<&str, i64> = HashMap::new();
    scores.insert("ann", 10);
    scores.entry("ann").and_modify(|s| *s *= 3);
    scores.entry("bob").and_modify(|s| *s *= 3);
    scores.entry("cid").and_modify(|s| { *s = 0; }).or_default();
    scores.entry("cid").and_modify(|s| *s -= 4).or_insert_with(|| 99);
    let mut names: Vec<_> = scores.iter().collect();
    names.sort();
    println!("{:?}", names);

    let mut groups: BTreeMap<u8, Vec<u8>> = BTreeMap::new();
    for x in [3u8, 7, 13, 17, 4] {
        groups.entry(x % 10).or_default().push(x);
    }
    println!("{:?}", groups);

    let letters: HashSet<char> = HashSet::from_iter("hello".chars());
    let ordered = BTreeSet::from_iter(letters.iter().copied());
    let squares = BTreeMap::from_iter((1..4).map(|i| (i, i * i)));
    let queue = VecDeque::from_iter([1, 2, 3].into_iter().rev());
    println!("{} {:?} {:?} {:?}", letters.len(), ordered, squares, queue);

    let base = [5, 1, 4];
    let mut copy = base[..].to_owned();
    copy.sort();
    println!("{:?} {:?}", base, copy);
}
