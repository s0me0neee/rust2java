fn message() -> i32 {
    print!("message");
    7
}

fn main() {
    assert_eq!(1, 2, "value {}", message());
}
