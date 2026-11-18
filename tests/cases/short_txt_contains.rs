// char pattern tests: contains(char) uses indexOf, and char::from drops the unboxing cast.
fn main() {
    let s = String::from("hello world");
    println!("{} {} {}", s.contains('o'), s.contains('z'), s.contains("lo w"));
    let c = 'w';
    println!("{} {} {}", s.contains(c), s.starts_with('h'), s.ends_with('d'));
    println!("{}", char::from(65u8));
}
