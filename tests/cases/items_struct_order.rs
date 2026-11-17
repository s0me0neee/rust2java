// Struct literal field order (side effects), `..base` and constructors as values.
#[derive(Debug)]
struct Log {
    a: i32,
    b: i32,
    c: i32,
}

#[derive(Debug)]
struct Pair(i32, i32);

impl Pair {
    fn swapped(&self) -> Pair {
        Pair(self.1, self.0)
    }
}

fn tick(n: i32) -> i32 {
    println!("tick {}", n);
    n
}

fn make_base() -> Log {
    println!("base");
    Log { a: 1, b: 2, c: 3 }
}

fn main() {
    let base = make_base();
    let x = Log { c: tick(3), a: tick(1), b: tick(2) };
    println!("{:?}", x);
    let y = Log { b: tick(20), ..base };
    println!("{:?}", y);
    let y2 = Log { b: tick(30), ..make_base() };
    println!("{:?}", y2);
    let p = Pair(4, 5);
    let q = p.swapped();
    println!("{:?} {:?}", p, q);
    let f: fn(i32, i32) -> Pair = Pair;
    let r = f(6, 7);
    println!("{:?}", r);
}
