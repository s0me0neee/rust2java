// Structs: fields, canonical constructor, inherent impls, receivers, struct literals.
struct Point {
    x: i32,
    y: i32,
}

impl Point {
    fn new(x: i32, y: i32) -> Point {
        Point { x, y }
    }

    fn norm2(&self) -> i32 {
        self.x * self.x + self.y * self.y
    }

    fn shifted(&self, dx: i32) -> Point {
        Point { x: self.x + dx, y: self.y }
    }

    fn move_by(self, dx: i32) -> Point {
        Point { x: self.x + dx, y: self.y }
    }

    fn origin() -> Self {
        Self { x: 0, y: 0 }
    }

    fn dup(&self) -> Self {
        Self::new(self.x, self.y)
    }
}

struct Empty {}

struct Marker;

struct Pair(i32, i32);

impl Pair {
    fn sum(&self) -> i32 {
        self.0 + self.1
    }
}

fn main() {
    let p = Point::new(3, 4);
    println!("{}", p.norm2());
    let q = p.shifted(2);
    println!("{} {}", q.x, q.y);
    let r = Point { y: 10, x: 1 };
    println!("{} {}", r.x, r.y);
    let s = Point { ..q };
    println!("{} {}", s.x, s.y);
    let e = Empty {};
    let _ = e;
    let m = Marker;
    let _ = m;
    let t = Pair(7, 8);
    println!("{} {} {}", t.0, t.1, t.sum());
    println!("{}", r.move_by(5).y);
    let o = Point::origin();
    println!("{} {} {}", o.x, o.y, o.dup().y);
}
