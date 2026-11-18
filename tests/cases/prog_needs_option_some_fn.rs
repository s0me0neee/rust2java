// `Some` used as a function value is not lowered yet.
fn main() {
    let n = "5".parse::<i32>().map(Some);
    println!("{:?}", n);
    let opt = Some(2);
    println!("{:?}", opt.map(Some));
}
