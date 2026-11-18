// Needs w8: `Display` for a data enum (probe `opt.rs` error type). Run with NEEDS=1.
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Parse(ParseIntError),
    Neg(i32),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            AppError::Parse(e) => write!(f, "parse: {}", e),
            AppError::Neg(n) => write!(f, "negative: {}", n),
        }
    }
}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::Parse(e)
    }
}

impl std::error::Error for AppError {}

fn parse_pos(s: &str) -> Result<i32, AppError> {
    let n: i32 = s.trim().parse()?;
    if n < 0 {
        return Err(AppError::Neg(n));
    }
    Ok(n)
}

fn main() {
    match parse_pos("12") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
    match parse_pos("-4") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
    match parse_pos("zz") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
}
