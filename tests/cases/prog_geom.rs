// Operator overloading, trait objects, generics over a trait, iterator impls.
use std::fmt;
use std::ops::{Add, AddAssign, Index, IndexMut, Mul, Neg, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct V2 {
    x: f64,
    y: f64,
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2 { x: self.x + o.x, y: self.y + o.y }
    }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 {
        V2 { x: self.x - o.x, y: self.y - o.y }
    }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 {
        V2 { x: self.x * k, y: self.y * k }
    }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 {
        V2 { x: -self.x, y: -self.y }
    }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) {
        self.x += o.x;
        self.y += o.y;
    }
}
impl fmt::Display for V2 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({:.2}, {:.2})", self.x, self.y)
    }
}

impl V2 {
    fn dot(self, o: V2) -> f64 {
        self.x * o.x + self.y * o.y
    }
    fn cross(self, o: V2) -> f64 {
        self.x * o.y - self.y * o.x
    }
    fn len(self) -> f64 {
        self.dot(self).sqrt()
    }
}

trait Shape {
    fn area(&self) -> f64;
    fn perimeter(&self) -> f64;
    fn label(&self) -> &str;
}

struct Poly {
    pts: Vec<V2>,
}
struct Circle {
    c: V2,
    r: f64,
}

impl Shape for Poly {
    fn area(&self) -> f64 {
        let n = self.pts.len();
        (0..n).map(|i| self.pts[i].cross(self.pts[(i + 1) % n])).sum::<f64>().abs() / 2.0
    }
    fn perimeter(&self) -> f64 {
        self.pts.windows(2).map(|w| (w[1] - w[0]).len()).sum::<f64>()
            + (self.pts[0] - *self.pts.last().unwrap()).len()
    }
    fn label(&self) -> &str {
        "poly"
    }
}

impl Shape for Circle {
    fn area(&self) -> f64 {
        std::f64::consts::PI * self.r * self.r
    }
    fn perimeter(&self) -> f64 {
        2.0 * std::f64::consts::PI * self.r
    }
    fn label(&self) -> &str {
        "circle"
    }
}

struct Grid<T> {
    w: usize,
    cells: Vec<T>,
}

impl<T: Clone> Grid<T> {
    fn new(w: usize, h: usize, fill: T) -> Self {
        Grid { w, cells: vec![fill; w * h] }
    }
}

impl<T> Index<(usize, usize)> for Grid<T> {
    type Output = T;
    fn index(&self, (r, c): (usize, usize)) -> &T {
        &self.cells[r * self.w + c]
    }
}
impl<T> IndexMut<(usize, usize)> for Grid<T> {
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut T {
        &mut self.cells[r * self.w + c]
    }
}

struct Fib {
    a: u64,
    b: u64,
}
impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        let r = self.a;
        self.a = self.b;
        self.b += r;
        Some(r)
    }
}

fn total_area(shapes: &[Box<dyn Shape>]) -> f64 {
    shapes.iter().map(|s| s.area()).sum()
}

fn main() {
    let a = V2 { x: 1.0, y: 2.0 };
    let b = V2 { x: 3.5, y: -1.0 };
    let mut c = a + b * 2.0 - -a;
    c += V2 { x: 0.25, ..Default::default() };
    println!("{} {} {:.3} {}", c, a.dot(b), (a - b).len(), a == V2 { x: 1.0, y: 2.0 });
    println!("{:?}", V2::default());

    let shapes: Vec<Box<dyn Shape>> = vec![
        Box::new(Poly { pts: vec![V2 { x: 0.0, y: 0.0 }, V2 { x: 4.0, y: 0.0 }, V2 { x: 4.0, y: 3.0 }] }),
        Box::new(Circle { c: a, r: 1.5 }),
    ];
    for s in shapes.iter() {
        println!("{:>8}: area={:>8.3} perim={:<8.3}|", s.label(), s.area(), s.perimeter());
    }
    println!("total {:.4}", total_area(&shapes));
    let biggest = shapes
        .iter()
        .max_by(|x, y| x.area().partial_cmp(&y.area()).unwrap())
        .unwrap();
    println!("biggest {}", biggest.label());

    let mut g: Grid<i32> = Grid::new(3, 2, 0);
    g[(1, 2)] = 7;
    g[(0, 0)] += 2;
    for r in 0..2 {
        let row: Vec<String> = (0..3).map(|c| g[(r, c)].to_string()).collect();
        println!("{}", row.join(","));
    }

    let fibs: Vec<u64> = Fib { a: 0, b: 1 }.skip(5).step_by(2).take(5).collect();
    println!("{:?}", fibs);
    let even_sum: u64 = Fib { a: 0, b: 1 }.take_while(|&x| x < 4_000_000).filter(|x| x % 2 == 0).sum();
    println!("{}", even_sum);
    let mut f = Fib { a: 0, b: 1 };
    let tenth = f.nth(10);
    println!("{:?} {:?}", tenth, f.next());
    let circle = Circle { c: b, r: 2.0 };
    println!("{} {:.1}", circle.c, circle.area());
}
