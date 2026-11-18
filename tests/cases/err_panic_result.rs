// `Result::unwrap` panics with Rust's message and exit 101.
fn main() {
    println!("before");
    let r: Result<i32, String> = Err("boom".into());
    let _ = r.unwrap();
    println!("after");
}
