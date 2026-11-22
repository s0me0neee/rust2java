use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError { Parse(ParseIntError), Neg(i64), Empty }
impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Parse(e) => write!(f, "parse error: {}", e),
            AppError::Neg(n) => write!(f, "negative: {}", n),
            AppError::Empty => write!(f, "empty input"),
        }
    }
}
impl From<ParseIntError> for AppError { fn from(e: ParseIntError) -> Self { AppError::Parse(e) } }

fn parse_pos(s: &str) -> Result<i64, AppError> {
    let s = s.trim();
    if s.is_empty() { return Err(AppError::Empty); }
    let n: i64 = s.parse()?;
    if n < 0 { Err(AppError::Neg(n)) } else { Ok(n) }
}
fn sum_all(xs: &[&str]) -> Result<i64, AppError> {
    let mut t = 0;
    for x in xs { t += parse_pos(x)?; }
    Ok(t)
}
fn first_even(xs: &[i64]) -> Option<i64> { let x = xs.iter().find(|x| *x % 2 == 0)?; Some(x * 10) }
fn main() {
    for input in ["42", " 7 ", "-3", "", "abc", "99999999999999999999"] {
        match parse_pos(input) {
            Ok(n) => println!("ok {n}"),
            Err(e) => println!("err {e} / {:?}", matches!(e, AppError::Parse(_))),
        }
    }
    println!("{:?}", sum_all(&["1", "2", "3"]).ok());
    println!("{}", sum_all(&["1", "x"]).map_or_else(|e| e.to_string(), |v| v.to_string()));
    println!("{:?} {:?}", first_even(&[1, 3, 4]), first_even(&[1]));
    let rs: Result<Vec<i64>, _> = ["1", "2", "z"].iter().map(|s| s.parse::<i64>()).collect();
    println!("{}", rs.is_err());
    let good: Vec<i64> = ["5", "q", "6"].iter().filter_map(|s| s.parse().ok()).collect();
    println!("{good:?}");
    let v: Option<Vec<i64>> = [Some(1), Some(2)].into_iter().collect();
    println!("{v:?} {}", parse_pos("8").unwrap_or(0) + parse_pos("-1").unwrap_or(0));
}
