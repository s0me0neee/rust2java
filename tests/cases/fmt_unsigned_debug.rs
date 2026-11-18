// `{:?}` of unsigned values above the signed range, alone, in containers and under width/fill flags.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug)]
struct Ids {
    ids: Vec<u32>,
    big: Option<u64>,
    pair: (u32, u16),
}

fn main() {
    let v: Vec<u32> = vec![4000000000, 1];
    let w: Vec<u64> = vec![u64::MAX, 2];
    let h: Vec<u16> = vec![65535, 3];
    let b: Vec<u8> = vec![255, 4];
    let nested = vec![vec![3000000000u32]];
    let o = Some(4000000000u32);
    let t = (4000000000u32, 70000u32);
    let d: VecDeque<u32> = VecDeque::from(vec![4000000000]);
    let s: BTreeSet<u32> = [4000000000].into_iter().collect();
    let m: BTreeMap<u32, u64> = BTreeMap::from([(4000000000, u64::MAX)]);
    println!("{:?} {:?} {:?} {:?} {:?}", v, w, h, b, nested);
    println!("{:?} {:?} {:?} {:?} {:?}", o, t, d, s, m);
    let ids = Ids { ids: v.clone(), big: Some(u64::MAX), pair: (4000000000, 65535) };
    println!("{:?}", ids);
    println!("{:?}", v.iter().map(|x| x / 2).collect::<Vec<_>>());

    let x = 4000000000u32;
    println!("[{:12?}] [{:<12?}] [{:012?}] [{:+?}] [{:^12?}]", x, x, x, x, x);
    println!("{:12?} {:<12?} {:012?} {:+?}", v, v, v, v);
    println!("{:22?} {:7?} {:07?} {:?}", w, h, h, Some(x));
    println!("{:5?} {:>3?}", nested, (x, 1u16));
}
