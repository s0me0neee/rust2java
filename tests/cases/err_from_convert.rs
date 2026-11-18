// `Err(e)?` converts through a user `From` impl: `MyError.from(e)`.
use std::num::ParseIntError;

#[derive(Debug)]
enum MyError {
    Parse(ParseIntError),
    Neg(i32),
}

impl From<ParseIntError> for MyError {
    fn from(e: ParseIntError) -> Self {
        MyError::Parse(e)
    }
}

fn check(s: &str) -> Result<i32, MyError> {
    let n: i32 = s.trim().parse()?;
    if n < 0 {
        return Err(MyError::Neg(n));
    }
    Ok(n)
}

fn main() {
    println!("{:?}", check("12"));
    println!("{:?}", check("-3"));
    println!("{:?}", check("zz"));
}
