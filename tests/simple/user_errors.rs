// A user error enum carried by an exception class; `?`, `match` and `unwrap_or` on its Results.
use std::fmt;

#[derive(Debug)]
enum AppError {
    Negative(i32),
    Empty,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Negative(n) => write!(f, "negative: {}", n),
            AppError::Empty => write!(f, "empty"),
        }
    }
}

fn parse(s: &str) -> Result<i32, AppError> {
    if s.is_empty() {
        return Err(AppError::Empty);
    }
    let n: i32 = s.trim().parse().unwrap();
    if n < 0 {
        return Err(AppError::Negative(n));
    }
    Ok(n)
}

fn total(xs: &[&str]) -> Result<i32, AppError> {
    let mut t = 0;
    for x in xs {
        t += parse(x)?;
    }
    Ok(t)
}

fn main() {
    println!("{}", total(&["1", "2"]).unwrap());

    match parse("-3") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
    let v = parse("").unwrap_or(0);
    if let Ok(n) = parse("5") {
        println!("five {}", n);
    }
    println!("{}", v);
}
