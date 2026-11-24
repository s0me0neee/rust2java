#[derive(Debug, PartialEq, Eq)]
pub enum Comparison {
    Equal,
    Sublist,
    Superlist,
    Unequal,
}

fn contains<T: PartialEq>(big: &[T], small: &[T]) -> bool {
    small.is_empty() || big.windows(small.len()).any(|w| w == small)
}

pub fn sublist<T: PartialEq>(a: &[T], b: &[T]) -> Comparison {
    if a == b {
        Comparison::Equal
    } else if contains(b, a) {
        Comparison::Sublist
    } else if contains(a, b) {
        Comparison::Superlist
    } else {
        Comparison::Unequal
    }
}

fn main() {
    println!("{:?}", sublist(&[1, 2, 3], &[1, 2, 3]));
    println!("{:?}", sublist(&[2, 3], &[1, 2, 3, 4]));
    println!("{:?}", sublist(&[1, 2, 3, 4], &[3, 4]));
    println!("{:?}", sublist(&[1, 3], &[1, 2, 3]));
    println!("{:?}", sublist::<i32>(&[], &[]));
    println!("{:?}", sublist(&["a", "b"], &["x", "a", "b"]));
}
