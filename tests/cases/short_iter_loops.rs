use std::collections::BTreeMap;

fn main() {
    let names = vec!["ann", "bob", "cid"];
    let ages = vec![30, 40, 50, 60];
    // zip over the shorter list.
    for (name, age) in names.iter().zip(ages.iter()) {
        println!("{name}={age}");
    }
    for (i, name) in names.iter().enumerate() {
        println!("{i}: {name}");
    }
    // The list element wins over the index when both patterns bind.
    for (i, name) in names.iter().enumerate().rev() {
        println!("{i}.{name}");
    }
    let mut counts = BTreeMap::new();
    counts.insert("a", 1);
    counts.insert("b", 2);
    for (k, v) in counts.iter() {
        println!("{k} => {v}");
    }
    for v in counts.values() {
        println!("value {v}");
    }
    let text = "a,b,,c";
    for part in text.split(',') {
        println!("<{part}>");
    }
    for ch in "xyz".chars().rev() {
        println!("{ch}");
    }
    let sorted = vec![1, 2, 3, 4];
    for value in sorted.iter().rev() {
        println!("{value}");
    }
}
