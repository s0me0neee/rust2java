use std::f64::consts::PI;
use std::fmt;

trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn name(&self) -> String {
        "shape".to_string()
    }
    fn describe(&self) -> String {
        format!("{} area={:.2} perim={:.2}", self.name(), self.area(), self.perimeter())
    }
}

struct Circle(f64);
struct Rect {
    w: f64,
    h: f64,
}
struct Square {
    side: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        PI * self.0 * self.0
    }
    fn perimeter(&self) -> f64 {
        2.0 * PI * self.0
    }
    fn name(&self) -> String {
        format!("circle(r={})", self.0)
    }
}

impl Shape for Rect {
    fn area(&self) -> f64 {
        self.w * self.h
    }
    fn perimeter(&self) -> f64 {
        2.0 * (self.w + self.h)
    }
}

impl Shape for Square {
    fn area(&self) -> f64 {
        self.side.powi(2)
    }
    fn perimeter(&self) -> f64 {
        4.0 * self.side
    }
    fn name(&self) -> String {
        "square".into()
    }
}

impl fmt::Debug for dyn Shape + '_ {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "<{}>", self.name())
    }
}

fn largest<'a>(shapes: &'a [Box<dyn Shape>]) -> Option<&'a dyn Shape> {
    shapes.iter().map(|b| b.as_ref()).max_by(|a, b| a.area().total_cmp(&b.area()))
}

fn main() {
    let shapes: Vec<Box<dyn Shape>> = vec![Box::new(Circle(1.5)), Box::new(Rect { w: 3.0, h: 2.5 }), Box::new(Square { side: 2.0 })];
    for s in &shapes {
        println!("{}", s.describe());
    }
    let total: f64 = shapes.iter().map(|s| s.area()).sum();
    println!("total {:.3}", total);
    println!("{:?}", largest(&shapes));
}
