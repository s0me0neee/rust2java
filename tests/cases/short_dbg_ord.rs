#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Row {
    a: i32,
    b: i32,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Version {
    major: u32,
    minor: u32,
    tags: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct WithOpt {
    a: i32,
    maybe: Option<i32>,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Tuple(u8, char);

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Expr {
    Num(i32),
    Add(i32, i32),
}

fn main() {
    let a = Row { a: 1, b: 2 };
    let b = Row { a: 1, b: 3 };
    let c = Row { a: 2, b: 0 };
    println!("{:?} {:?} {:?}", a.cmp(&b), b.cmp(&c), c.cmp(&a));
    println!("{} {} {}", a < b, b < c, a.cmp(&b) == std::cmp::Ordering::Less);

    let mut versions = vec![
        Version { major: 2, minor: 0, tags: vec![String::from("b")] },
        Version { major: 1, minor: 5, tags: vec![String::from("z")] },
        Version { major: 2, minor: 0, tags: vec![String::from("a"), String::from("q")] },
    ];
    versions.sort();
    println!("{:?}", versions);

    let x = WithOpt { a: 1, maybe: None };
    let y = WithOpt { a: 1, maybe: Some(0) };
    let z = WithOpt { a: 1, maybe: Some(5) };
    println!("{:?} {:?} {:?}", x.cmp(&y), y.cmp(&z), z.cmp(&x));

    println!("{:?} {:?}", Tuple(3, 'a').cmp(&Tuple(3, 'b')), Tuple(4, 'a').cmp(&Tuple(3, 'z')));

    println!("{:?} {:?}", Expr::Num(1).cmp(&Expr::Num(2)), Expr::Num(9).cmp(&Expr::Add(1, 2)));
    println!("{:?} {:?}", Expr::Add(1, 5).cmp(&Expr::Add(1, 4)), Expr::Add(1, 5).cmp(&Expr::Add(2, 0)));
}
