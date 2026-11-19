// Traits & generics: generic trait impls for several types, PartialOrd/Ord/Hash derives on enums and
// structs, HashSet of structs, Default with struct update, impl blocks with associated fns, sum of custom type.
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ops::{Add, Mul};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Card {
    rank: u8,
    suit: Suit,
}

#[derive(Debug, Default, Clone)]
struct Config {
    name: String,
    retries: u32,
    verbose: bool,
    ratio: f64,
    tags: Vec<String>,
}

trait Area {
    fn area(&self) -> f64;
    fn bigger_than<T: Area>(&self, other: &T) -> bool {
        self.area() > other.area()
    }
}

struct Sq(f64);
struct Rect(f64, f64);

impl Area for Sq {
    fn area(&self) -> f64 {
        self.0 * self.0
    }
}
impl Area for Rect {
    fn area(&self) -> f64 {
        self.0 * self.1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Money {
    cents: i64,
}

impl Add for Money {
    type Output = Money;
    fn add(self, o: Money) -> Money {
        Money { cents: self.cents + o.cents }
    }
}

impl Mul<i64> for Money {
    type Output = Money;
    fn mul(self, k: i64) -> Money {
        Money { cents: self.cents * k }
    }
}

impl std::iter::Sum for Money {
    fn sum<I: Iterator<Item = Money>>(iter: I) -> Money {
        iter.fold(Money::default(), |a, b| a + b)
    }
}

fn main() {
    let mut hand = vec![
        Card { rank: 10, suit: Suit::Hearts },
        Card { rank: 2, suit: Suit::Spades },
        Card { rank: 10, suit: Suit::Clubs },
        Card { rank: 14, suit: Suit::Diamonds },
        Card { rank: 2, suit: Suit::Spades },
    ];
    hand.sort();
    println!("{:?}", hand.iter().map(|c| format!("{}{:?}", c.rank, c.suit)).collect::<Vec<_>>());
    hand.sort_by(|a, b| b.suit.cmp(&a.suit).then(b.rank.cmp(&a.rank)));
    println!("{:?}", hand.iter().map(|c| (c.rank, c.suit as u8)).collect::<Vec<_>>());
    let unique: HashSet<Card> = hand.iter().cloned().collect();
    let suits: BTreeSet<Suit> = hand.iter().map(|c| c.suit).collect();
    println!("{} unique, suits {:?}, max {:?}", unique.len(), suits, hand.iter().max());
    println!("{} {}", Suit::Hearts > Suit::Diamonds, Card { rank: 3, suit: Suit::Clubs } < Card { rank: 3, suit: Suit::Spades });
    let mut by_suit: HashMap<Suit, Vec<u8>> = HashMap::new();
    for c in &hand {
        by_suit.entry(c.suit).or_default().push(c.rank);
    }
    let mut keys: Vec<_> = by_suit.keys().collect();
    keys.sort();
    println!("{:?} {:?}", keys, by_suit[&Suit::Spades]);

    let base = Config { name: "svc".into(), retries: 3, ..Default::default() };
    let custom = Config { verbose: true, tags: vec!["a".into()], ..base.clone() };
    println!("{:?}", base);
    println!("{:?}", custom);
    println!("{:?}", Config::default());

    let (s, r) = (Sq(3.0), Rect(2.0, 5.0));
    println!("{} {} {}", s.bigger_than(&r), r.bigger_than(&s), Sq(2.0).bigger_than(&Sq(1.0)));

    let prices = [Money { cents: 250 }, Money { cents: 199 }, Money { cents: 1 }];
    let total: Money = prices.iter().copied().sum();
    let doubled: Money = prices.iter().map(|&m| m * 2).sum();
    println!("{:?} {:?} {}", total, doubled, total == Money { cents: 450 });
}
