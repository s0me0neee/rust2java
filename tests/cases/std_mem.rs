// mem::{swap, replace, take}, cmp::{min, max}, cells passed to std.
use std::cmp;
use std::mem;

fn main() {
    let mut a = 3;
    let mut b = 8;
    mem::swap(&mut a, &mut b);
    println!("{a} {b}");

    let old = mem::replace(&mut a, 42);
    println!("{old} {a}");

    let mut s = String::from("hi");
    let taken = mem::take(&mut s);
    println!("{taken} {}", s.is_empty());

    let mut opt = Some(5);
    let prev = opt.take();
    println!("{:?} {:?}", prev, opt);

    println!("{} {}", cmp::min(3, 9), cmp::max(3, 9));
    println!("{} {}", cmp::max("apple", "pear"), 2.5f64.min(1.5));
    println!("{} {}", cmp::min('a', 'b'), cmp::max(true, false));

    let n = 7;
    let m = mem::replace(&mut a, n);
    println!("{m} {a} {b}");

    let mut v = vec![1, 2, 3];
    let first = v.remove(0);
    v.insert(0, first + 10);
    println!("{:?}", v);
}
