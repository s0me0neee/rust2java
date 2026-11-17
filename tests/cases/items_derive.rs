// Derives: Debug, Clone, PartialEq/Eq/Hash, PartialOrd/Ord, Default, Copy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
struct Version {
    major: i32,
    minor: i32,
    tags: Vec<String>,
}

impl Version {
    fn new(major: i32, minor: i32) -> Version {
        Version { major, minor, tags: Vec::new() }
    }
}

#[derive(Debug, Default, Copy, Clone)]
struct Config {
    retries: i32,
    verbose: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct Wrapper(String);

fn main() {
    let a = Version::new(1, 2);
    let b = Version::new(1, 3);
    println!("{:?}", a);
    println!("{:?}", Version::default());
    let c = a.clone();
    println!("{} {} {}", a == c, a != b, a < b);
    let d = Config::default();
    println!("{:?}", d);
    println!("{:?}", Config { retries: 5, ..d });
    println!("{:?}", Wrapper(String::from("hi")));
    println!("{}", Wrapper(String::from("x")) == Wrapper(String::from("x")));
}
