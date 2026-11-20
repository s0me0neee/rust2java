// Trait objects: Vec<Box<dyn Shape>>, default methods, &dyn params, generic struct holding a
// Box<dyn Fn>, HashMap of named closures, trait with associated fn constructors.
use std::collections::HashMap;
use std::f64::consts::PI;

trait Shape {
    fn area(&self) -> f64;
    fn name(&self) -> String;
    fn describe(&self) -> String {
        format!("{} with area {:.2}", self.name(), self.area())
    }
    fn scale(&mut self, k: f64);
}

struct Circle {
    r: f64,
}
struct Poly {
    sides: u32,
    len: f64,
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        PI * self.r * self.r
    }
    fn name(&self) -> String {
        "circle".into()
    }
    fn scale(&mut self, k: f64) {
        self.r *= k;
    }
}

impl Shape for Poly {
    fn area(&self) -> f64 {
        let n = self.sides as f64;
        n * self.len * self.len / (4.0 * (PI / n).tan())
    }
    fn name(&self) -> String {
        match self.sides {
            3 => "triangle".to_string(),
            4 => "square".to_string(),
            n => format!("{}-gon", n),
        }
    }
    fn describe(&self) -> String {
        format!("[{}] side {}", self.name(), self.len)
    }
    fn scale(&mut self, k: f64) {
        self.len *= k;
    }
}

fn largest<'a>(shapes: &'a [Box<dyn Shape>]) -> Option<&'a dyn Shape> {
    shapes
        .iter()
        .map(|b| b.as_ref())
        .max_by(|a, b| a.area().total_cmp(&b.area()))
}

fn total(shapes: &[&dyn Shape]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

struct Pipeline<T> {
    stages: Vec<(String, Box<dyn Fn(T) -> T>)>,
}

impl<T: Copy + std::fmt::Debug> Pipeline<T> {
    fn new() -> Self {
        Pipeline { stages: Vec::new() }
    }
    fn stage(mut self, name: &str, f: impl Fn(T) -> T + 'static) -> Self {
        self.stages.push((name.to_string(), Box::new(f)));
        self
    }
    fn run(&self, x: T) -> T {
        let mut v = x;
        for (name, f) in &self.stages {
            let next = f(v);
            println!("  {}: {:?} -> {:?}", name, v, next);
            v = next;
        }
        v
    }
}

fn main() {
    let mut shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Circle { r: 1.5 }),
        Box::new(Poly { sides: 4, len: 2.0 }),
        Box::new(Poly { sides: 6, len: 1.0 }),
        Box::new(Poly { sides: 3, len: 3.0 }),
    ];
    for s in &shapes {
        println!("{}", s.describe());
    }
    if let Some(big) = largest(&shapes) {
        println!("largest: {}", big.name());
    }
    for s in shapes.iter_mut() {
        s.scale(2.0);
    }
    let names: Vec<String> = shapes.iter().map(|s| s.name()).collect();
    println!("{:?}", names);
    let c = Circle { r: 1.0 };
    let p = Poly { sides: 4, len: 1.0 };
    println!("total {:.3}", total(&[&c, &p]));
    shapes.retain(|s| s.area() < 20.0);
    println!("kept {}", shapes.len());

    let offset = 10;
    let pipe = Pipeline::new()
        .stage("double", |x: i64| x * 2)
        .stage("offset", move |x| x + offset)
        .stage("square", |x| x * x);
    println!("result {}", pipe.run(3));

    let mut ops: HashMap<&str, Box<dyn Fn(f64, f64) -> f64>> = HashMap::new();
    ops.insert("add", Box::new(|a, b| a + b));
    ops.insert("hyp", Box::new(|a: f64, b: f64| (a * a + b * b).sqrt()));
    ops.insert("pow", Box::new(f64::powf));
    let mut keys: Vec<_> = ops.keys().copied().collect();
    keys.sort_unstable();
    for k in keys {
        println!("{} -> {}", k, ops[k](3.0, 4.0));
    }
    let f = ops.get("hyp").unwrap();
    println!("{}", f(5.0, 12.0));
}
