// mem::size_of for primitive types, alongside swap.
use std::mem;

fn main() {
    println!("{} {} {}", mem::size_of::<u8>(), mem::size_of::<i32>(), mem::size_of::<u64>());
    println!("{} {} {}", mem::size_of::<f32>(), mem::size_of::<f64>(), mem::size_of::<bool>());
    println!("{} {} {}", mem::size_of::<char>(), mem::size_of::<i128>(), mem::size_of::<usize>());
    println!("{} {}", mem::size_of::<()>(), mem::size_of::<i16>());
    let mut a = 1;
    let mut b = 2;
    mem::swap(&mut a, &mut b);
    println!("{} {} {}", a, b, mem::size_of::<u16>());
}
