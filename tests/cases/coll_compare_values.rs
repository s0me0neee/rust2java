// <, cmp, partial_cmp, max/min on lists, tuples and options (Java types without compareTo).
use std::cmp::Ordering;
fn main() {
    let a = vec![1, 2, 3];
    let b = vec![1, 3];
    println!("{} {} {} {:?}", a < b, a >= b, a == b, a.cmp(&b));
    let p = (2, "x");
    let q = (2, "y");
    println!("{} {:?} {:?} {:?}", p < q, p.cmp(&q), p.max(q), std::cmp::min(p, q));
    let o: Option<i32> = None;
    println!("{} {:?}", o < Some(0), Some(3).cmp(&Some(2)));
    let s = "abc".to_string();
    println!("{} {:?}", s < "abd".to_string(), s.as_str().cmp("ab"));
    let best = [vec![2, 1], vec![2], vec![1, 9]].into_iter().max().unwrap();
    println!("{:?} {}", best, a.cmp(&b) == Ordering::Less);
    let mut pairs = vec![(1, 'b'), (1, 'a')];
    pairs.sort_by(|x, y| x.cmp(y));
    println!("{:?} {:?}", pairs, (1u32, 2u8).partial_cmp(&(1, 3)));
}
