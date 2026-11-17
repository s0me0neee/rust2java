// Integer, float and Ordering methods (std_map).
fn main() {
    let a: i32 = -7;
    let b: i32 = 3;
    println!("{} {} {}", a.abs(), a.pow(2), a.rem_euclid(b));
    println!("{} {} {}", a.min(b), a.max(b), a.div_euclid(b));
    println!("{} {} {}", 12i32.count_ones(), 12i32.leading_zeros(), 12i32.trailing_zeros());
    println!("{} {}", 12i32.count_ones() + 12i32.count_zeros(), 7i32.is_positive());
    println!("{:?} {:?}", 100i32.checked_add(27), 100i32.checked_add(2_000_000_000));
    println!("{:?} {:?}", 5u8.checked_sub(9), 9u8.checked_sub(5));
    println!("{:?} {:?}", 6i32.checked_div(3), 6i32.checked_div(0));
    println!("{:?} {:?}", 100i8.checked_mul(2), 100i8.checked_mul(1));
    println!("{} {} {}", 200u8.saturating_add(100), 5u8.saturating_sub(9), 20i32.saturating_sub(100));
    println!("{} {}", 20i32.saturating_mul(100000), 200i32.wrapping_add(100));
    let min = -128i8;
    println!("{} {}", min.wrapping_neg(), 100i32.rem_euclid(-7));
    println!("{} {} {}", i32::MAX, i32::MIN, u8::MAX);
    println!("{} {} {}", i64::MIN, i16::MAX, u16::MAX);
    println!("{} {}", 255u8, 65_535u16);
    println!("{}", i32::from_str_radix("ff", 16).unwrap());
    println!("{:?}", i32::from_str_radix("zz", 16).is_err());
    println!("{} {}", 7i32.cmp(&9).is_lt(), 7i32.partial_cmp(&9).is_some());
    println!("{} {}", std::cmp::min(3, 9), std::cmp::max(3, 9));
    println!("{} {}", 2.5f64.min(1.5), std::cmp::max("apple", "pear"));

    let f = 2.5f64;
    println!("{} {} {} {}", f.abs(), f.sqrt(), f.floor(), f.ceil());
    println!("{} {} {}", f.powi(2), f.powf(0.5), f.round());
    println!("{} {} {}", f.is_nan(), f.is_finite(), (1.0f64 / 3.0).is_finite());
    println!("{} {}", 1.0f64 / 3.0, 2.0f64);
    println!("{} {}", 0.1f64 + 0.2, 1.5f64.round());
    println!("{}", (-1.5f64).round());
    println!("{} {}", std::f64::consts::PI, std::f64::EPSILON);
    println!("{} {}", f64::MAX, f64::MIN_POSITIVE);
    println!("{} {}", std::f32::consts::PI, std::f32::EPSILON);
    println!("{} {}", (4.0f64).sqrt(), (9.0f32).sqrt());
    println!("{:?} {}", f.partial_cmp(&1.0).is_some(), f.is_finite());
    println!("{} {}", f.clamp(0.0, 1.0), 3.7f64.trunc());
}
