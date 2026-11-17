fn main() {
    let n = 42;
    let word = "cat";
    println!("{} {} {}", 'a', 'b', true);
    println!("{:5}|{:>5}|{:^6}|{:_^6}", word, word, word, word);
    println!("{:5}|{:<5}|{:05}|{:+05}", n, n, n, n);
    println!("{:x}|{:X}|{:o}|{:b}|{:#x}|{:#o}|{:#b}", n, n, n, n, n, n, n);
    println!("{:.2}|{:8.2}|{:+08.2}", 1.25, 1.25, 1.25);
    println!("{0} {0} {name}", n, name = word);
    println!("{n:>width$}|{:.prec$}|{:.*}", 1.234, 2, 1.234, width = 5, prec = 2);
    println!("{:#08x}|{:#08o}|{:#08b}|{:+x}", 42, 42, 42, -42i32);
    println!("{:.3}|{:*^8}", "abcdef", "a");
    println!("{:05}|{:05}|{:05}|", "cat", 'x', true);
    println!("{:>8?}|{:05?}|{:.2?}", "cat", 3, 1.234);
    println!("{:5?}|{:.2?}|{:5?}", vec![1, 2], vec![1.234, 2.345], Some(3));
    println!("{:5?}|{:5?}|{:.0?}", true, 'x', 1.25);
}
