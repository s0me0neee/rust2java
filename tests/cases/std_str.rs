// String, char and slicing methods (std_map).
fn main() {
    let s = String::from("  Hello, World  ");
    println!("{} {}", s.len(), s.is_empty());
    println!("[{}] [{}] [{}]", s.trim(), s.trim_start(), s.trim_end());
    println!("{}", s.to_uppercase());
    println!("{}", s.to_lowercase());
    println!("{} {}", s.contains("World"), s.contains("xyz"));
    println!("{} {} {}", s.starts_with("  He"), s.ends_with("  "), s.starts_with(' '));
    println!("{:?} {:?}", s.find("World"), s.find("zzz"));
    println!("{}", s.replace("World", "Rust"));
    println!("{}", "ab".repeat(3));
    println!("{}", s.as_str().len());

    let mut t = String::with_capacity(8);
    t.push_str("abc");
    t.push('d');
    println!("{} {}", t, t.len());
    t.insert(1, 'X');
    println!("{}", t);
    println!("{:?}", t.pop());
    println!("{}", t);
    t.truncate(2);
    println!("{}", t);
    t.clear();
    println!("{} {}", t.is_empty(), t.len());

    let n: i32 = "42".parse().unwrap();
    let f: f64 = "2.5".parse().unwrap();
    println!("{} {}", n + 1, f * 2.0);
    println!("{:?} {:?}", "12x".parse::<i32>().is_err(), "12x".parse::<i32>().ok());
    println!("{:?} {:?}", "200".parse::<u8>().ok(), "-3".parse::<i64>().ok());
    let big: i64 = "9000000000".parse().unwrap();
    println!("{}", big + 1);

    let s2 = String::from("abcdef");
    println!("{} {} {}", &s2[1..3], &s2[3..], &s2[..2]);
    println!("{}", s2.get(1..3).is_some());
    let v = vec![10, 20, 30, 40];
    println!("{:?}", &v[1..3]);
    println!("{:?}", &v[2..]);

    let c = 'A';
    println!("{} {} {} {}", c.is_alphabetic(), c.is_ascii_digit(), c.is_uppercase(), c.to_ascii_lowercase());
    println!("{:?} {:?}", '7'.to_digit(10), 'z'.to_digit(10));
    println!("{:?} {:?}", char::from_digit(11, 16), char::from_u32(65));
    println!("{} {} {}", '7'.is_ascii_digit(), 'g'.is_ascii_hexdigit(), ' '.is_ascii_whitespace());
    println!("{} {}", c.to_ascii_lowercase(), c.to_string());

    let parts: Vec<&str> = "a,b,,c".split(',').collect();
    println!("{} {}", parts.len(), parts[2].is_empty());
    let ws: Vec<&str> = " a  b ".split_whitespace().collect();
    println!("{:?}", ws);
    println!("{}", "one\ntwo".lines().count());
    println!("{:?}", s2.chars().rev().collect::<Vec<char>>());
}
