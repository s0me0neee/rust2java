// Using a numeric method as a function value (`f64::max`) is not lowered yet.
fn main() {
    let values = [1.5, -2.5, 0.5];
    let biggest = values.iter().copied().fold(0.0, f64::max);
    println!("{}", biggest);
}
