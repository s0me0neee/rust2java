// Debug/Display of generic values: strings quoted, floats, Options, collections, crate types.
use std::fmt::{Debug, Display};
use std::collections::HashMap;
#[derive(Debug, Clone)]
struct P { x: i32, name: String }
fn show<T: Debug>(items: &[T]) {
    for (i, it) in items.iter().enumerate() { println!("{} = {:?}", i, it); }
    println!("{:?}", items);
}
fn pair<A: Debug, B: Display>(a: A, b: B) -> String { format!("{:?}/{}/{:>6}", a, b, b) }
fn main() {
    show(&[1, 2]);
    show(&["a\"b", "c"]);
    show(&[Some(1.5), None]);
    show(&[P { x: 1, name: "n".into() }]);
    show(&[vec!['x', 'y']]);
    let mut m = HashMap::new(); m.insert("k", 2.0);
    show(&[m]);
    println!("{} {} {}", pair("s", 2.0), pair(3.0, "t"), pair((1, "z"), 7));
}
