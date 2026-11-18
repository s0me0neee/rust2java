// `impl Iterator` for a user type, and `Deref`/`DerefMut` newtypes forwarding to their target.
use std::ops::Deref;

struct Counter {
    n: i32,
    max: i32,
}
impl Counter {
    fn new(max: i32) -> Counter {
        Counter { n: 0, max }
    }
}
impl Iterator for Counter {
    type Item = i32;
    fn next(&mut self) -> Option<i32> {
        if self.n < self.max {
            self.n += 1;
            Some(self.n)
        } else {
            None
        }
    }
}

use std::ops::DerefMut;

struct Wrapper(Vec<i32>);
impl Deref for Wrapper {
    type Target = Vec<i32>;
    fn deref(&self) -> &Vec<i32> {
        &self.0
    }
}
impl DerefMut for Wrapper {
    fn deref_mut(&mut self) -> &mut Vec<i32> {
        &mut self.0
    }
}

fn main() {
    let mut sum = 0;
    for x in Counter::new(5) {
        sum += x;
    }
    println!("{}", sum);
    let v: Vec<i32> = Counter::new(5).take(3).collect();
    println!("{:?}", v);
    println!("{}", Counter::new(4).sum::<i32>());
    let doubled: Vec<i32> = Counter::new(3).map(|x| x * 2).collect();
    println!("{:?}", doubled);
    println!("{}", Counter::new(9).filter(|x| x % 2 == 0).count());

    let mut w = Wrapper(vec![1, 2, 3]);
    println!("{}", w.len());
    println!("{}", w[1]);
    w[0] = 7;
    println!("{:?}", w.0);
    let mut seen = String::new();
    for x in w.iter() {
        seen.push_str(&x.to_string());
    }
    println!("{}", seen);
    w.push(9);
    println!("{:?}", w.deref());
}
