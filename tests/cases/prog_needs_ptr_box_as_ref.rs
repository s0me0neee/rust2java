// `Box::as_ref` on a boxed trait object is not lowered yet.
trait Shape {
    fn area(&self) -> i32;
}

struct Dot;

impl Shape for Dot {
    fn area(&self) -> i32 {
        1
    }
}

fn main() {
    let boxed: Box<dyn Shape> = Box::new(Dot);
    let shape: &dyn Shape = boxed.as_ref();
    println!("{}", shape.area());
}
