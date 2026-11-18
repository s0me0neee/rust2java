// Closures as values: Box<dyn Fn> in collections, impl Fn, F: Fn bounds, fn pointers.
struct Handler {
    f: Box<dyn Fn(i32) -> i32>,
}

impl Handler {
    fn call(&self, x: i32) -> i32 {
        (self.f)(x)
    }
}

fn make_adder(n: i32) -> impl Fn(i32) -> i32 {
    move |x| x + n
}

fn apply<F: Fn(i32) -> i32>(f: F, v: i32) -> i32 {
    f(v)
}

fn apply_mut<F: FnMut(i32) -> i32>(mut f: F) -> i32 {
    let a = f(1);
    let b = f(2);
    a + b
}

fn twice(f: fn(i32) -> i32, x: i32) -> i32 {
    f(f(x))
}

fn add_one(x: i32) -> i32 {
    x + 1
}

fn main() {
    let fns: Vec<Box<dyn Fn(i32) -> i32>> = vec![Box::new(|x| x + 1), Box::new(|x| x * 10)];
    println!("{:?}", fns.iter().map(|f| f(2)).collect::<Vec<_>>());
    let add = make_adder(5);
    println!("{} {}", add(1), apply(|x| x - 1, 10));
    println!("{}", twice(add_one, 4));

    let mut total = 0;
    {
        let mut acc = |x: i32| total += x;
        acc(3);
        acc(4);
    }
    println!("{total}");
    println!("{}", apply_mut(|x| x * 2));

    let mut count = 0;
    let mut tick = || {
        count += 1;
        count
    };
    println!("{} {}", tick(), tick());
    println!("{count}");

    let h = Handler { f: Box::new(|x| x * x) };
    println!("{} {}", h.call(3), (h.f)(4));

    let named = add_one;
    println!("{}", named(1));
    let boxed: Box<dyn Fn(i32) -> i32> = Box::new(add_one);
    println!("{}", boxed(9));
}
