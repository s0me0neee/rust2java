// An inherent `name()` on a fieldless enum collides with Java's final `Enum.name()`.
enum Color {
    Red,
    Green,
}

impl Color {
    fn name(&self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Green => "green",
        }
    }
}

fn main() {
    println!("{}", Color::Green.name());
}
