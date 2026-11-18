// Casts that used to mask an already-in-range value.
fn main() {
    println!("{} {}", b'a' as u8 + 1, b'z' as u32);
    println!("{} {}", 'A' as u32 + 1, 'Ā' as u8);
    println!("{} {}", 65u8 as char, 300i32 as u8);
    let code = 250u8;
    println!("{}", code as char);
    println!("{} {}", 250u8.div_euclid(7), 250u8.rem_euclid(7));
    println!("{} {}", 8i8.leading_zeros(), 16u16.trailing_zeros());
    println!(
        "{} {}",
        0x12i8.rotate_left(4),
        0x1234u16.rotate_right(4)
    );
}
