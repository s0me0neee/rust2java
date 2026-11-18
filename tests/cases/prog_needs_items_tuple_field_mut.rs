// Mutating a tuple field goes through `getKey()`/`getValue()`, which Java cannot assign.
fn main() {
    let mut stats = (0, 0.0);
    stats.0 += 1;
    stats.1 += 2.5;
    println!("{:?}", stats);
}
