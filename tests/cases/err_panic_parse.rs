// Unwrapping a parse error panics with the ParseIntError Debug text.
fn main() {
    println!("before");
    let n: i32 = "zz".parse().unwrap();
    println!("{}", n);
}
