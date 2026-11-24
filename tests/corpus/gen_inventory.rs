use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum Category {
    Tool,
    Food,
    Toy,
}

#[derive(Debug, Clone)]
struct Item {
    sku: String,
    cat: Category,
    qty: u32,
    price_cents: u64,
}

fn main() {
    let data = "T-100,Tool,5,1299\nF-200,Food,40,199\nT-101,Tool,0,4599\nY-300,Toy,12,999\nF-201,Food,3,549\nY-301,Toy,7,2499";
    let items: Vec<Item> = data
        .lines()
        .map(|l| {
            let f: Vec<&str> = l.split(',').collect();
            let cat = match f[1] {
                "Tool" => Category::Tool,
                "Food" => Category::Food,
                _ => Category::Toy,
            };
            Item { sku: f[0].to_string(), cat, qty: f[2].parse().unwrap(), price_cents: f[3].parse().unwrap() }
        })
        .collect();
    let mut by_cat: BTreeMap<Category, Vec<&Item>> = BTreeMap::new();
    for it in &items {
        by_cat.entry(it.cat).or_default().push(it);
    }
    for (cat, its) in &by_cat {
        let value: u64 = its.iter().map(|i| i.qty as u64 * i.price_cents).sum();
        println!("{:?}: {} skus, value ${}.{:02}", cat, its.len(), value / 100, value % 100);
    }
    let out_of_stock: HashSet<&str> = items.iter().filter(|i| i.qty == 0).map(|i| i.sku.as_str()).collect();
    println!("out of stock: {:?}", out_of_stock);
    let index: HashMap<&str, &Item> = items.iter().map(|i| (i.sku.as_str(), i)).collect();
    for sku in ["Y-301", "Z-999"] {
        match index.get(sku) {
            Some(item) => println!("{sku}: {} @ {}", item.qty, item.price_cents),
            None => println!("{sku}: unknown"),
        }
    }
    let cheapest = items.iter().min_by_key(|i| i.price_cents).map(|i| &i.sku);
    let priciest = items.iter().max_by(|a, b| a.price_cents.cmp(&b.price_cents)).unwrap();
    println!("{:?} {}", cheapest, priciest.sku);
    let mut restock: Vec<Item> = items.iter().filter(|i| i.qty < 5).cloned().collect();
    restock.iter_mut().for_each(|i| i.qty += 10);
    restock.retain(|i| i.cat != Category::Food);
    println!("{:?}", restock.iter().map(|i| (&i.sku, i.qty)).collect::<Vec<_>>());
}
