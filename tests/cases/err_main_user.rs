// A user error enum in `main`'s Result: `Error: <Debug>` and exit 1.
#[derive(Debug)]
enum Fail {
    Boom,
}

fn main() -> Result<(), Fail> {
    println!("start");
    Err(Fail::Boom)
}
