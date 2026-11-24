fn is_valid_isbn(isbn: &str) -> bool {
    let chars: Vec<char> = isbn.chars().filter(|&c| c != '-').collect();
    if chars.len() != 10 {
        return false;
    }
    let mut sum = 0;
    for (i, c) in chars.iter().enumerate() {
        let value = match c.to_digit(10) {
            Some(d) => d,
            None if *c == 'X' && i == 9 => 10,
            None => return false,
        };
        sum += value * (10 - i as u32);
    }
    sum % 11 == 0
}

fn main() {
    for s in ["3-598-21508-8", "3-598-21508-9", "3-598-21507-X", "3-598-2X507-9", "359821507X", "3-598-21507"] {
        println!("{s}: {}", is_valid_isbn(s));
    }
}
