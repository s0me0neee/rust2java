fn main() {
    let mut v: Vec<i32> = vec![5, 3, 8, 1, 9, 2];
    let e: Vec<i32> = Vec::new();
    println!("{:?} {:?} {:?} {:?}", v.iter().max(), e.iter().max(), v.iter().min_by_key(|x| (*x - 4).abs()), e.first());
    println!("{:?} {:?}", v.iter().max_by_key(|x| *x % 3), v.iter().min_by_key(|x| *x % 3));
    println!("{:?} {:?}", v.iter().position(|&x| x == 8), v.iter().rposition(|&x| x < 3));
    v.retain(|&x| x != 8);
    v.dedup();
    v.insert(0, 7);
    let r = v.remove(2);
    println!("{:?} {}", v, r);
    let mut w = vec![1, 1, 2, 2, 2, 3, 1];
    w.dedup();
    println!("{:?}", w);
    let mut d = vec![3, 1, 2];
    d.sort_by(|a, b| b.cmp(a));
    d.reverse();
    println!("{:?} {} {}", d, d.contains(&2), d.iter().any(|&x| x > 5));
    let mut dr = vec![1, 2, 3, 4, 5];
    let drained: Vec<i32> = dr.drain(1..3).collect();
    println!("{:?} {:?}", dr, drained);
    let tail = dr.split_off(1);
    println!("{:?} {:?}", dr, tail);
    let mut c = vec![0; 3];
    c.extend([4, 5].iter());
    c.resize(7, -1);
    c.swap(0, 6);
    println!("{:?} {:?} {:?}", c, c.iter().rev().skip(1).step_by(2).collect::<Vec<_>>(), c.concat_check());
    let words = vec!["b", "a", "c"];
    println!("{} {:?}", words.join("+"), words.iter().map(|w| w.len()).sum::<usize>());
    let mut s = vec!["bb", "a", "ccc", "dd"];
    s.sort_by_key(|w| std::cmp::Reverse(w.len()));
    println!("{:?}", s);
    let nested = vec![vec![1, 2], vec![], vec![3]];
    println!("{:?} {}", nested.concat(), nested.iter().map(Vec::len).max().unwrap());
    println!("{:?} {:?}", v.iter().take(100).count(), v.iter().skip(100).next());
    println!("{:?}", v.iter().zip(v.iter().skip(1)).map(|(a, b)| b - a).collect::<Vec<_>>());
    println!("{:?}", (1..=4).flat_map(|i| (0..i).map(move |j| i * 10 + j)).filter(|x| x % 2 == 1).collect::<Vec<_>>());
    println!("{:?}", v.iter().scan(0, |acc, &x| { *acc += x; Some(*acc) }).collect::<Vec<_>>());
    println!("{:?} {:?}", v.last(), v.get(1..3));
    println!("{:?}", v.iter().copied().reduce(|a, b| a.max(b)));
    let (small, big): (Vec<i32>, Vec<i32>) = v.iter().partition(|&&x| x < 5);
    println!("{:?} {:?} {}", small, big, v.iter().all(|x| *x > 0));
    println!("{:?}", v.chunks_exact(2).map(|c| c[0] * c[1]).collect::<Vec<_>>());
    println!("{:?}", v.iter().rev().enumerate().map(|(i, x)| i as i32 * x).collect::<Vec<_>>());
}
trait C { fn concat_check(&self) -> usize; }
impl C for Vec<i32> { fn concat_check(&self) -> usize { self.len() } }
