// Records for immutable structs, map lookups and heap pops as nullable locals, Debug as toString.
use std::collections::{BinaryHeap, HashMap};

#[derive(Debug, Clone, PartialEq)]
struct Item {
    name: String,
    price: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Size {
    Small,
    Large,
}

fn size_of(price: u32) -> Size {
    match price {
        0..=9 => Size::Small,
        _ => Size::Large,
    }
}

fn label(size: Size) -> &'static str {
    match size {
        Size::Small => "small",
        Size::Large => "large",
    }
}

fn main() {
    let mut stock: HashMap<String, Item> = HashMap::new();
    for (name, price) in [("pen", 3), ("lamp", 40)] {
        stock.insert(name.to_string(), Item { name: name.to_string(), price });
    }

    for key in ["pen", "desk"] {
        match stock.get(key) {
            Some(item) => println!("{} costs {} ({})", item.name, item.price, label(size_of(item.price))),
            None => println!("no {}", key),
        }
    }
    if let Some(item) = stock.get("lamp") {
        println!("{:?}", item);
    }

    let mut heap = BinaryHeap::new();
    for p in [5, 1, 9] {
        heap.push(p);
    }
    while let Some(top) = heap.pop() {
        println!("{} {:?}", top, size_of(top));
    }
}
