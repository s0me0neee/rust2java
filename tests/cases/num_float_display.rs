// Float Display corner cases: no exponent, trailing zeros, f32 digits (-0/NaN/inf are accepted gaps).
fn main() {
    println!("{}", 1e-7f64);
    println!("{}", 1e21f64);
    println!("{}", 0.0f64);
    println!("{}", f64::MAX);
    println!("{}", f64::MIN);
    println!("{}", 1.0f64 / 3.0);
    println!("{}", 1.0f32 / 3.0);
    println!("{}", 1e-7f32);
    println!("{}", f32::MAX);
    println!("{}", 12345678901234567890.0f64);
    println!("{} {}", -1.5f64, 2.0f64);
    println!("{}", 2.5f64 - 2.5);
}
