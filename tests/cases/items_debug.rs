// Derived Debug for strings, chars, floats, lists, options and tuples.
#[derive(Debug)]
struct Info {
    name: String,
    initial: char,
    ratio: f64,
    flags: Vec<bool>,
    maybe: Option<i32>,
    pair: (i32, String),
}

#[derive(Debug)]
struct Bag {
    items: Vec<Info>,
}

#[derive(Debug)]
struct Empty {}

#[derive(Debug)]
struct Marker;

fn main() {
    let info = Info {
        name: String::from("x"),
        initial: 'y',
        ratio: 1.5,
        flags: Vec::new(),
        maybe: Some(3),
        pair: (1, String::from("a")),
    };
    println!("{:?}", info);
    let mut flags = Vec::new();
    flags.push(true);
    flags.push(false);
    let info2 = Info { flags, maybe: None, ..info };
    println!("{:?}", info2);
    let mut items = Vec::new();
    items.push(info2);
    let bag = Bag { items };
    println!("{:?}", bag);
    println!("{:?} {:?}", Empty {}, Marker);
}
