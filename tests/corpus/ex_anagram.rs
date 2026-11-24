use std::collections::HashSet;

fn sorted_lower(word: &str) -> (String, Vec<char>) {
    let lower = word.to_lowercase();
    let mut chars: Vec<char> = lower.chars().collect();
    chars.sort_unstable();
    (lower, chars)
}

fn anagrams_for<'a>(word: &str, candidates: &[&'a str]) -> HashSet<&'a str> {
    let (lower, key) = sorted_lower(word);
    candidates
        .iter()
        .copied()
        .filter(|c| {
            let (cl, ck) = sorted_lower(c);
            cl != lower && ck == key
        })
        .collect()
}

fn main() {
    let cases: [(&str, &[&str]); 3] = [
        ("listen", &["enlists", "google", "inlets", "banana", "Silent", "LISTEN"]),
        ("Orchestra", &["cashregister", "Carthorse", "radishes"]),
        ("go", &["Go", "og", "goo"]),
    ];
    for (word, cands) in cases {
        let mut found: Vec<&str> = anagrams_for(word, cands).into_iter().collect();
        found.sort();
        println!("{word}: {}", found.join(","));
    }
}
