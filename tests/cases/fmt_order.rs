fn value(n: i32) -> i32 {
    print!("v{}", n);
    n
}

fn main() {
    println!(" {1} {0} {1}", value(1), value(2));
    let empty = vec![value(3); 0];
    println!(" {:?}", empty);
    assert!(true, "{}", value(4));
    println!("done");
}
