// A type's Java body spans to its last impl; unrelated items move after the class.
struct A {
    x: i32,
}

fn helper(n: i32) -> i32 {
    n + 1
}

impl A {
    fn get(&self) -> i32 {
        helper(self.x)
    }
}

struct B {
    y: i32,
}

impl B {
    fn get(&self) -> i32 {
        self.y * 2
    }
}

fn main() {
    let a = A { x: 4 };
    let b = B { y: 5 };
    println!("{} {}", a.get(), b.get());
}
