use std::collections::HashMap;
use std::fmt::{self, Display};

trait Shape { fn area(&self) -> f64; fn name(&self) -> String { "shape".to_string() } }
struct Circle(f64);
struct Rect { w: f64, h: f64 }
impl Shape for Circle { fn area(&self) -> f64 { 3.14159 * self.0 * self.0 } fn name(&self) -> String { "circle".into() } }
impl Shape for Rect { fn area(&self) -> f64 { self.w * self.h } }

fn largest<'a>(shapes: &'a [Box<dyn Shape>]) -> Option<&'a Box<dyn Shape>> {
    shapes.iter().max_by(|a, b| a.area().partial_cmp(&b.area()).unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Suit { Hearts, Spades }

#[derive(Debug, Clone, PartialEq)]
struct Stack<T> { items: Vec<T> }
impl<T: Clone + Display> Stack<T> {
    fn new() -> Self { Stack { items: Vec::new() } }
    fn push(&mut self, t: T) -> &mut Self { self.items.push(t); self }
    fn peek(&self) -> Option<&T> { self.items.last() }
    fn render(&self) -> String { self.items.iter().map(|t| t.to_string()).collect::<Vec<_>>().join("|") }
}
impl<T: Display> Display for Stack<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[")?;
        for (i, it) in self.items.iter().enumerate() { if i > 0 { write!(f, ", ")?; } write!(f, "{}", it)?; }
        write!(f, "]")
    }
}

fn count_by<T, K: std::hash::Hash + Eq, F: Fn(&T) -> K>(xs: &[T], f: F) -> HashMap<K, usize> {
    let mut m = HashMap::new();
    for x in xs { *m.entry(f(x)).or_insert(0) += 1; }
    m
}

fn apply_n<F>(n: usize, mut f: F) where F: FnMut(usize) { for i in 0..n { f(i) } }

fn main() {
    let shapes: Vec<Box<dyn Shape>> = vec![Box::new(Circle(1.5)), Box::new(Rect { w: 2.0, h: 4.0 }), Box::new(Circle(0.5))];
    let total: f64 = shapes.iter().map(|s| s.area()).sum();
    println!("{:.3} {}", total, largest(&shapes).map(|s| s.name()).unwrap_or_default());
    for s in &shapes { println!("{:>8}: {:7.2}", s.name(), s.area()); }
    let mut st: Stack<i32> = Stack::new();
    st.push(1).push(2).push(3);
    println!("{} {} {:?} {}", st, st.render(), st.peek(), st == st.clone());
    let words = ["apple", "avocado", "banana", "blueberry", "cherry"];
    let mut counts: Vec<(char, usize)> = count_by(&words, |w| w.chars().next().unwrap()).into_iter().collect();
    counts.sort();
    println!("{:?}", counts);
    let mut hands = vec![(Suit::Spades, 3), (Suit::Hearts, 10), (Suit::Spades, 1)];
    hands.sort();
    println!("{:?} {:?}", hands, Suit::Hearts < Suit::Spades);
    let mut acc = Vec::new();
    apply_n(4, |i| acc.push(i * i));
    println!("{:?}", acc);
}
