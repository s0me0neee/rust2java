// Ordering Debug and sign-only comparisons: signum only where the value must be -1/0/1.
use std::cmp::Ordering;

fn main() {
    let a = 3.cmp(&7);
    let b = 7.cmp(&3);
    let c = 5.cmp(&5);
    println!("{:?} {:?} {:?}", a, b, c);
    println!("{:?} {:?}", "b".cmp("a"), "a".cmp("b"));
    println!("{} {}", a == Ordering::Equal, c == Ordering::Equal);
    println!("{} {} {}", a < b, c >= a, a != b);
    println!("{:?} {:?}", a.then(b), c.then_with(|| 9.cmp(&2)));
    let m = Ordering::Equal;
    println!("{:?} {}", m == Ordering::Equal, m != Ordering::Greater);
    println!("{:?} {:?}", std::cmp::min(a, b), std::cmp::max(a, c));
    println!("{:?}", 3i32.cmp(&9));
    println!("{:?}", 2.0f64.total_cmp(&3.0));
}
