use std::collections::HashSet;

fn is_isogram(s: &str) -> bool {
    let mut seen = HashSet::new();
    s.chars()
        .filter(|c| c.is_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .all(|c| seen.insert(c))
}

fn is_pangram(s: &str) -> bool {
    let letters: HashSet<char> = s.to_lowercase().chars().filter(|c| c.is_ascii_lowercase()).collect();
    letters.len() == 26
}

fn main() {
    for s in ["lumberjacks", "isograms", "six-year-old", "Alphabet", "The quick brown fox jumps over the lazy dog", ""] {
        println!("{:?}: iso={} pan={}", s, is_isogram(s), is_pangram(s));
    }
}
