fn is_valid(code: &str) -> bool {
    let digits: Option<Vec<u32>> = code
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| c.to_digit(10))
        .collect();
    let Some(digits) = digits else { return false };
    if digits.len() < 2 {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| if i % 2 == 1 { if d * 2 > 9 { d * 2 - 9 } else { d * 2 } } else { d })
        .sum();
    sum % 10 == 0
}

fn main() {
    for s in ["4539 3195 0343 6467", "8273 1232 7352 0569", "059", "0", "055 444 285", "055a 444 285"] {
        println!("{:<22}{}", s, if is_valid(s) { "valid" } else { "invalid" });
    }
}
