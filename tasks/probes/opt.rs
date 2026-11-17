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
fn parse_pos(s: &str) -> Result<i32, AppError> {
    let n: i32 = s.trim().parse()?;
    if n < 0 {
        return Err(AppError::Neg(n));
    }
    Ok(n)
}
fn half(n: i32) -> Option<i32> {
    if n % 2 == 0 { Some(n / 2) } else { None }
}
fn first_even(v: &[i32]) -> Option<i32> {
    let x = v.iter().find(|&&x| x % 2 == 0)?;
    Some(*x * 100)
}
fn main() {
    let o = Some(4);
    println!("{:?} {:?}", o.map(|x| x + 1), o.and_then(half));
    println!("{:?}", o.and_then(half).and_then(half).and_then(half));
    println!("{} {}", o.unwrap_or(0), None.unwrap_or(7));
    println!("{}", o.map_or(0, |x| x * 3));
    println!("{:?}", o.filter(|&x| x > 10));
    println!("{:?}", o.or(Some(9)));
    println!("{:?}", None.or_else(|| Some(1)));
    println!("{:?}", o.xor(None::<i32>));
    println!("{} {}", o.is_some(), o.is_none());
    println!("{}", o.is_some_and(|x| x > 3));
    println!("{:?}", o.zip(Some('a')));
    println!("{:?}", o.ok_or("none"));
    println!("{}", o.unwrap_or_default());
    let mut t = Some(3);
    let taken = t.take();
    println!("{:?} {:?}", taken, t);
    let mut r = Some(1);
    let old = r.replace(5);
    println!("{:?} {:?}", old, r);
    if let Some(ref mut x) = r {
        *x += 1;
    }
    println!("{:?}", r);
    let got = t.get_or_insert(8);
    *got += 1;
    println!("{:?}", t);
    println!("{:?}", first_even(&[1, 3, 4]));
    println!("{:?}", first_even(&[1, 3]));
    println!("{:?}", parse_pos("12"));
    match parse_pos("-4") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
    match parse_pos("zz") {
        Ok(n) => println!("ok {}", n),
        Err(e) => println!("error: {}", e),
    }
    let res: Result<i32, String> = Ok(3);
    println!("{:?}", res.as_ref().map(|x| x * 2));
    println!("{:?}", res.clone().map_err(|e| e.len()));
    println!("{}", res.clone().unwrap_or_else(|e| e.len() as i32));
    println!("{:?}", res.clone().ok());
    println!("{}", res.is_ok());
    let bad: Result<i32, String> = Err("boom".into());
    println!("{:?} {:?}", bad.clone().err(), bad.clone().unwrap_or_default());
    println!("{}", bad.clone().is_err_and(|e| e.len() == 4));
    let all: Result<Vec<i32>, _> = "1 2 3".split(' ').map(|s| s.parse::<i32>()).collect();
    println!("{:?}", all);
    let opts: Option<Vec<i32>> = vec![Some(1), Some(2)].into_iter().collect();
    println!("{:?}", opts);
    let nums: Vec<i32> = vec!["1", "x", "3"].iter().filter_map(|s| s.parse().ok()).collect();
    println!("{:?}", nums);
    let flat: Vec<i32> = vec![Some(1), None, Some(3)].into_iter().flatten().collect();
    println!("{:?}", flat);
    let e: Box<dyn std::error::Error> = "oops".into();
    println!("{}", e);
    println!("{}", o.expect("present"));
    let v: Option<&str> = Some("hi");
    println!("{:?}", v.map(|s| s.len()));
    println!("{}", v.unwrap_or_else(|| "none"));
    println!("{:?}", v.copied_test());
}
trait CopiedTest { fn copied_test(&self) -> usize; }
impl CopiedTest for Option<&str> { fn copied_test(&self) -> usize { self.map(|s| s.len()).unwrap_or(0) } }
