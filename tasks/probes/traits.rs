use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, AddAssign, Index, Mul, Neg, Sub};
#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct V2 {
    x: f64,
    y: f64,
}
impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 { V2 { x: self.x + o.x, y: self.y + o.y } }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 { V2 { x: self.x - o.x, y: self.y - o.y } }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 { V2 { x: self.x * k, y: self.y * k } }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 { V2 { x: -self.x, y: -self.y } }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) { self.x += o.x; self.y += o.y; }
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Card {
    rank: u8,
    suit: char,
}
struct Grid {
    w: usize,
    cells: Vec<char>,
}
impl Index<(usize, usize)> for Grid {
    type Output = char;
    fn index(&self, (r, c): (usize, usize)) -> &char { &self.cells[r * self.w + c] }
}
#[derive(Debug, Default)]
struct Config {
    name: String,
    level: u32,
    verbose: bool,
}
struct Celsius(f64);
struct Fahrenheit(f64);
impl From<Celsius> for Fahrenheit {
    fn from(c: Celsius) -> Self { Fahrenheit(c.0 * 9.0 / 5.0 + 32.0) }
}
impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { write!(f, "{}{}", self.rank, self.suit) }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Level { Low, Mid, High }
fn main() {
    let mut a = V2 { x: 1.0, y: 2.0 };
    let b = V2 { x: 0.5, y: 0.5 };
    println!("{:?}", a + b);
    println!("{:?}", (a - b) * 2.0);
    println!("{:?}", -a);
    a += b;
    println!("{:?} {}", a, a == V2 { x: 1.5, y: 2.5 });
    println!("{:?}", V2::default());
    let mut cards = vec![Card { rank: 3, suit: 'h' }, Card { rank: 1, suit: 's' }, Card { rank: 3, suit: 'c' }];
    cards.sort();
    println!("{:?}", cards);
    println!("{}", cards[0]);
    println!("{:?}", cards.iter().max());
    println!("{}", cards[0] < cards[1]);
    let mut by_card: HashMap<Card, u32> = HashMap::new();
    *by_card.entry(cards[0].clone()).or_insert(0) += 1;
    println!("{:?}", by_card.get(&Card { rank: 1, suit: 's' }));
    let g = Grid { w: 2, cells: vec!['a', 'b', 'c', 'd'] };
    println!("{}", g[(1, 0)]);
    let cfg = Config { name: "x".into(), ..Default::default() };
    println!("{:?}", cfg);
    let f: Fahrenheit = Celsius(100.0).into();
    println!("{}", f.0);
    let f2 = Fahrenheit::from(Celsius(0.0));
    println!("{}", f2.0);
    let mut lv = vec![Level::High, Level::Low, Level::Mid];
    lv.sort();
    println!("{:?} {}", lv, Level::Mid > Level::Low);
    println!("{:?}", Level::High.cmp(&Level::Low));
    println!("{}", Level::Mid as i32);
    let s: String = cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
    println!("{}", s);
    let n: i64 = 5i32.into();
    println!("{}", n);
    let words = vec!["b", "a"];
    let owned: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    println!("{:?}", owned);
    println!("{}", std::mem::size_of::<u8>() > 0);
}
