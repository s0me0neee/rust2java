// Numeric From / TryFrom / into between std types.
fn main() {
    let a: i32 = 7;
    let wide: i64 = i64::from(a);
    println!("{}", wide * 1_000_000_000);
    let f: f64 = f64::from(a) / 2.0;
    println!("{}", f);
    let u: u32 = 4_000_000_000;
    let ul: u64 = u64::from(u);
    println!("{}", ul + 1);
    let s: u16 = u16::from(200u8);
    println!("{}", s + 1);
    let up: f64 = 4294967295u32.into();
    println!("{}", up);
    let c: u32 = u32::from('A');
    println!("{}", c);
    let ch = char::from(65u8);
    println!("{}", ch);
    println!("{}", u8::from(true));
    let ok = u8::try_from(42i32);
    println!("{} {}", ok.unwrap(), u8::try_from(300i32).is_err());
    println!("{:?} {:?}", u8::try_from(300i32).is_ok(), u16::try_from(70000i32).is_err());
    println!("{} {}", u8::try_from(300i32).unwrap_or(7), u8::try_from(9i32).unwrap_or(7));
    println!("{:?}", usize::try_from(-1i32).is_err());
    println!("{:?} {:?}", i8::try_from(200i32).is_err(), i8::try_from(127i32).unwrap());
    println!("{:?}", i32::try_from(5_000_000_000i64).is_err());
    println!("{:?}", i32::try_from(-5i64).unwrap());
    let small: u16 = u16::try_from(1000u32).unwrap();
    println!("{}", small);
    println!("{}", 65u8 as char);
    println!("{} {}", 'A' as u8, 'Ā' as u8);
    println!("{}", 'A' as u32 + 1);
    println!("{}", 300i32 as u8);
    println!("{}", -1i32 as u32);
    println!("{}", u32::MAX as u64 + 1);
    println!("{} {}", 3.99f64 as i32, -3.99f64 as i32);
    println!("{}", 300.7f64 as u8);
    println!("{}", -5.0f64 as u8);
}
