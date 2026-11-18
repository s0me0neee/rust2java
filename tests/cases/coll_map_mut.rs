use std::collections::{BTreeMap, HashMap};

fn key() -> i32 {
    print!("k");
    1
}

fn default_value() -> i32 {
    print!("d");
    9
}

fn delta() -> i32 {
    print!("r");
    1
}

fn main() {
    let mut map = BTreeMap::new();
    map.extend([(1, 10), (2, 20), (1, 30)]);
    if let Some(value) = map.get_mut(&1) {
        *value += 2;
    }
    if let Some(value) = map.get_mut(&8) {
        *value = 0;
    }
    for value in map.values_mut() {
        *value += 1;
    }
    for (key, value) in map.iter_mut() {
        *value += *key;
    }
    println!("{:?}", map);
    let mut grid: HashMap<(i32, i32), i32> = HashMap::new();
    grid.extend(vec![((0, 1), 4), ((1, 2), 5)].into_iter());
    println!("{:?} {:?}", grid.get(&(0, 1)), grid.get(&(9, 9)));
    let mut lists = BTreeMap::new();
    lists.insert(1, vec![1]);
    lists.retain(|_, value| {
        *value = vec![3, 4];
        true
    });
    println!("{:?}", lists);
    *map.entry(3).or_insert_with(|| 9) -= 1;
    *map.entry(3).or_insert(100) += 2;
    *map.entry(4).or_insert(7) = 11;
    println!("{:?}", map);
    *map.entry(key()).or_insert(default_value()) += delta();
    println!(" {:?}", map.get(&1));
    *map.entry(5).or_insert_with(|| default_value()) += delta();
    println!(" {:?}", map.get(&5));
}
