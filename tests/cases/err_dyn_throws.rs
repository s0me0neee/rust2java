// `?` from a user error into `Box<dyn Error>`, a string error boxed with `.into()`, and `and_then`.
use std::error::Error;
use std::fmt;

#[derive(Debug)]
enum AppError {
    Neg(i64),
    TooBig(i64),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Neg(n) => write!(f, "negative: {}", n),
            AppError::TooBig(n) => write!(f, "too big: {}", n),
        }
    }
}

impl Error for AppError {}

fn check(s: &str) -> Result<i64, AppError> {
    let n: i64 = s.trim().parse().unwrap_or(0);
    if n < 0 {
        return Err(AppError::Neg(n));
    }
    Ok(n)
}

fn small(n: i64) -> Result<i64, AppError> {
    if n > 50 { Err(AppError::TooBig(n)) } else { Ok(n) }
}

fn total(items: &[&str]) -> Result<i64, Box<dyn Error>> {
    let mut t = 0;
    for it in items {
        t += check(it).and_then(small)?;
    }
    if t == 13 {
        return Err("unlucky total".into());
    }
    Ok(t)
}

fn main() {
    for items in [vec!["1", "2"], vec!["5", "-3"], vec!["6", "7"], vec!["60"]] {
        match total(&items) {
            Ok(t) => println!("total {t}"),
            Err(e) => println!("error: {e}"),
        }
    }
}
