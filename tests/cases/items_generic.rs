// Generics: generic structs/impls/fns, bounds mapped to Java bounds.
struct Pair<T> {
    first: T,
    second: T,
}

impl<T> Pair<T> {
    fn new(first: T, second: T) -> Pair<T> {
        Pair { first, second }
    }

    fn first_of(&self) -> &T {
        &self.first
    }
}

impl<T: std::fmt::Display> Pair<T> {
    fn show(&self) -> String {
        format!("({}, {})", self.first, self.second)
    }
}

fn bigger<T: PartialOrd>(a: &T, b: &T) -> bool {
    a > b
}

fn max_of<T>(a: T, b: T) -> T
where
    T: PartialOrd + Copy,
{
    if a > b { a } else { b }
}

fn main() {
    let p = Pair::new(3, 4);
    println!("{}", p.show());
    println!("{} {}", p.first_of(), bigger(&3, &5));
    println!("{} {}", max_of(7, 9), max_of("a", "z"));
    let q = Pair::new(String::from("a"), String::from("b"));
    println!("{} {}", q.show(), bigger(&"a", &"b"));
}
