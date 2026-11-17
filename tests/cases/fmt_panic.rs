fn main() {
    let n = if true { 3 } else { panic!() };
    println!("{}", n);
    println!("before");
    panic!("failure {}", 7);
}
