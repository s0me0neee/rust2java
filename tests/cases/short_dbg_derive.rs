use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Debug)]
struct Card {
    rank: u8,
    suit: char,
    label: String,
}

#[derive(Debug)]
struct Nested {
    tags: Vec<i32>,
    grid: Vec<Vec<i32>>,
    names: Vec<String>,
    chars: Vec<char>,
    flags: Vec<bool>,
    set: BTreeSet<i32>,
    map: BTreeMap<String, Vec<i32>>,
    maybe: Option<u32>,
}

#[derive(Debug)]
struct Pair(i32, String);

#[derive(Debug)]
struct Deep {
    opt_list: Option<Vec<i32>>,
    map: BTreeMap<i32, Vec<i32>>,
    list_opt: Vec<Option<Vec<i32>>>,
    list_str: Option<Vec<String>>,
    pair: (i32, Vec<i32>),
}

#[derive(Debug)]
struct Triple(i32, String, char);

#[derive(Debug)]
struct Marker;

fn main() {
    let p = Point { x: 3, y: -4 };
    println!("{:?}", p);
    let c = Card {
        rank: 7,
        suit: '\'',
        label: String::from("a\"b\nc"),
    };
    println!("{:?}", c);
    let mut set = BTreeSet::new();
    set.insert(3);
    set.insert(1);
    let mut map = BTreeMap::new();
    map.insert(String::from("k"), vec![1, 2]);
    let n = Nested {
        tags: vec![1, 2, 3],
        grid: vec![vec![1, 2], vec![3]],
        names: vec![String::from("x"), String::from("y\"z")],
        chars: vec!['a', '\n'],
        flags: vec![true, false],
        set,
        map,
        maybe: Some(4294967295),
    };
    println!("{:?}", n);
    println!(
        "{:?} {:?} {:?}",
        Pair(1, String::from("p")),
        Triple(2, String::from("t"), 'q'),
        Marker
    );
    let list = vec![p, Point { x: 0, y: 1 }];
    println!("{:?}", list);
    let mut deep_map = BTreeMap::new();
    deep_map.insert(1, vec![2, 3]);
    let deep = Deep {
        opt_list: Some(vec![4, 5]),
        map: deep_map,
        list_opt: vec![Some(vec![6]), None],
        list_str: Some(vec![String::from("a\"b")]),
        pair: (7, vec![8]),
    };
    println!("{:?}", deep);
    println!("{:?}", Some(vec![vec![1], Vec::new()]));
}
