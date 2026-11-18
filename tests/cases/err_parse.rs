// ParseIntError / ParseFloatError Display and Debug match rustc.
use std::num::{ParseFloatError, ParseIntError};

fn main() {
    for s in ["zz", "", "99999999999999999999", "-99999999999999999999"] {
        let e: ParseIntError = s.parse::<i32>().unwrap_err();
        println!("{} | {:?}", e, e);
    }
    for s in ["zz", ""] {
        let e: ParseFloatError = s.parse::<f64>().unwrap_err();
        println!("{} | {:?}", e, e);
    }

    let good: Result<i32, ParseIntError> = "12".parse();
    println!("{:?}", good);
    let bad: Result<i32, ParseIntError> = "nope".parse();
    println!("{:?}", bad);
    let float: Result<f64, ParseFloatError> = "1.5".parse();
    println!("{:?}", float);
    println!("{:?}", "x".parse::<f64>());

    println!("{:?}", "256".parse::<u8>());
    println!("{:?}", "-1".parse::<u8>());
    println!("{:?}", "999".parse::<i8>());
    println!("{:?}", "-129".parse::<i8>());
    println!("{:?}", "127".parse::<i8>());

    println!("{:?}", i32::from_str_radix("ff", 16));
    println!("{:?}", i32::from_str_radix("zz", 16));
    println!("{:?}", u8::from_str_radix("ff", 16));
    println!("{:?}", u8::from_str_radix("100", 16));
}
