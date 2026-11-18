// `Result::expect` panics with `msg: <debug>` and exit 101.
fn main() {
    println!("before");
    let r: Result<i32, String> = Err("boom".into());
    let _ = r.expect("my msg");
}
