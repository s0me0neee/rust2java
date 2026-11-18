// By-value consumers on an iterator that was already advanced with next(); parse().unwrap().
fn main() {
    let line = "add 3 milk eggs";
    let mut parts = line.split_whitespace();
    let cmd = parts.next().unwrap();
    let n: i32 = parts.next().unwrap().parse().unwrap();
    let rest: Vec<&str> = parts.collect();
    println!("{} {} {:?}", cmd, n, rest);

    let mut it = "a,b,c".split(',');
    it.next();
    println!("{}", it.count());

    let mut chars = "hello".chars();
    let first = chars.next().expect("empty");
    let tail: String = chars.collect();
    println!("{} {}", first, tail);

    let mut nums = "1 2 3 4".split(' ').map(|s| s.parse::<i64>().unwrap());
    let head = nums.next();
    let sum: i64 = nums.sum();
    println!("{:?} {}", head, sum);

    let mut words = vec!["x", "y", "z"].into_iter();
    while let Some(w) = words.next() {
        if w == "y" {
            break;
        }
    }
    let left: Vec<String> = words.map(|w| w.to_uppercase()).collect();
    println!("{:?}", left);

    let big: u64 = "18446744073709551615".parse().unwrap();
    let small: i8 = "-128".parse().expect("i8");
    let idx: usize = "+7".parse().unwrap();
    println!("{} {} {}", big, small, idx);
    let bad: u32 = "-1".parse().unwrap();
    println!("unreachable {}", bad);
}
