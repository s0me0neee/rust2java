// Integer methods: clamp, abs_diff, euclidean division, bit counts per width, signum.
fn main() {
    println!("{} {}", 15i32.clamp(0, 10), (-5i32).clamp(0, 10));
    println!("{} {}", 7u8.clamp(10, 20), 250u32.clamp(0, 100));
    println!("{} {}", 4294967295u32.clamp(0, 100), 5u32.clamp(10, 20));
    println!("{} {}", 2.5f64.clamp(0.0, 1.0), (-2.5f64).clamp(0.0, 1.0));
    println!("{} {}", 37i64.abs_diff(50), 50i64.abs_diff(37));
    println!("{} {}", 0u32.abs_diff(4294967295), (-128i8).abs_diff(127));
    println!("{} {}", i32::MIN.abs_diff(i32::MAX), i64::MIN.abs_diff(i64::MAX));
    println!("{} {} {}", (-3i32).signum(), 0i32.signum(), 3i8.signum());
    println!("{} {}", (-7i32).div_euclid(3), (-7i32).rem_euclid(3));
    println!("{} {}", 7i32.div_euclid(-3), 7i32.rem_euclid(-3));
    println!("{} {}", 4294967295u32.div_euclid(7), 4294967295u32.rem_euclid(7));
    println!("{} {}", 250u8.div_euclid(7), 65000u16.rem_euclid(7));
    println!("{} {}", 12u32.is_power_of_two(), 2147483648u32.is_power_of_two());
    println!("{} {}", 0u32.is_power_of_two(), 0u8.is_power_of_two());
    println!("{} {}", 8i8.leading_zeros(), 8i8.trailing_zeros());
    println!("{} {}", 0u8.leading_zeros(), 0u8.trailing_zeros());
    println!("{} {}", (-1i16).leading_zeros(), 16u16.trailing_zeros());
    println!("{} {}", 1i32.leading_zeros(), 1024i32.trailing_zeros());
    println!("{} {}", 1u32.leading_zeros(), 0u32.trailing_zeros());
    println!("{} {}", 1i64.leading_zeros(), 1u64.trailing_zeros());
    println!("{} {}", 255u8.count_ones(), (-1i16).count_ones());
    println!("{} {}", 0u8.count_zeros(), 4i32.count_zeros());
    println!("{} {}", 0x1234u16.swap_bytes(), 0x12u8.reverse_bits());
    println!("{} {}", 0x01u32.rotate_left(8), 0x8000_0000u32.rotate_right(4));
    println!("{} {}", 0x12i8.rotate_left(4), 0x1234u16.rotate_right(4));
    println!("{} {}", 3.0f64.hypot(4.0), (2.0f64).hypot(3.0) > 3.6);
}
