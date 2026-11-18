// `let c = m.entry(k).or_insert(..)` / `get_mut(..).unwrap()` bindings to a number, String or
// Option in a map, and the `std::iter` sources.
use std::collections::{BTreeMap, HashMap};

fn main() {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for w in "the cat and the hat and the bat".split_whitespace() {
        let c = counts.entry(w.to_string()).or_insert(0);
        *c += 1;
    }
    println!("{:?}", counts);

    let mut names: HashMap<u8, String> = HashMap::new();
    for (k, part) in [(1, "a"), (2, "b"), (1, "c")] {
        let s = names.entry(k).or_default();
        s.push_str(part);
        s.push('!');
    }
    let mut keys: Vec<_> = names.keys().copied().collect();
    keys.sort();
    for k in keys {
        println!("{} => {}", k, names[&k]);
    }

    let mut best: HashMap<&str, Option<i32>> = HashMap::new();
    best.insert("x", None);
    let slot = best.get_mut("x").unwrap();
    if slot.is_none() {
        *slot = Some(4);
    }
    let lazy = best.entry("y").or_insert_with(|| Some(9));
    *lazy = lazy.map(|v| v + 1);
    println!("{:?} {:?}", best["x"], best["y"]);

    let threes: Vec<u64> = std::iter::successors(Some(1u64), |&x| if x < 500 { Some(x * 3) } else { None }).collect();
    let pad: String = std::iter::repeat('-').take(5).collect();
    let mut n = 0;
    let squares: Vec<i32> = std::iter::from_fn(|| {
        n += 1;
        if n <= 4 { Some(n * n) } else { None }
    })
    .collect();
    let one: Vec<&str> = std::iter::once("solo").chain(std::iter::empty()).collect();
    println!("{:?} {} {:?} {:?}", threes, pad, squares, one);
}
