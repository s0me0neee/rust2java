// `Option::unwrap` panics with Rust's message and exit 101.
fn main() {
    println!("before");
    let o: Option<i32> = None;
    let _ = o.unwrap();
}
