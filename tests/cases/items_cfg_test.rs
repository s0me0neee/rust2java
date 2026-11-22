fn double(x: i32) -> i32 {
    x * 2
}

fn main() {
    println!("{}", double(21));
}

#[test]
fn top_level_test() {
    assert_eq!(double(2), 4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doubles() {
        assert_eq!(double(3), 6);
        assert!(double(0) == 0, "zero");
    }
}
