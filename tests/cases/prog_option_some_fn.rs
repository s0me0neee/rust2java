// `Some` as a function value is `Optional::of`.
fn main() {
    let n = "5".parse::<i32>().map(Some);
    println!("{:?}", n);
    let opt = Some(2);
    println!("{:?}", opt.map(Some));
}
