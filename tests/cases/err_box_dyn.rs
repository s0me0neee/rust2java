// `Box<dyn Error>`: string conversion, `?` into it, Display/Debug/to_string.
fn parse_one(s: &str) -> Result<i32, Box<dyn std::error::Error>> {
    let n: i32 = s.trim().parse()?;
    Ok(n)
}

fn inner() -> Result<(), String> {
    Err("inner".into())
}

fn outer() -> Result<(), Box<dyn std::error::Error>> {
    inner()?;
    Ok(())
}

fn main() {
    let e: Box<dyn std::error::Error> = "oops".into();
    println!("{}", e);
    println!("{}", e.to_string());
    println!("{:?}", e);
    println!("{:?}", parse_one("5"));
    println!("{:?}", parse_one("zz"));
    println!("{}", parse_one("7").unwrap());
    println!("{:?}", outer());
}
