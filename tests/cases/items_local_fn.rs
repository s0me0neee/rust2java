// Nested `fn` items become private static methods of the class.
fn main() {
    fn sq(x: i32) -> i32 {
        x * x
    }

    fn fact(n: i32) -> i32 {
        if n <= 1 {
            1
        } else {
            n * fact(n - 1)
        }
    }

    println!("{} {}", sq(7), fact(5));
}
