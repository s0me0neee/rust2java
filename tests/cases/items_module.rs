// Inline modules become nested classes; cross-module references are qualified.
mod geometry {
    pub struct Point {
        pub x: i32,
        pub y: i32,
    }

    impl Point {
        pub fn new(x: i32, y: i32) -> Point {
            Point { x, y }
        }

        pub fn area(&self) -> i32 {
            self.x * self.y
        }
    }

    pub fn origin() -> Point {
        Point::new(0, 0)
    }

    pub fn diagonal(p: &Point) -> i32 {
        p.x + p.y
    }
}

mod util {
    pub fn double(n: i32) -> i32 {
        n * 2
    }
}

fn main() {
    let p = geometry::Point::new(3, 4);
    println!("{} {}", p.x, p.area());
    let o = geometry::origin();
    println!("{} {}", util::double(o.y), geometry::diagonal(&p));
}
