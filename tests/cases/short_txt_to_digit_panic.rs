// A non-digit still exits 101 (Java's NumberFormatException message, which the design accepts).
fn main() {
    println!("before");
    let d = 'x'.to_digit(10).unwrap();
    println!("{}", d);
}
