#[derive(Debug)]
struct P {
    x: i32,
}

fn tick(log: &mut Vec<i32>, v: i32) -> i32 {
    log.push(v);
    v * 10
}

fn main() {
    println!("[{:>8?}] [{:>6?}] [{:>6?}] [{:5?}] [{:?}]", "ab", 'c', 1.5, 7, P { x: 1 });
    println!("[{:>8}] [{:<6}] [{:6}] [{:+}] [{:08.3}] [{:#06x}] [{:5}]", "ab", 'c', true, 5, -1.5, 255, 1.5);
    println!("[{:8.3}] [{:05}] [{:x}] [{:X}] [{:o}]", "abcdef", -42, -1i8, -1i16, 8u16);
    let pct = 12.5;
    println!("{:.1}% done, 100% sure", pct);
    let width = 7;
    let prec = 3;
    println!("[{:>width$.prec$}] [{:<width$}] [{:^5}]", 3.14159, "ab", "mid");
    println!("[{:>1$}] [{:.*}]", 9, 4, 0.5);
    let mut log = Vec::new();
    let a = tick(&mut log, 1);
    println!("{} {} {}", a, tick(&mut log, 2), log.len());
    let s = format!("{1}-{0}-{1}", tick(&mut log, 3), tick(&mut log, 4));
    println!("{} {:?}", s, log);
    let v = vec![3, 1, 2];
    println!("len={} first={:?} last={:>4}", v.len(), v[0], v[v.len() - 1]);
    println!("{}{}", 1, 2);
    println!("{}", 'a' as u8 + 1);
    let word = String::from("hi");
    println!("{word:>6}|{word:-<6}|{}", word.len());
}
