// Mutating a tuple field builds a new tuple for the place.
fn main() {
    let mut stats = (0, 0.0);
    stats.0 += 1;
    stats.1 += 2.5;
    println!("{:?}", stats);
}
