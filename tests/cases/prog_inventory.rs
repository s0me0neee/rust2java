// Inventory with a BTreeMap keyed by SKU, category grouping and Display tables
// built with aligned format specs.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Category {
    Tools,
    Food,
    Books,
    Other,
}

impl Category {
    fn label(&self) -> &'static str {
        match self {
            Category::Tools => "tools",
            Category::Food => "food",
            Category::Books => "books",
            Category::Other => "other",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct Item {
    sku: String,
    name: String,
    category: Category,
    quantity: i32,
    price: f64,
}

impl Item {
    fn value(&self) -> f64 {
        self.quantity as f64 * self.price
    }
}

impl fmt::Display for Item {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<8} {:<16} {:<6} {:>5} {:>8.2} {:>9.2}",
            self.sku,
            self.name,
            self.category.label(),
            self.quantity,
            self.price,
            self.value()
        )
    }
}

#[derive(Debug, PartialEq)]
enum StockError {
    Unknown(String),
    OutOfStock(String, i32),
    BadQuantity(i32),
}

impl fmt::Display for StockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StockError::Unknown(sku) => write!(f, "no such sku: {}", sku),
            StockError::OutOfStock(sku, left) => {
                write!(f, "{}: only {} left", sku, left)
            }
            StockError::BadQuantity(n) => write!(f, "quantity must be positive, got {}", n),
        }
    }
}

struct Inventory {
    items: BTreeMap<String, Item>,
    categories: BTreeSet<Category>,
    sales: f64,
}

impl Inventory {
    fn new() -> Inventory {
        Inventory {
            items: BTreeMap::new(),
            categories: BTreeSet::new(),
            sales: 0.0,
        }
    }

    fn add(&mut self, item: Item) {
        self.categories.insert(item.category);
        self.items.insert(item.sku.clone(), item);
    }

    fn restock(&mut self, sku: &str, amount: i32) -> Result<i32, StockError> {
        if amount <= 0 {
            return Err(StockError::BadQuantity(amount));
        }
        let item = self
            .items
            .get_mut(sku)
            .ok_or_else(|| StockError::Unknown(sku.to_string()))?;
        item.quantity += amount;
        Ok(item.quantity)
    }

    fn sell(&mut self, sku: &str, amount: i32) -> Result<f64, StockError> {
        if amount <= 0 {
            return Err(StockError::BadQuantity(amount));
        }
        let price = match self.items.get(sku) {
            Some(item) => item.price,
            None => return Err(StockError::Unknown(sku.to_string())),
        };
        let item = self.items.get_mut(sku).expect("checked above");
        if item.quantity < amount {
            return Err(StockError::OutOfStock(sku.to_string(), item.quantity));
        }
        item.quantity -= amount;
        let total = price * amount as f64;
        self.sales += total;
        Ok(total)
    }

    fn reprice(&mut self, category: Category, factor: f64) {
        for item in self.items.values_mut() {
            if item.category == category {
                item.price = (item.price * factor * 100.0).round() / 100.0;
            }
        }
    }

    fn header() -> String {
        format!(
            "{:<8} {:<16} {:<6} {:>5} {:>8} {:>9}",
            "sku", "name", "cat", "qty", "price", "value"
        )
    }

    fn table(&self) -> Vec<String> {
        let mut rows = vec![Inventory::header()];
        rows.push("-".repeat(58));
        for item in self.items.values() {
            rows.push(format!("{}", item));
        }
        rows
    }

    fn by_category(&self) -> BTreeMap<Category, Vec<&Item>> {
        let mut groups: BTreeMap<Category, Vec<&Item>> = BTreeMap::new();
        for item in self.items.values() {
            groups.entry(item.category).or_default().push(item);
        }
        groups
    }

    fn total_value(&self) -> f64 {
        self.items.values().map(|item| item.value()).sum()
    }

    fn low_stock(&self, threshold: i32) -> Vec<&Item> {
        self.items
            .values()
            .filter(|item| item.quantity < threshold)
            .collect()
    }
}

fn parse_category(text: &str) -> Result<Category, StockError> {
    match text {
        "tools" => Ok(Category::Tools),
        "food" => Ok(Category::Food),
        "books" => Ok(Category::Books),
        "other" => Ok(Category::Other),
        _ => Err(StockError::Unknown(text.to_string())),
    }
}

fn main() {
    let mut inventory = Inventory::new();
    let catalog = [
        ("T-001", "hammer", "tools", 12, 9.5),
        ("T-002", "screwdriver", "tools", 3, 4.25),
        ("B-001", "cookbook", "books", 5, 24.0),
        ("B-002", "atlas", "books", 1, 39.99),
        ("F-001", "coffee", "food", 20, 7.75),
        ("F-002", "tea", "food", 0, 5.5),
    ];
    for (sku, name, category, quantity, price) in catalog {
        let category = parse_category(category).unwrap_or(Category::Other);
        inventory.add(Item {
            sku: sku.to_string(),
            name: name.to_string(),
            category,
            quantity,
            price,
        });
    }
    for line in inventory.table() {
        println!("{}", line);
    }
    println!("total value: {:.2}", inventory.total_value());
    println!("sales so far: {:.2}", inventory.sales);
    let actions = [
        ("T-001", 5),
        ("F-002", 4),
        ("F-002", 4),
        ("Z-999", 1),
        ("B-002", 2),
        ("B-001", 0),
    ];
    for (sku, amount) in actions {
        match inventory.sell(sku, amount) {
            Ok(total) => println!("sold {} x {} for {:.2}", amount, sku, total),
            Err(error) => println!("sale failed: {}", error),
        }
    }
    match inventory.restock("F-002", 10) {
        Ok(left) => println!("restocked F-002 to {}", left),
        Err(error) => println!("restock failed: {}", error),
    }
    inventory.reprice(Category::Books, 1.1);
    for (category, items) in inventory.by_category() {
        let value: f64 = items.iter().map(|item| item.value()).sum();
        println!(
            "{:<6} {:>2} items {:>9.2}",
            category.label(),
            items.len(),
            value
        );
        for item in items {
            println!("    {:<10} {:>4}", item.name, item.quantity);
        }
    }
    let low: Vec<&str> = inventory
        .low_stock(5)
        .iter()
        .map(|item| item.sku.as_str())
        .collect();
    println!("low stock: {:?}", low);
    println!("sales total: {:.2}", inventory.sales);
    println!("{:?}", parse_category("garden").is_err());
    let mut names: Vec<&String> = inventory.items.keys().collect();
    names.sort();
    println!("skus: {}", names.len());
    println!("first sku: {}", names.first().unwrap());
}
