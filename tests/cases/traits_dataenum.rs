// Display for a data enum (sealed interface) and derive(PartialOrd/Ord) on data enums.
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Low,
    Mid,
    High,
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    Point,
    Circle(i32),
    Rect { w: i32, h: i32 },
}

impl fmt::Display for Shape {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shape::Point => write!(f, "point"),
            Shape::Circle(r) => write!(f, "circle {r}"),
            Shape::Rect { w, h } => write!(f, "rect {w}x{h}"),
        }
    }
}

fn main() {
    let mut levels = vec![Level::High, Level::Low, Level::Mid];
    levels.sort();
    println!("{:?} {}", levels, levels[0] < levels[2]);
    println!("{}", Level::Low as i32);
    let mut shapes = vec![
        Shape::Rect { w: 2, h: 1 },
        Shape::Circle(3),
        Shape::Point,
        Shape::Circle(1),
    ];
    shapes.sort();
    for s in &shapes {
        println!("{}", s);
    }
}
