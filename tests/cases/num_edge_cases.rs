fn main() { more(); sat();
    let a: i32 = 7;
    let b: i32 = -2;
    println!("{} {} {} {}", a / b, a % b, a.rem_euclid(b), a.div_euclid(b));
    println!("{} {}", (-7i32).div_euclid(2), (-7i32).rem_euclid(2));
    println!("{} {}", 3i64.pow(40), 2u64.pow(63));
    println!("{} {}", 3i32.wrapping_pow(25), 10i32.checked_pow(10).is_none());
    println!("{:?} {:?} {:?}", 200u8.checked_add(100), i32::MAX.checked_add(1), 5u32.checked_sub(6));
    println!("{} {} {}", 250u8.saturating_add(10), (-5i32).saturating_sub(i32::MAX), i64::MIN.saturating_sub(1));
    println!("{} {}", 255u8.wrapping_add(3), 0u32.wrapping_sub(1));
    println!("{} {} {}", (-1i32) as u32, 3000000000u32 as i32, -1i64 as u8);
    println!("{} {} {}", 300i32 as u8, -129i32 as i8, 65u8 as char);
    println!("{} {} {}", 2.9f64 as i32, -2.9f64 as i32, 1e20 as i32);
    println!("{} {} {}", f64::NAN as i32, (0.1f32 + 0.2f32), 1.0f64 / 3.0);
    println!("{} {} {}", 2.5f64.round(), (-2.5f64).round(), 0.5f64.round());
    println!("{} {} {}", 7.0f64.sqrt(), (2.0f64).powi(10), (2.0f64).powf(0.5));
    println!("{} {} {}", f64::MAX, f64::MIN_POSITIVE, 1e15 + 0.5);
    println!("{} {} {}", 1e16, 123456789012345680.0f64, 0.000012345);
    println!("{} {} {}", u64::MAX, i64::MIN, u32::MAX);
    println!("{} {}", (u64::MAX / 3), u64::MAX % 1000);
    println!("{} {}", i32::MAX.count_ones(), (-1i32).leading_zeros());
    println!("{} {} {}", 40u32.trailing_zeros(), 1u64 << 40, (-16i32) >> 2);
    println!("{} {}", (-16i32 as u32) >> 2, 0x80000000u32 >> 31);
    println!("{} {}", i32::abs(-5), (-5i32).signum());
    println!("{} {} {}", 17 / 5 * 5, 1 + 2 * 3 - 4 / 2, (1 << 3) | 2 ^ 7 & 3);
    println!("{} {}", 10i32.min(3).max(5), 3.5f64.clamp(0.0, 1.0));
    println!("{:?} {:?}", "255".parse::<u8>(), "256".parse::<u8>().is_err());
    println!("{:?} {:?}", "-1".parse::<u32>().is_err(), "ff".parse::<i32>().is_err());
    println!("{:?}", i64::from_str_radix("-ff", 16));
    println!("{} {}", u8::MAX as u32 + 1, (b'z' - b'a') as usize);
    println!("{} {}", 7u32.abs_diff(10), (-3i32).abs_diff(4));
    println!("{} {}", 10u32.is_power_of_two(), 1000u64.ilog10());
    println!("{} {} {}", 9.0f64.ln().floor(), (-0.0f64).abs(), f64::INFINITY.min(3.0));
}
fn more() {
    let b: i64 = 3;
    let e: u32 = 35;
    println!("{} {} {}", b.pow(e), b.wrapping_pow(41), 7u64.pow(23));
    println!("{:?} {:?} {:?}", 2i32.checked_pow(30), 2i32.checked_pow(31), 3u8.checked_pow(5));
    println!("{} {} {}", 3u8.wrapping_pow(6), (-2i8).wrapping_pow(7), 10i64.pow(18));
}
fn sat() {
    let (a, b) = (i64::MAX - 5, 10i64);
    println!("{} {} {}", a.saturating_add(b), (-a).saturating_sub(b * 2), a.saturating_mul(-3));
    println!("{} {} {}", 5i64.saturating_add(-7), 4i64.saturating_mul(5), i64::MIN.saturating_mul(-1));
    let (u, v) = (u64::MAX - 1, 3u64);
    println!("{} {} {}", u.saturating_add(v), 2u64.saturating_sub(v), u.saturating_mul(2));
    println!("{} {}", 6u64.saturating_mul(7), 10u64.saturating_sub(4));
}
