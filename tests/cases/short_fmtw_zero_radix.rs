fn main() {
    let n = 42;
    let neg = -42i32;
    let u8v: u8 = 255;
    let i8v = -1i8;
    let i16v = -1i16;
    let u16v: u16 = 65535;
    let l: i64 = 42;
    println!("[{:#08x}] [{:#08o}] [{:#08b}] [{:#08X}]", n, n, n, u8v);
    println!("[{:08x}] [{:08o}] [{:08b}] [{:08X}]", n, n, n, n);
    println!("[{:#08x}] [{:08x}] [{:08X}] [{:08o}]", neg, neg, neg, neg);
    println!("[{:08x}] [{:08X}] [{:08o}] [{:08b}]", i8v, i8v, i8v, i8v);
    println!("[{:08x}] [{:08X}] [{:08o}] [{:08b}]", i16v, i16v, i16v, i16v);
    println!("[{:#010x}] [{:010X}] [{:010o}] [{:010b}]", u16v, u16v, u16v, u16v);
    println!("[{:#012x}] [{:012X}] [{:012o}] [{:012b}]", l, l, l, l);
    println!("[{:#4x}] [{:#3x}] [{:4X}] [{:4b}]", 255u8, 255u8, 255u8, 255u8);
    println!("[{:08x}] [{:08x}] [{:#08x}]", 0, 1, 0);
}
