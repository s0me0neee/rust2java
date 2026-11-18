use std::collections::BTreeMap;

fn main() {
    let mut map = BTreeMap::new();
    map.insert(1, 10);
    map.insert(2, 20);
    map.insert(3, 30);
    println!("{:?} {:?}", map.first_key_value(), map.last_key_value());
    println!("{:?}", map.range(1..3).map(|(k, v)| (*k, *v)).collect::<Vec<_>>());
    println!("{:?} {:?}", map.keys().last(), map.iter().next_back());
    map.retain(|k, v| {
        *v += 1;
        *k != 2 && *v > 5
    });
    println!("{:?} {:?}", map.pop_first(), map.pop_last());
    println!("{:?}", map.pop_first());
}
