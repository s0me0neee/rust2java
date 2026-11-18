// Clones of immutable records share the value; variants holding containers still copy deeply.
#[derive(Debug, Clone, PartialEq)]
enum Maybe {
    Nothing,
    Word(String),
    Num(i32),
}

#[derive(Debug, Clone, PartialEq)]
enum Bag {
    Empty,
    Items(Vec<i32>),
}

#[derive(Debug, Clone, PartialEq)]
struct Wrapped {
    tag: Maybe,
    id: i32,
}

fn main() {
    let a = Maybe::Word(String::from("hi"));
    let b = a.clone();
    println!("{} {} {:?}", a == b, a.clone() == a, b);

    let x = Bag::Items(vec![1, 2]);
    let mut y = x.clone();
    match y {
        Bag::Items(ref mut v) => v.push(3),
        _ => {}
    }
    println!("{:?} {:?} {}", x, y, x == y);

    let w = Wrapped {
        tag: Maybe::Num(4),
        id: 1,
    };
    let w2 = w.clone();
    println!("{} {:?}", w == w2, w2);
    let empty = Bag::Empty;
    println!("{}", empty == empty.clone());
}
