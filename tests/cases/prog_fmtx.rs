// Formatting sweep: widths/precision from args, named & positional args, sign, alternate forms,
// pretty Debug of nested data, padding of user Display.
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
struct Item {
    name: String,
    qty: u32,
    tags: Vec<&'static str>,
    price: Option<f64>,
}

#[derive(Debug)]
enum Node {
    Leaf(i32),
    Pair(Box<Node>, Box<Node>),
}

struct Money(i64);

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let sign = if self.0 < 0 { "-" } else { "" };
        write!(f, "{}${}.{:02}", sign, self.0.abs() / 100, self.0.abs() % 100)
    }
}

fn main() {
    let w = 10;
    let p = 3;
    let x = 3.14159265;
    println!("[{:>w$}] [{:<w$.p$}] [{:^width$.prec$}]", "right", x, x, width = 12, prec = 1);
    println!("[{0}-{1}-{0}] [{a}{b}{a}]", "x", "y", a = 1, b = 2);
    println!("[{:+}] [{:+.2}] [{:+05}] [{:05}] [{:<5}|]", 42, 2.5, 7, -7, -3);
    println!("[{:#x}] [{:#X}] [{:#b}] [{:#o}] [{:08b}] [{:#010x}]", 255, 255, 5, 8, 19, 3054);
    let item = Item { name: "bolt".into(), qty: 12, tags: vec!["m4", "steel"], price: Some(0.25) };
    println!("{:?}", item);
    println!("{:#?}", item);
    let tree = Node::Pair(Box::new(Node::Leaf(1)), Box::new(Node::Pair(Box::new(Node::Leaf(2)), Box::new(Node::Leaf(3)))));
    println!("{:?}", tree);
    let mut m = BTreeMap::new();
    m.insert("a", vec![1, 2]);
    m.insert("b", vec![]);
    println!("{:?}", m);
    println!("{:#?}", m);
    println!("{:?} {:?} {:?}", (1, "two", 3.0), Some(("x", 'y')), [Some(1), None]);
    for cents in [123456, -5, 0, 99] {
        println!("{:>12}|{}", Money(cents).to_string(), Money(cents));
    }
    let names = ["ann", "bartholomew", "cy"];
    let longest = names.iter().map(|n| n.len()).max().unwrap();
    for (i, n) in names.iter().enumerate() {
        println!("{:>3}. {:<width$} | {:>5.1}%", i + 1, n, 100.0 * (i as f64 + 1.0) / 3.0, width = longest);
    }
    println!("{:?} {:?} {:?}", 1.0f64, 0.1 + 0.2, 1e21);
    println!("{} {} {}", f64::NAN, -f64::INFINITY, -0.0f64);
    println!("{:5}|{:<5}|{:^5}|{:>5}", true, 'c', "ab", 1u8);
    eprintln!("this goes to stderr");
    let s = format!("{:-^20}", " mid ");
    print!("{}", s);
    println!();
}
