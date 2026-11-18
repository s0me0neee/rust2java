// `Option::expect` panics with its message and exit 101.
fn main() {
    println!("before");
    let o: Option<i32> = None;
    let _ = o.expect("gone");
}
