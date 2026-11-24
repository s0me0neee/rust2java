use std::collections::HashSet;

fn marker(s: &str, n: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    bytes
        .windows(n)
        .position(|w| w.iter().collect::<HashSet<_>>().len() == n)
        .map(|p| p + n)
}

fn main() {
    let inputs = [
        "mjqjpqmgbljsphdztnvjfqwrcgsmlb",
        "bvwbjplbgvbhsrlpgdmjqwftvncz",
        "nppdvjthqldpwncqszvftbrmjlhg",
        "aaaa",
    ];
    for s in inputs {
        match (marker(s, 4), marker(s, 14)) {
            (Some(a), Some(b)) => println!("{a} {b}"),
            (Some(a), None) => println!("{a} -"),
            _ => println!("none"),
        }
    }
}
