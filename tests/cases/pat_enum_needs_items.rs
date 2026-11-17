// Local enum/struct patterns; needs worker 1's item lowering.
enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
    Empty,
}

enum Color {
    Red,
    Green,
    Blue,
}

struct Point {
    x: i32,
    y: i32,
}

fn area(s: &Shape) -> f64 {
    match s {
        Shape::Circle(r) => 3.14 * r * r,
        Shape::Rect { w, h } => w * h,
        Shape::Empty => 0.0,
    }
}

fn name(c: Color) -> &'static str {
    match c {
        Color::Red => "red",
        Color::Green => "green",
        Color::Blue => "blue",
    }
}

fn sum(p: Point) -> i32 {
    match p {
        Point { x, y } => x + y,
    }
}

fn main() {
    println!("{}", area(&Shape::Circle(2.0)));
    println!("{}", area(&Shape::Rect { w: 2.0, h: 3.0 }));
    println!("{}", area(&Shape::Empty));
    println!("{}", name(Color::Red));
    println!("{}", name(Color::Blue));
    match Color::Green {
        Color::Green | Color::Blue => println!("cool"),
        Color::Red => println!("warm"),
    }
    match Shape::Circle(1.5) {
        Shape::Circle(r) if r > 1.0 => println!("big circle {}", r),
        Shape::Circle(r) => println!("small circle {}", r),
        _ => println!("other"),
    }
    println!("{}", sum(Point { x: 2, y: 5 }));
}
