// Functions returning impl Iterator, a custom Iterator struct, iterator adaptors chained across
// function boundaries, Box<dyn Iterator>, and generic functions taking IntoIterator.
struct Collatz {
    n: u64,
    done: bool,
}

impl Iterator for Collatz {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.done {
            return None;
        }
        let cur = self.n;
        if cur == 1 {
            self.done = true;
        } else if cur % 2 == 0 {
            self.n = cur / 2;
        } else {
            self.n = 3 * cur + 1;
        }
        Some(cur)
    }
}

fn collatz(n: u64) -> Collatz {
    Collatz { n, done: false }
}

fn evens(limit: u32) -> impl Iterator<Item = u32> {
    (0..limit).filter(|x| x % 2 == 0)
}

fn squares_of<'a>(v: &'a [i32]) -> impl Iterator<Item = i32> + 'a {
    v.iter().map(|x| x * x)
}

fn fib() -> impl Iterator<Item = u64> {
    let mut state = (0u64, 1u64);
    std::iter::from_fn(move || {
        let r = state.0;
        state = (state.1, state.0 + state.1);
        Some(r)
    })
}

fn pick(desc: bool) -> Box<dyn Iterator<Item = i32>> {
    if desc {
        Box::new((1..=5).rev())
    } else {
        Box::new(1..=5)
    }
}

fn sum_all<I: IntoIterator<Item = i32>>(items: I) -> i32 {
    items.into_iter().sum()
}

fn joined<T: ToString>(items: impl Iterator<Item = T>) -> String {
    items.map(|t| t.to_string()).collect::<Vec<_>>().join(",")
}

fn main() {
    println!("{}", joined(collatz(6)));
    let longest = (1..30u64).max_by_key(|&n| collatz(n).count()).unwrap();
    println!("longest under 30: {} ({} steps)", longest, collatz(longest).count());
    println!("{}", joined(evens(11)));
    let v = vec![3, -1, 4];
    println!("{}", joined(squares_of(&v)));
    println!("{}", joined(fib().take(12)));
    println!("{:?}", fib().skip_while(|&x| x < 100).step_by(2).take(3).collect::<Vec<_>>());
    println!("{}", joined(pick(true)));
    println!("{}", joined(pick(false).map(|x| x * 10)));
    println!("{} {} {}", sum_all(vec![1, 2, 3]), sum_all([4, 5]), sum_all(1..=10));
    let mut c = collatz(7);
    let first: Vec<u64> = c.by_ref().take(4).collect();
    let rest = c.count();
    println!("{:?} then {} more", first, rest);
    let pairs: Vec<(u32, u64)> = evens(100).zip(fib()).skip(5).take(3).collect();
    println!("{:?}", pairs);
    let mut it = (1..=3).chain(7..9).peekable();
    while let Some(x) = it.next() {
        match it.peek() {
            Some(&n) if n == x + 1 => print!("{}-", x),
            Some(_) => print!("{} | ", x),
            None => println!("{}", x),
        }
    }
    let total: u64 = collatz(27).filter(|n| n % 2 == 1).take(10).sum();
    println!("odd sum {}", total);
    let windows: Vec<i32> = [1, 4, 9, 16, 25].windows(2).map(|w| w[1] - w[0]).collect();
    println!("{:?} last={:?}", windows, windows.last());
}
