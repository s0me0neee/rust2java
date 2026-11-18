// FnMut fields, where-clause bounds, &dyn Fn, and captures.
struct Accumulator {
    add: Box<dyn FnMut(i32) -> i32>,
    total: i32,
}

impl Accumulator {
    fn feed(&mut self, x: i32) {
        let v = (self.add)(x);
        self.total += v;
    }
}

fn combine<F>(f: F, a: i32, b: i32) -> i32
where
    F: Fn(i32, i32) -> i32,
{
    f(a, b)
}

fn both<F: Fn(i32) -> i32, G: Fn(i32) -> i32>(f: F, g: G) -> i32 {
    g(f(1))
}

fn apply_ref(f: &dyn Fn(i32) -> i32, x: i32) -> i32 {
    f(x)
}

fn main() {
    let mut acc = Accumulator { add: Box::new(|x| x * 2), total: 0 };
    acc.feed(3);
    acc.feed(4);
    println!("{}", acc.total);

    println!("{}", combine(|a, b| a + b * 3, 1, 2));
    println!("{}", apply_ref(&|x| x + 100, 5));
    println!("{}", both(|x| x + 2, |x| x * 3));

    let mut seen: Vec<i32> = Vec::new();
    {
        let mut log = |x: i32| {
            seen.push(x);
            x * 10
        };
        println!("{} {}", log(1), log(2));
    }
    println!("{:?}", seen);

    let mut counter = 0;
    let mut bump = move || {
        counter += 1;
        counter
    };
    println!("{} {} {}", bump(), bump(), counter);
}
