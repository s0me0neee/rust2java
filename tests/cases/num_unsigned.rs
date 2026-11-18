// Unsigned printing, division, comparison and shifts above the signed range.
fn main() {
    let a: u32 = 4_294_967_295;
    let b: u64 = 18_446_744_073_709_551_615;
    println!("{} {}", a, b);
    println!("{} {} {}", a / 7, a % 7, a / 2);
    println!("{} {}", b / 3, b % 1_000_000_007);
    println!("{} {}", a > 5, a < 10);
    println!("{} {}", 2147483648u32 > 1, 1u32 < 4294967295);
    println!("{}", a >> 4);
    println!("{}", 4294967295u32 as u64);
    println!("{}", u32::MAX as u64 + 1);
    let s: u16 = 65000;
    println!("{} {}", s, s + 1);
    println!("{} {}", s > 100, s.to_string());
    println!("{} {}", 255u8, 255u8 as i32);
    println!("{}", 1u64 << 63);
    println!("{}", 1u32 << 31);
    println!("{}", (1u64 << 63) as i64);
    println!("{:?}", 4294967295u32.cmp(&1u32));
    println!("{}", 4294967295u32.max(7));
    println!("{}", b.max(0));
}
