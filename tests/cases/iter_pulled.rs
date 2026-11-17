fn main() {
    let v = vec![10, 20, 30];
    let mut it = v.iter();
    println!("{:?}", it.next());
    println!("{:?}", it.next());
    let mut chars = "ab1".chars().peekable();
    println!("{:?}", chars.peek());
    println!("{:?}", chars.next_if_eq(&'a'));
    println!("{:?}", chars.next_if(|c| *c == 'z'));
    for c in chars {
        print!("{c}");
    }
    println!();
}
