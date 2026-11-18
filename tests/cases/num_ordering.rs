// Ordering: Debug names, then/then_with, total_cmp, partial_cmp and normalised equality.
use std::cmp::Ordering;

fn main() {
    let a = 3.cmp(&7);
    let b = 7.cmp(&3);
    let c = 5.cmp(&5);
    println!("{:?} {:?} {:?}", a, b, c);
    println!("{} {} {}", a.is_lt(), c.is_eq(), b.is_gt());
    println!("{:?} {:?}", a.reverse(), a.then(b));
    println!("{:?}", c.then_with(|| 9.cmp(&2)));
    println!("{}", a == Ordering::Less);
    println!("{} {}", a < b, c >= a);
    println!("{:?} {:?}", 10.0f64.partial_cmp(&3.0), 3.0f64.partial_cmp(&10.0));
    println!("{:?} {:?}", f64::NAN.partial_cmp(&1.0), 1.0f64.partial_cmp(&f64::NAN));
    println!("{:?}", 2.0f64.total_cmp(&3.0));
    println!("{:?}", 5i32.cmp(&3).then(Ordering::Greater));
    let m = Ordering::Equal;
    println!("{:?} {}", m == Ordering::Equal, m != Ordering::Greater);
    let tie = (2.5f64).partial_cmp(&2.5);
    println!("{:?}", tie);
    println!("{:?}", std::cmp::min(a, b));
    println!("{:?}", std::cmp::max(a, c));
    println!("{:?}", 3i32.cmp(&9));
}
