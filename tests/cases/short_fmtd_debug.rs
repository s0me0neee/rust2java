use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Pair {
    a: i32,
    b: bool,
}

fn main() {
    println!("{:?}", vec![1, 2, 3]);
    println!("{:?}", vec![true, false]);
    println!("{:?}", vec![vec![1, 2], vec![]]);
    println!("{:?}", vec![vec![true], vec![false]]);
    let mut set = BTreeSet::new();
    set.insert(3);
    set.insert(1);
    set.insert(2);
    println!("{:?}", set);
    let mut map = BTreeMap::new();
    map.insert(3, 4);
    map.insert(1, 2);
    println!("{:?}", map);
    println!("{:?}", vec![(1, 'x'), (2, 'y')]);
    println!("{:?}", vec![Some(1), None]);
    let some: Option<i32> = Some(7);
    let none: Option<i32> = None;
    println!("{:?} {:?}", some, none);
    println!("{:?}", Some((1, "two")));
    println!("{:?}", Some(vec![1, 2]));
    println!("{:?}", Some(Some(1)));
    println!("{:?}", Some(1.5));
    let ok: Result<i32, &str> = Ok(1);
    let err: Result<i32, &str> = Err("bad");
    println!("{:?} {:?}", ok, err);
    let pair = Pair { a: 1, b: true };
    println!("{:?}", vec![Pair { a: 2, b: false }]);
    println!("{:?}", pair);
    println!("[{:6?}] [{:04?}] [{:.1?}]", Some(1), vec![1, 2], Some(1.234));
    println!("[{:6?}] {:?}", (1, 2), (3, "x"));
}
