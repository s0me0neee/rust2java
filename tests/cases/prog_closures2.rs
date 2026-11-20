// Closures as values: returning impl Fn / Box<dyn Fn>, FnMut counters, function pointers in
// arrays, compose, memoization struct with HashMap cache, callbacks taking &mut state.
use std::collections::HashMap;

fn adder(k: i32) -> impl Fn(i32) -> i32 {
    move |x| x + k
}

fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |x| g(f(x))
}

fn make_counter() -> impl FnMut() -> u32 {
    let mut n = 0;
    move || {
        n += 1;
        n
    }
}

fn pick(op: char) -> Box<dyn Fn(f64, f64) -> f64> {
    match op {
        '+' => Box::new(|a, b| a + b),
        '-' => Box::new(|a, b| a - b),
        '*' => Box::new(|a, b| a * b),
        _ => Box::new(|a, _| a),
    }
}

fn double(x: i32) -> i32 {
    x * 2
}
fn square(x: i32) -> i32 {
    x * x
}

fn apply_all(fs: &[fn(i32) -> i32], x: i32) -> Vec<i32> {
    fs.iter().map(|f| f(x)).collect()
}

struct Memo<F: Fn(u64) -> u64> {
    f: F,
    cache: HashMap<u64, u64>,
    hits: u32,
}

impl<F: Fn(u64) -> u64> Memo<F> {
    fn new(f: F) -> Self {
        Memo { f, cache: HashMap::new(), hits: 0 }
    }
    fn get(&mut self, n: u64) -> u64 {
        if let Some(&v) = self.cache.get(&n) {
            self.hits += 1;
            return v;
        }
        let v = (self.f)(n);
        self.cache.insert(n, v);
        v
    }
}

fn repeat_apply<F>(mut f: F, times: usize)
where
    F: FnMut(usize),
{
    for i in 0..times {
        f(i);
    }
}

fn main() {
    let add5 = adder(5);
    let inc_then_double = compose(adder(1), double);
    let to_text = compose(square, |n: i32| format!("<{}>", n));
    println!("{} {} {}", add5(10), inc_then_double(4), to_text(7));
    let mut c1 = make_counter();
    let mut c2 = make_counter();
    println!("{} {} {} {}", c1(), c1(), c2(), c1());
    let ops = "+-*/";
    let results: Vec<String> = ops.chars().map(|o| format!("{}{}", o, pick(o)(6.0, 1.5))).collect();
    println!("{}", results.join(" "));
    let table: [fn(i32) -> i32; 3] = [double, square, |x| x - 1];
    println!("{:?}", apply_all(&table, 9));
    let slow_sq = |n: u64| (0..n).map(|_| n).sum::<u64>();
    let mut memo = Memo::new(slow_sq);
    let xs = [12, 7, 12, 3, 7, 12];
    let ys: Vec<u64> = xs.iter().map(|&x| memo.get(x)).collect();
    println!("{:?} hits={} cached={}", ys, memo.hits, memo.cache.len());
    let mut log = Vec::new();
    repeat_apply(|i| log.push(i * i), 4);
    println!("{:?}", log);
    let mut total = 0;
    let mut add_to = |x: i32| total += x;
    for x in [3, 4, 5] {
        add_to(x);
    }
    println!("total {}", total);
    let fs: Vec<Box<dyn Fn(i32) -> i32>> = (1..=3).map(|k| Box::new(move |x| x * k) as Box<dyn Fn(i32) -> i32>).collect();
    println!("{:?}", fs.iter().map(|f| f(10)).collect::<Vec<_>>());
    let pipeline: Vec<fn(i32) -> i32> = vec![double, square, double];
    println!("{}", pipeline.iter().fold(3, |acc, f| f(acc)));
    let mut sorted = vec!["bb", "a", "ccc", "dd"];
    let key = |s: &&str| (s.len(), s.to_string());
    sorted.sort_by_key(key);
    println!("{:?}", sorted);
}
