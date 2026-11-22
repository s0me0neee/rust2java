struct Fib { a: u64, b: u64 }
impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> Option<u64> { let r = self.a; self.a = self.b; self.b += r; Some(r) }
}
struct Countdown(u32);
impl Iterator for Countdown {
    type Item = u32;
    fn next(&mut self) -> Option<Self::Item> { if self.0 == 0 { None } else { self.0 -= 1; Some(self.0 + 1) } }
}
fn evens() -> impl Iterator<Item = u32> { (0..).step_by(2) }
fn main() {
    let f: Vec<u64> = Fib { a: 0, b: 1 }.take(12).collect();
    println!("{:?}", f);
    let big = Fib { a: 0, b: 1 }.skip_while(|&x| x < 1000).next();
    println!("{:?}", big);
    let s: u32 = Countdown(5).sum();
    let v: Vec<String> = Countdown(3).map(|x| format!("<{x}>")).collect();
    println!("{s} {}", v.concat());
    for (i, e) in evens().take(3).enumerate() { print!("{i}:{e} "); }
    println!();
    let mut it = Countdown(4).peekable();
    while let Some(x) = it.next() {
        if let Some(&n) = it.peek() { print!("{x}->{n} "); } else { println!("{x} end"); }
    }
    let z: Vec<(u32, char)> = Countdown(3).zip("abc".chars()).collect();
    println!("{:?}", z);
    let folded = Countdown(4).fold(String::new(), |mut acc, x| { if !acc.is_empty() { acc.push(','); } acc += &x.to_string(); acc });
    println!("{folded}");
    let (mn, mx) = Countdown(6).fold((u32::MAX, 0), |(lo, hi), x| (lo.min(x), hi.max(x)));
    println!("{mn} {mx} {}", Countdown(10).filter(|x| x % 3 == 0).count());
    let chained: Vec<u32> = Countdown(2).chain(Countdown(2).map(|x| x * 10)).collect();
    println!("{chained:?} {:?}", Countdown(5).position(|x| x == 2));
}
