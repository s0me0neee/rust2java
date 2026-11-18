// Manual PartialEq/Eq/Hash/Ord/PartialOrd/Debug/Clone/Default impls on a struct.
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

struct Card {
    rank: u8,
    suit: char,
}
impl PartialEq for Card {
    fn eq(&self, other: &Self) -> bool {
        self.rank == other.rank && self.suit == other.suit
    }
}
impl Eq for Card {}
impl Hash for Card {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.rank.hash(state);
        self.suit.hash(state);
    }
}
impl Ord for Card {
    fn cmp(&self, other: &Self) -> Ordering {
        self.rank.cmp(&other.rank).then(self.suit.cmp(&other.suit))
    }
}
impl PartialOrd for Card {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl fmt::Debug for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Card({}, {})", self.rank, self.suit)
    }
}
impl Clone for Card {
    fn clone(&self) -> Self {
        Card { rank: self.rank, suit: self.suit }
    }
}
impl Default for Card {
    fn default() -> Self {
        Card { rank: 2, suit: 's' }
    }
}

struct Celsius(i32);
struct Fahrenheit(i32);
impl PartialEq<Fahrenheit> for Celsius {
    fn eq(&self, other: &Fahrenheit) -> bool {
        self.0 * 9 / 5 + 32 == other.0
    }
}

fn main() {
    let a = Card { rank: 3, suit: 'h' };
    let b = Card { rank: 1, suit: 's' };
    println!("{} {} {}", a == a.clone(), a == b, a != b);
    println!("{} {} {}", a < b, a > b, a.cmp(&b) == Ordering::Greater);
    println!("{:?}", Card::default());
    let mut cards = vec![a.clone(), b.clone(), Card { rank: 3, suit: 'c' }];
    cards.sort();
    println!("{:?}", cards);
    println!("{:?}", cards.iter().max());
    let mut counts: HashMap<Card, u32> = HashMap::new();
    *counts.entry(a.clone()).or_insert(0) += 1;
    *counts.entry(a.clone()).or_insert(0) += 1;
    println!("{}", counts[&a]);
    let mut def = Card::default();
    def.rank = 9;
    println!("{:?}", def);

    let c = Celsius(100);
    let f = Fahrenheit(212);
    println!("{} {}", c == f, c != f);
}
