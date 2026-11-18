use std::cmp::Ordering;
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level { Low, Mid, High }
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Card { rank: u8, suit: char }
fn main() {
    let a = Card { rank: 3, suit: 'h' };
    let b = Card { rank: 3, suit: 'c' };
    println!("{:?} {:?}", a.cmp(&b), a.partial_cmp(&b));
    println!("{:?}", Level::High.cmp(&Level::Low));
    println!("{}", a.cmp(&b) == Ordering::Greater);
    println!("{} {}", a.eq(&b), a.ne(&b));
    println!("{:?} {:?}", a.clone().max(b.clone()), Level::Mid.min(Level::Low));
    println!("{:?} {:?}", Level::High.clamp(Level::Low, Level::Mid), Level::Low.max(Level::Low));
    let mut v = vec![a.clone(), b.clone()];
    v.sort_by(|x, y| y.cmp(x));
    println!("{:?}", v);
}
