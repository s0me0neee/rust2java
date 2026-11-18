// The entry point prints `Error: <Debug>` for a boxed string error.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("start");
    let e: Box<dyn std::error::Error> = "oops".into();
    Err(e)
}
