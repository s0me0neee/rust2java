// `&mut` of an `Option<Box<T>>` local: the alias shares the cell, so the write reaches `head`.
struct Node {
    value: i32,
}

fn main() {
    let mut head: Option<Box<Node>> = Some(Box::new(Node { value: 1 }));
    let slot = &mut head;
    *slot = Some(Box::new(Node { value: 2 }));
    println!("{}", head.as_ref().map(|node| node.value).unwrap_or(-1));
}
