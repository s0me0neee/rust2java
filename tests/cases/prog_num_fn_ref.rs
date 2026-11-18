// A numeric method as a function value (`f64::max`) is a `Math` method reference.
fn main() {
    let values = [1.5, -2.5, 0.5];
    let biggest = values.iter().copied().fold(0.0, f64::max);
    println!("{}", biggest);
}
