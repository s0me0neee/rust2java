use std::collections::{BTreeMap, HashMap};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Category { Tool, Food, Toy }

#[derive(Debug, Clone)]
struct Item { name: String, cat: Category, qty: u32, price_cents: u64 }

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:<10} {:>4} @ ${}.{:02}", self.name, self.qty, self.price_cents / 100, self.price_cents % 100)
    }
}

#[derive(Debug)]
enum InvError { NotFound(String), Insufficient { name: String, have: u32, want: u32 } }

impl fmt::Display for InvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InvError::NotFound(n) => write!(f, "no item named {n}"),
            InvError::Insufficient { name, have, want } => write!(f, "{name}: have {have}, want {want}"),
        }
    }
}

struct Inventory { items: HashMap<String, Item> }

impl Inventory {
    fn new() -> Self { Inventory { items: HashMap::new() } }
    fn add(&mut self, name: &str, cat: Category, qty: u32, price_cents: u64) {
        self.items.entry(name.to_string())
            .and_modify(|i| i.qty += qty)
            .or_insert(Item { name: name.to_string(), cat, qty, price_cents });
    }
    fn take(&mut self, name: &str, want: u32) -> Result<u64, InvError> {
        let item = self.items.get_mut(name).ok_or_else(|| InvError::NotFound(name.to_string()))?;
        if item.qty < want {
            return Err(InvError::Insufficient { name: name.to_string(), have: item.qty, want });
        }
        item.qty -= want;
        Ok(item.price_cents * want as u64)
    }
    fn value_by_cat(&self) -> BTreeMap<String, u64> {
        let mut m = BTreeMap::new();
        for it in self.items.values() {
            *m.entry(format!("{:?}", it.cat)).or_insert(0) += it.price_cents * it.qty as u64;
        }
        m
    }
}

fn main() {
    let mut inv = Inventory::new();
    inv.add("hammer", Category::Tool, 3, 1299);
    inv.add("apple", Category::Food, 50, 45);
    inv.add("yoyo", Category::Toy, 7, 350);
    inv.add("apple", Category::Food, 10, 45);
    for (name, want) in [("apple", 20), ("hammer", 5), ("saw", 1), ("yoyo", 7)] {
        match inv.take(name, want) {
            Ok(cost) => println!("took {want} {name} for {cost}"),
            Err(e) => println!("error: {e}"),
        }
    }
    let mut names: Vec<&Item> = inv.items.values().collect();
    names.sort_by(|a, b| b.qty.cmp(&a.qty).then_with(|| a.name.cmp(&b.name)));
    for it in &names { println!("{it}"); }
    for (cat, v) in inv.value_by_cat() { println!("{cat:>6}: {v}"); }
    let empty: Vec<_> = names.iter().filter(|i| i.qty == 0).map(|i| i.name.as_str()).collect();
    println!("{:?} {}", empty, names.iter().any(|i| i.cat == Category::Tool));
}
