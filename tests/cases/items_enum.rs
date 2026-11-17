// Enums: fieldless (with discriminants) as Java enums, data enums as sealed interfaces.
#[derive(Debug, Copy, Clone)]
enum Dir {
    North,
    East = 5,
    South,
}

#[derive(Debug, Clone, PartialEq)]
enum Shape {
    Point,
    Circle(f64),
    Rect { w: f64, h: f64 },
}

impl Shape {
    fn circle(r: f64) -> Shape {
        Shape::Circle(r)
    }
}

fn main() {
    let d = Dir::East;
    println!("{} {} {} {:?}", d as i32, Dir::North as i32, Dir::South as i32, d);
    let p = Shape::Point;
    let c = Shape::Circle(1.5);
    let r = Shape::Rect { w: 2.0, h: 3.0 };
    println!("{:?}", p);
    println!("{:?}", c);
    println!("{:?}", r);
    println!("{:?}", Shape::circle(4.0));
    let c2 = Shape::Circle(1.5);
    println!("{} {}", c == c2, c == r);
    println!("{:?}", Shape::Rect { h: 1.0, w: 0.5 });
}
