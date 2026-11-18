fn main() {
    let words = vec!["alpha", "beta", "gamma"];
    let lens: Vec<usize> = words.iter().map(|w| w.len()).collect();
    println!("{:?} {}", lens, lens.len());
    let sum: usize = lens.iter().sum();
    println!("{sum}");
    // Collected only to be printed.
    println!("{:?}", words.iter().map(|w| w.to_uppercase()).collect::<Vec<String>>());
    // Collected then joined.
    let joined = words.iter().map(|w| w.to_string()).collect::<Vec<String>>().join("|");
    println!("{joined}");
    // Collected and mutated: still a mutable list.
    let mut owned: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    owned.push("delta".to_string());
    println!("{:?}", owned);
    // Chars collected into a String.
    let initials: String = words.iter().filter_map(|w| w.chars().next()).collect();
    println!("{initials}");
    let reversed = "stressed".chars().rev().collect::<String>();
    println!("{reversed}");
    for part in reversed.split('-') {
        println!("{part}");
    }
}
