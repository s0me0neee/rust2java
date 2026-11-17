// Generic data enums: sealed interface + generic variant records.
#[derive(Debug, Clone, PartialEq)]
enum Maybe<T> {
    Nothing,
    Just(T),
}

fn wrap<T>(v: T) -> Maybe<T> {
    Maybe::Just(v)
}

fn main() {
    let a = Maybe::<i32>::Nothing;
    let b = wrap(5);
    let c = Maybe::Just(String::from("hi"));
    println!("{:?}", a);
    println!("{:?}", b);
    println!("{:?}", c);
    println!("{} {}", b == wrap(5), b == Maybe::Nothing);
    let d = b.clone();
    println!("{:?}", d);
}
