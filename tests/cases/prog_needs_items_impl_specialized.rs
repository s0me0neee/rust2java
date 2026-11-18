// `impl` for one instantiation of a generic type has no Java equivalent yet.
struct Wrap<T> {
    value: T,
}

impl Wrap<i32> {
    fn doubled(&self) -> i32 {
        self.value * 2
    }
}

fn main() {
    let w = Wrap { value: 21 };
    println!("{}", w.doubled());
}
