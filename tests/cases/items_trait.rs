// Traits -> interfaces; impls add `implements`; foreign impls become static methods.
trait HasSides {
    const DEFAULT_SIDES: i32 = 4;
    const SIDES: i32;
}

trait Shape {
    fn area(&self) -> f64;

    fn name(&self) -> String {
        String::from("shape")
    }
}

struct Circle {
    r: f64,
}

impl HasSides for Circle {
    const SIDES: i32 = 1;
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        3.0 * self.r * self.r
    }
}

struct Square {
    s: f64,
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.s * self.s
    }

    fn name(&self) -> String {
        String::from("square")
    }
}

impl Shape for i32 {
    fn area(&self) -> f64 {
        *self as f64
    }
}

trait Sum {
    fn total(&self) -> i32;
}

impl Sum for Vec<i32> {
    fn total(&self) -> i32 {
        let mut s = 0;
        for x in self {
            s += x;
        }
        s
    }
}

fn describe(s: &dyn Shape) -> String {
    format!("{} {}", s.name(), s.area())
}

fn main() {
    let c = Circle { r: 2.0 };
    let q = Square { s: 3.0 };
    println!("{}", describe(&c));
    println!("{}", describe(&q));
    println!("{}", c.area());
    let five: i32 = 5;
    println!("{}", five.area());
    println!("{}", Shape::area(&7));
    println!("{} {}", Circle::SIDES, <Circle as HasSides>::DEFAULT_SIDES);
    let z: &dyn Shape = &q;
    println!("{}", z.name());
    let mut v: Vec<i32> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println!("{}", v.total());
}
