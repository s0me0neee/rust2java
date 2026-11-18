// char::to_digit: an unwrapped digit hoists once and throws, the Option stays when used as one.
fn main() {
    let c = '7';
    println!("{}", c.to_digit(10).unwrap());
    println!("{}", 'f'.to_digit(16).expect("hex digit"));
    let radix = 16;
    println!("{}", 'b'.to_digit(radix).unwrap());
    println!("{:?} {:?}", 'a'.to_digit(10), c.to_digit(10));
    let digits: Vec<u32> = "1aZ".chars().filter_map(|c| c.to_digit(16)).collect();
    println!("{:?}", digits);
}
