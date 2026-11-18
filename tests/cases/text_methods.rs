// `str` methods: split_once, replacen, insert_str, strip_*, rfind, as_bytes, get, cmp.
use std::cmp::Ordering;

fn main() {
    let s = "a,b;c,d";
    println!("{:?} {:?}", s.split_once(','), s.split_once(';'));
    println!("{:?} {:?}", s.rsplit_once(','), "no-sep".split_once('x'));
    println!("{:?}", "".split_once(','));

    println!("{}", "banana".replacen('a', "o", 2));
    println!("{}", "banana".replacen("na", "NA", 1));
    println!("{}", "xyz".replacen('q', "-", 3));

    let mut b = String::from("abc");
    b.insert_str(1, "ZZ");
    println!("{}", b);

    println!(
        "{:?} {:?} {:?}",
        "prefix-body".strip_prefix("prefix-"),
        "x".strip_prefix("y"),
        "-dash".strip_prefix('-')
    );
    println!("{:?} {:?}", "tail!".strip_suffix('!'), "tail".strip_suffix('!'));
    println!(
        "{:?} {:?}",
        "body-suffix".strip_suffix("-suffix"),
        "body-suffix".strip_suffix("x")
    );

    println!("{:?} {:?}", "hello".rfind('l'), "hello".rfind("ll"));
    println!("{:?}", "abc".as_bytes());
    println!(
        "{:?} {:?} {:?}",
        "abcdef".get(1..3),
        "abcdef".get(4..),
        "abcdef".get(2..9)
    );
    println!("{:?} {:?}", "abcdef".get(..3), "abcdef".get(3..1));

    println!("{}", "HeLLo".eq_ignore_ascii_case("hello"));
    println!("{:?} {:?} {:?}", "b".cmp("a"), "b".cmp("b"), "a".cmp("b"));
    println!(
        "{} {} {}",
        "b".cmp("a") == Ordering::Greater,
        "b".cmp("b") >= Ordering::Equal,
        "a".cmp("b") < Ordering::Equal
    );

    match "b".cmp("a") {
        Ordering::Less => println!("less"),
        Ordering::Equal => println!("equal"),
        Ordering::Greater => println!("greater"),
    }

    println!("{} {}", format!("{}{}", "a", 1) == "a1", format!("x") != "y");

    let bytes = "abc".as_bytes();
    println!("{} {}", bytes.len(), bytes[1]);
    let bumped: Vec<u8> = "Hi".as_bytes().iter().map(|b| b + 1).collect();
    println!("{:?}", bumped);
}
