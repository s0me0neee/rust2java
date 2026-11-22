fn main() {
    let xs = [0.1 + 0.2, 1e21, 1e-7, 1.0 / 3.0, -0.0, 0.0, f64::NAN, f64::NEG_INFINITY, 100.0, 1.5e300, 123456789.125, 9.999, 2.5e-308];
    for x in xs { println!("{:e} {:E} {:.2e} {:.0e} {:>12e}|", x, x, x, x, x); }
    let f: f32 = 0.1;
    println!("{:e} {:.3e}", f, 2.5f32);
    println!("{:e} {:e} {:.1e}", 1234, -7i64, 1250u32);
}
