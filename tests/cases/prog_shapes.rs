// Trait objects: a Shape trait with default methods, several implementors kept in
// a Vec<Box<dyn Shape>>, dynamic dispatch, sorting and a Display summary.
use std::fmt;

trait Shape {
    fn name(&self) -> &'static str;
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;

    fn describe(&self) -> String {
        format!(
            "{:<9} area {:>8.2} perimeter {:>8.2}",
            self.name(),
            self.area(),
            self.perimeter()
        )
    }

    fn is_round(&self) -> bool {
        false
    }
}

struct Circle {
    radius: f64,
}

impl Circle {
    fn new(radius: f64) -> Circle {
        Circle { radius }
    }
}

impl Shape for Circle {
    fn name(&self) -> &'static str {
        "circle"
    }

    fn area(&self) -> f64 {
        std::f64::consts::PI * self.radius * self.radius
    }

    fn perimeter(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.radius
    }

    fn is_round(&self) -> bool {
        true
    }
}

impl fmt::Display for Circle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Circle(r = {:.1})", self.radius)
    }
}

struct Rectangle {
    width: f64,
    height: f64,
    label: String,
}

impl Shape for Rectangle {
    fn name(&self) -> &'static str {
        "rectangle"
    }

    fn area(&self) -> f64 {
        self.width * self.height
    }

    fn perimeter(&self) -> f64 {
        2.0 * (self.width + self.height)
    }
}

impl fmt::Display for Rectangle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rectangle({} x {})", self.width, self.height)
    }
}

struct Triangle {
    a: f64,
    b: f64,
    c: f64,
}

impl Triangle {
    fn new(a: f64, b: f64, c: f64) -> Option<Triangle> {
        if a + b <= c || b + c <= a || a + c <= b {
            None
        } else {
            Some(Triangle { a, b, c })
        }
    }
}

impl Shape for Triangle {
    fn name(&self) -> &'static str {
        "triangle"
    }

    fn area(&self) -> f64 {
        let s = self.perimeter() / 2.0;
        (s * (s - self.a) * (s - self.b) * (s - self.c)).sqrt()
    }

    fn perimeter(&self) -> f64 {
        self.a + self.b + self.c
    }
}

fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|shape| shape.area()).sum()
}

fn largest(shapes: &[Box<dyn Shape>]) -> Option<&dyn Shape> {
    let mut best: Option<&dyn Shape> = None;
    for shape in shapes {
        if best.is_none() || shape.area() > best.unwrap().area() {
            best = Some(&**shape);
        }
    }
    best
}

fn count_round(shapes: &[Box<dyn Shape>]) -> usize {
    shapes.iter().filter(|shape| shape.is_round()).count()
}

fn main() {
    let mut shapes: Vec<Box<dyn Shape>> = Vec::new();
    shapes.push(Box::new(Circle::new(1.0)));
    shapes.push(Box::new(Circle::new(2.5)));
    shapes.push(Box::new(Rectangle {
        width: 3.0,
        height: 4.0,
        label: "table".to_string(),
    }));
    shapes.push(Box::new(Rectangle {
        width: 1.5,
        height: 10.0,
        label: "plank".to_string(),
    }));
    match Triangle::new(3.0, 4.0, 5.0) {
        Some(triangle) => shapes.push(Box::new(triangle)),
        None => println!("not a triangle"),
    }
    if let Some(bad) = Triangle::new(1.0, 2.0, 9.0) {
        shapes.push(Box::new(bad));
    } else {
        println!("rejected degenerate triangle");
    }
    for shape in &shapes {
        println!("{}", shape.describe());
    }
    println!("total area: {:.2}", total_area(&shapes));
    println!("round shapes: {}", count_round(&shapes));
    match largest(&shapes) {
        Some(shape) => println!("largest: {} ({:.2})", shape.name(), shape.area()),
        None => println!("no shapes"),
    }
    let mut sorted: Vec<&Box<dyn Shape>> = shapes.iter().collect();
    sorted.sort_by(|a, b| b.area().partial_cmp(&a.area()).unwrap());
    let ranking: Vec<String> = sorted
        .iter()
        .map(|shape| format!("{}({:.1})", shape.name(), shape.area()))
        .collect();
    println!("by area: {}", ranking.join(" > "));
    let ratios: Vec<f64> = shapes.iter().map(|shape| shape.area() / shape.perimeter()).collect();
    let mean = ratios.iter().sum::<f64>() / ratios.len() as f64;
    println!("mean area/perimeter: {:.3}", mean);
    let biggest_first = sorted.first().map(|shape| shape.name());
    println!("first in sorted: {:?}", biggest_first);
    let names: Vec<&str> = shapes.iter().map(|shape| shape.name()).collect();
    let kinds = names.iter().filter(|name| **name == "circle").count();
    println!("circles: {}", kinds);
    let circle = Circle::new(3.0);
    let rectangle = Rectangle {
        width: 2.0,
        height: 2.0,
        label: "square".to_string(),
    };
    println!("circle display: {}", circle);
    println!("rectangle display: {}", rectangle);
    let same_area = (circle.area() - rectangle.area()).abs() < 0.5;
    println!("similar area: {}", same_area);
    let values = vec![1.0, 2.5, 3.75];
    let total: f64 = values.iter().sum();
    println!("values total: {:.2}", total);
}
