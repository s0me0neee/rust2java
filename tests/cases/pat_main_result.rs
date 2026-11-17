// `fn main` returning Result: Ok exits 0, Err exits 1.
fn step(n: i32) -> Result<i32, String> {
    if n > 0 {
        Ok(n * 2)
    } else {
        Err(format!("bad step {}", n))
    }
}

fn main() -> Result<(), String> {
    let a = step(2)?;
    let b = step(3)?;
    println!("{} {}", a, b);
    let n = step(-1)?;
    println!("not reached {}", n);
    Ok(())
}
