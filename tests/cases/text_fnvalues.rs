// String/char functions used as values.
fn main() {
    let parts: Vec<String> = "x y  z".split_whitespace().map(String::from).collect();
    println!("{:?}", parts);

    let owned: Vec<String> = vec!["a", "bb"].iter().map(|s| str::to_string(*s)).collect();
    println!("{:?}", owned);

    let upper: Vec<String> = vec!["a", "b"].iter().map(ToString::to_string).collect();
    println!("{}", upper.join("+"));

    let kept: Vec<bool> = "Ab1 -".chars().map(char::is_alphanumeric).collect();
    println!("{:?}", kept);

    let letters: Vec<bool> = "a1 B".chars().map(char::is_alphabetic).collect();
    println!("{:?}", letters);

    let spaced: Vec<bool> = "a b\tc".chars().map(char::is_whitespace).collect();
    println!("{:?}", spaced);

    let lowered: Vec<char> = "AbC".chars().map(|c| c.to_ascii_lowercase()).collect();
    println!("{:?}", lowered);

    let from_bytes: String = [97u8, 98, 99].iter().map(|&b| char::from(b)).collect();
    println!("{}", from_bytes);
}
