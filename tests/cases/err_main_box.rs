// `main() -> Result<(), Box<dyn Error>>` failure: `Error: <Debug>` and exit 1.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("start");
    let n: i32 = "zz".parse()?;
    println!("{}", n);
    Ok(())
}
