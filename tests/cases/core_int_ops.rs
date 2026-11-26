// Integer methods across widths: checked / overflowing / saturating / wrapping / strict families,
// shifts, logs and roots, bit queries, byte order, mixed-sign arithmetic. Values stay inside the
// signed range of each Java storage type (u32 / u64 past it are out of scope).
use std::fmt::NumBuffer;

fn main() {
    let (a8, b8): (i8, i8) = (-128, 100);
    let (a16, b16): (i16, i16) = (-300, 7);
    let (a32, b32): (i32, i32) = (i32::MIN, -1);
    let (a64, b64): (i64, i64) = (-1_000_000_000_000, 3);
    let (u8a, u8b): (u8, u8) = (250, 9);
    let (u16a, u16b): (u16, u16) = (300, 7);
    let (u32a, u32b): (u32, u32) = (1_000_000, 3);
    let u64a: u64 = 5_000_000_000;

    // checked_*
    println!("{:?} {:?} {:?} {:?}", a8.checked_abs(), b8.checked_neg(), a8.checked_sub(1), a16.checked_sub(5));
    println!("{:?} {:?} {:?}", a32.checked_div_euclid(b32), a16.checked_div_euclid(b16), a16.checked_rem_euclid(b16));
    println!("{:?} {:?} {:?} {:?}", b8.checked_shl(3), b8.checked_shl(8), a64.checked_shr(63), a64.checked_shr(64));
    println!("{:?} {:?} {:?} {:?}", 1000i32.checked_ilog(10), 0i32.checked_ilog2(), 999i64.checked_ilog10(), (-4i16).checked_isqrt());
    println!("{:?} {:?} {:?}", 120i8.checked_add_unsigned(7), 120i8.checked_add_unsigned(8), (-120i8).checked_sub_unsigned(9));
    println!("{:?} {:?} {:?}", u8b.checked_add_signed(-10), u8a.checked_add_signed(5), u16a.checked_sub_signed(-7));
    println!("{:?} {:?} {:?} {:?}", u8a.checked_div_euclid(0), u16a.checked_rem_euclid(u16b), u32a.checked_ilog(10), u8b.checked_ilog2());
    println!("{:?} {:?} {:?}", 0u8.checked_neg(), 1u8.checked_neg(), u16a.checked_rem(0));
    println!("{:?} {:?} {:?}", u8a.checked_next_multiple_of(5), u8a.checked_next_multiple_of(7), u8b.checked_next_multiple_of(0));
    println!("{:?} {:?} {:?}", 100u8.checked_next_power_of_two(), 200u8.checked_next_power_of_two(), u8b.checked_signed_diff(u8a));
    println!("{:?} {:?} {:?}", u16a.checked_shl(16), u32a.checked_shr(31), u64a.checked_ilog10());

    // overflowing_*
    println!("{:?} {:?} {:?}", a8.overflowing_abs(), a8.overflowing_add(-1), b8.overflowing_sub(-100));
    println!("{:?} {:?} {:?}", b8.overflowing_mul(2), a32.overflowing_div(b32), a32.overflowing_div_euclid(b32));
    println!("{:?} {:?} {:?}", a32.overflowing_rem(b32), a32.overflowing_rem_euclid(b32), a16.overflowing_neg());
    println!("{:?} {:?} {:?}", 3i8.overflowing_pow(5), b8.overflowing_shl(9), a64.overflowing_shr(65));
    println!("{:?} {:?}", 100i8.overflowing_add_unsigned(200), (-100i8).overflowing_sub_unsigned(200));
    println!("{:?} {:?} {:?}", u8a.overflowing_add(u8b), u8b.overflowing_sub(u8a), u8a.overflowing_mul(2));
    println!("{:?} {:?} {:?}", u16a.overflowing_div(u16b), u16a.overflowing_div_euclid(u16b), u16a.overflowing_rem(u16b));
    println!("{:?} {:?} {:?}", u16a.overflowing_rem_euclid(u16b), 5u8.overflowing_neg(), 2u8.overflowing_pow(9));
    println!("{:?} {:?} {:?} {:?}", 1u16.overflowing_shl(17), u32a.overflowing_shr(33), u8a.overflowing_add_signed(10), u8b.overflowing_sub_signed(10));

    // saturating_*
    println!("{} {} {} {}", a8.saturating_abs(), a8.saturating_neg(), a32.saturating_div(b32), 3i16.saturating_pow(20));
    println!("{} {}", 100i8.saturating_add_unsigned(200), (-100i8).saturating_sub_unsigned(200));
    println!("{} {} {} {}", u8a.saturating_add_signed(10), u8b.saturating_sub_signed(10), u16a.saturating_div(u16b), 10u8.saturating_pow(3));

    // strict_* (no overflow; the last line overflows and panics)
    println!("{} {} {} {}", b8.strict_abs(), b8.strict_add(27), b8.strict_sub(-27), 11i8.strict_mul(11));
    println!("{} {} {} {}", a16.strict_div(b16), a16.strict_div_euclid(b16), a16.strict_rem(b16), a16.strict_rem_euclid(b16));
    println!("{} {} {} {}", b8.strict_neg(), 2i64.strict_pow(40), 1i32.strict_shl(30), a64.strict_shr(4));
    println!("{} {}", 100i8.strict_add_unsigned(27), (-100i8).strict_sub_unsigned(28));
    println!("{} {} {} {}", u8a.strict_add(5), u8a.strict_sub(u8b), 15u8.strict_mul(17), u16a.strict_div(u16b));
    println!("{} {} {} {}", u16a.strict_div_euclid(u16b), u16a.strict_rem(u16b), u16a.strict_rem_euclid(u16b), 0u8.strict_neg());
    println!("{} {} {} {} {}", 3u32.strict_pow(10), 1u8.strict_shl(7), u64a.strict_shr(2), u8b.strict_add_signed(-9), u8b.strict_sub_signed(-9));

    // shifts that never panic
    println!("{} {} {} {}", 1i8.unbounded_shl(7), 1i8.unbounded_shl(8), (-8i32).unbounded_shr(2), (-8i32).unbounded_shr(40));
    println!("{} {} {}", u8a.unbounded_shl(1), u8a.unbounded_shr(9), u64a.unbounded_shl(64));

    // logs, roots, midpoint
    println!("{} {} {} {}", 81i32.ilog(3), 1024i64.ilog2(), 12345i16.ilog10(), 99i32.isqrt());
    println!("{} {} {} {}", 255u8.ilog(2), u16a.ilog2(), u64a.isqrt(), u32a.isqrt());
    println!("{} {} {} {}", (-7i32).midpoint(4), 7i32.midpoint(-4), i8::MIN.midpoint(i8::MAX), u8a.midpoint(u8b));
    println!("{} {} {}", u16a.div_ceil(u16b), u8b.next_multiple_of(4), u8a.bit_width());

    // bit queries
    println!("{} {} {} {}", (-1i8).leading_ones(), (-16i16).leading_ones(), 7i32.trailing_ones(), u8a.leading_ones());
    println!("{:?} {:?} {:?} {:?}", 0b0101_0000u8.highest_one(), 0u16.highest_one(), (-8i32).lowest_one(), 0i64.lowest_one());
    println!("{} {} {} {}", 0b0110u8.isolate_highest_one(), 0b0110u8.isolate_lowest_one(), (-8i16).isolate_highest_one(), 96i32.isolate_lowest_one());
    println!("{} {} {}", 1i8.reverse_bits(), 0x12345678i32.rotate_right(8), 0x1234i16.swap_bytes());

    // byte order
    println!("{} {} {} {}", 0x1234i16.to_be(), 0x1234i16.to_le(), i32::from_be(0x01020304), u16::from_le(0x0102));
    println!("{:?} {:?} {:?}", 258i16.to_be_bytes(), 258u32.to_le_bytes(), 258i32.to_ne_bytes());
    println!("{} {} {} {}", i16::from_be_bytes([1, 2]), u32::from_le_bytes([4, 3, 2, 1]), i32::from_ne_bytes([1, 0, 0, 0]), u16::from_ne_bytes([2, 1]));

    // mixed-sign carries and casts
    println!("{:?} {:?} {:?}", u8a.carrying_add(u8b, true), 3u16.borrowing_sub(5, false), 3u8.borrowing_sub(2, true));
    println!("{:?} {:?}", 200u8.carrying_mul(3, 7), 200u8.carrying_mul_add(3, 7, 100));
    println!("{} {} {} {}", 200u8.cast_signed(), (-56i8).cast_unsigned(), 40000u16.cast_signed(), (-1i64).cast_unsigned() >> 60);
    let mut buf = NumBuffer::new();
    println!("{} {}", 1234i32.format_into(&mut buf), b8.is_negative());

    // strict overflow panics
    println!("{}", i8::MAX.strict_add(1));
    println!("unreachable");
}
