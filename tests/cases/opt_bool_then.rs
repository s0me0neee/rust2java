fn half(n: i32) -> Option<i32> { (n % 2 == 0).then(|| n / 2) }
fn main() {
    let v: Vec<i32> = (1..=6).collect();
    let evens: Vec<i32> = v.iter().filter_map(|&x| (x % 2 == 0).then_some(x * 10)).collect();
    println!("{:?} {:?} {:?}", evens, half(7), half(8));
    let flag = v.len() > 10;
    println!("{:?} {:?}", flag.then_some("big"), (!flag).then(|| format!("small {}", v.len())));
    let s = "[12]x";
    let n = s.starts_with('[').then(|| s[1..3].parse::<u32>().ok()).flatten();
    println!("{n:?}");
}
