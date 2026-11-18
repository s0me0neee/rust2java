// parse::<T>(): short path when the error value is never read; i64/u64 need no range check.
use std::num::{ParseFloatError, ParseIntError};

fn first_num(s: &str) -> Option<i32> {
    let n = s.split(' ').next()?.parse::<i32>().ok()?;
    Some(n * 2)
}

fn classify(s: &str) -> &'static str {
    match s.parse::<i32>() {
        Ok(n) if n > 10 => "big",
        Ok(_) => "small",
        Err(_) => "bad",
    }
}

fn main() {
    for s in ["42", "7", "x", "", "99999999999999999999"] {
        println!("{} {} {} {}", s, s.parse::<i32>().is_err(), s.parse::<i32>().map_err(|_| "oops".to_string()).unwrap_or(0), classify(s));
        if let Ok(n) = s.parse::<i64>() {
            println!("i64 {}", n);
        }
        let f: Result<f64, ParseFloatError> = s.parse();
        println!("{}", f.unwrap_or_else(|_| -0.5));
    }
    let nums: Vec<i32> = "1 x 3".split(' ').map(|w| w.parse().unwrap_or_else(|_| -1)).collect();
    println!("{:?}", nums);
    println!("{:?}", first_num("8 9"));
    println!("{:?}", first_num("nope"));
    let r: Result<i64, ParseIntError> = "9223372036854775807".parse();
    println!("{}", r.unwrap());
    let r2: Result<i64, ParseIntError> = "9223372036854775808".parse();
    println!("{:?}", r2);
    let r3: Result<u64, ParseIntError> = "18446744073709551615".parse();
    println!("{}", r3.is_ok());
    println!("{}", "18446744073709551615".parse::<u64>().unwrap());
    match u64::from_str_radix("18446744073709551615", 10) {
        Ok(v) => println!("radix {}", v),
        Err(_) => println!("radix err"),
    }
    match "9.5".parse::<f32>() {
        Ok(v) => println!("f32 {}", v),
        Err(_) => println!("f32 err"),
    }
    println!("{}", "abc".parse::<f64>().unwrap_or_else(|_| -0.5));
    println!("{:?}", "abc".parse::<f64>().ok());
    if let Ok(v) = "2.5e2".parse::<f64>() {
        println!("f64 {}", v);
    }
    println!("{}", "abc".parse::<f64>().map_err(|_| "bad".to_string()).unwrap_or(0.0));
    let e: Result<i32, ParseIntError> = "x".parse();
    match e {
        Ok(v) => println!("{}", v),
        Err(err) => println!("{} {:?}", err, err),
    }
}
