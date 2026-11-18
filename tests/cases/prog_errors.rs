// Error handling: custom error enum with From impls, ? across types, Box<dyn Error>, map_err, and_then chains.
use std::error::Error;
use std::fmt;
use std::num::ParseIntError;

#[derive(Debug)]
enum AppError {
    Parse(ParseIntError),
    Range { value: i64, min: i64, max: i64 },
    Empty,
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::Parse(e) => write!(f, "parse error: {}", e),
            AppError::Range { value, min, max } => write!(f, "{} not in {}..={}", value, min, max),
            AppError::Empty => write!(f, "empty input"),
        }
    }
}

impl Error for AppError {}

impl From<ParseIntError> for AppError {
    fn from(e: ParseIntError) -> Self {
        AppError::Parse(e)
    }
}

fn parse_in_range(s: &str, min: i64, max: i64) -> Result<i64, AppError> {
    let s = s.trim();
    if s.is_empty() {
        return Err(AppError::Empty);
    }
    let value: i64 = s.parse()?;
    if value < min || value > max {
        return Err(AppError::Range { value, min, max });
    }
    Ok(value)
}

fn sum_all(items: &[&str]) -> Result<i64, Box<dyn Error>> {
    let mut total = 0;
    for it in items {
        total += parse_in_range(it, -100, 100)?;
    }
    if total == 13 {
        return Err("unlucky total".into());
    }
    Ok(total)
}

fn first_even(items: &[&str]) -> Option<i64> {
    items.iter().filter_map(|s| s.parse::<i64>().ok()).find(|n| n % 2 == 0)
}

fn halve(n: i64) -> Result<i64, String> {
    if n % 2 == 0 { Ok(n / 2) } else { Err(format!("{} is odd", n)) }
}

fn main() {
    for s in ["42", " 7 ", "", "abc", "500", "-100", "99999999999999999999"] {
        match parse_in_range(s, -100, 100) {
            Ok(v) => println!("{:?} ok {}", s, v),
            Err(e) => println!("{:?} err {} / {:?}", s, e, matches!(e, AppError::Parse(_))),
        }
    }
    println!("{:?}", sum_all(&["1", "2", "3"]).ok());
    match sum_all(&["10", "3"]) {
        Ok(_) => {}
        Err(e) => println!("error: {}", e),
    }
    if let Err(e) = sum_all(&["1", "x"]) {
        println!("error: {}", e);
    }
    println!("{:?} {:?}", first_even(&["a", "3", "8", "10"]), first_even(&[]));
    let chain = halve(40).and_then(halve).and_then(halve).and_then(halve);
    println!("{:?} {:?}", chain, halve(12).map(|x| x * 10).unwrap_or(-1));
    let results: Result<Vec<i64>, String> = [8, 4, 2].iter().map(|&n| halve(n)).collect();
    let bad: Result<Vec<i64>, String> = [8, 3, 2].iter().map(|&n| halve(n)).collect();
    println!("{:?} {:?}", results, bad);
    let opts: Option<Vec<u32>> = ["1", "2", "3"].iter().map(|s| s.parse().ok()).collect();
    println!("{:?}", opts);
    let n: i64 = "12".parse::<i64>().map_err(|e| e.to_string()).and_then(halve).unwrap_or_default();
    let e = "x".parse::<u8>().unwrap_err();
    println!("{} {} {:?}", n, e, "300".parse::<u8>().is_err());
    let total: i64 = ["5", "x", "7"].iter().flat_map(|s| s.parse::<i64>()).sum();
    println!("{}", total);
    let v = parse_in_range("x", 0, 1).unwrap_or_else(|err| {
        println!("fallback because {}", err);
        0
    });
    println!("{}", v);
    let r: Result<i64, AppError> = Err(AppError::Empty);
    println!("{}", r.expect("needs a value"));
}
