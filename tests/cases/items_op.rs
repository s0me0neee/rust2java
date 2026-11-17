// Operator traits become methods (`add`, `neg`, `add_assign`).
use std::ops::{Add, AddAssign, Neg};

#[derive(Debug, Clone)]
struct V {
    x: f64,
    y: f64,
}

impl Add for V {
    type Output = V;

    fn add(self, o: V) -> V {
        V { x: self.x + o.x, y: self.y + o.y }
    }
}

impl Neg for V {
    type Output = V;

    fn neg(self) -> V {
        V { x: -self.x, y: -self.y }
    }
}

impl AddAssign for V {
    fn add_assign(&mut self, o: V) {
        self.x += o.x;
        self.y += o.y;
    }
}

fn main() {
    let a = V { x: 1.0, y: 2.0 };
    let b = V { x: 10.0, y: 20.0 };
    let c = a + b;
    println!("{:?}", c);
    println!("{:?}", -c.clone());
    let mut d = V { x: 0.5, y: 0.5 };
    d += c;
    println!("{:?}", d);
}
