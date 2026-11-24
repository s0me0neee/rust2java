struct Fib {
    a: u64,
    b: u64,
}

impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        let r = self.a;
        self.a = self.b;
        self.b = self.b.checked_add(r)?;
        Some(r)
    }
}

fn fib() -> Fib {
    Fib { a: 0, b: 1 }
}

struct Collatz(u64);

impl Iterator for Collatz {
    type Item = u64;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0 == 1 {
            return None;
        }
        self.0 = if self.0 % 2 == 0 { self.0 / 2 } else { 3 * self.0 + 1 };
        Some(self.0)
    }
}

fn main() {
    let small: Vec<u64> = fib().take_while(|&x| x < 100).collect();
    println!("{:?}", small);
    let even_sum: u64 = fib().take_while(|&x| x < 4_000_000).filter(|x| x % 2 == 0).sum();
    println!("{even_sum}");
    println!("{} fibs fit in u64", fib().count());
    let longest = (1..30u64).max_by_key(|&n| Collatz(n).count()).unwrap();
    println!("longest collatz under 30: {} ({} steps)", longest, Collatz(longest).count());
    let zipped: Vec<String> = fib().skip(5).step_by(3).zip(Collatz(27)).take(4).map(|(a, b)| format!("{a}/{b}")).collect();
    println!("{}", zipped.join(" "));
}
