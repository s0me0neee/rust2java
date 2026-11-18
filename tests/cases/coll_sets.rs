use std::collections::{BTreeSet, HashSet};

fn main() {
    let mut a = BTreeSet::new();
    a.extend([1, 2, 3]);
    let b = BTreeSet::from([2, 3, 4]);
    println!("{:?} {:?}", a.first(), a.last());
    println!("{:?}", a.pop_first());
    println!("{:?}", a.difference(&b).copied().collect::<Vec<_>>());
    println!("{:?}", a.symmetric_difference(&b).copied().collect::<Vec<_>>());
    let x = BTreeSet::from([1, 3]);
    let y = BTreeSet::from([2, 4]);
    println!("{:?}", x.symmetric_difference(&y).copied().collect::<Vec<_>>());
    println!("{:?}", x.union(&y).copied().collect::<Vec<_>>());
    println!("{}", a.is_subset(&b));
    a.retain(|x| *x != 2);
    println!("{:?}", a);
    let h = HashSet::from([2, 3, 3]);
    println!("{} {}", h == HashSet::from([3, 2]), h.len());
}
