use std::collections::HashMap;

fn word_count(text: &str) -> HashMap<String, u32> {
    let mut counts = HashMap::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .map(|w| w.trim_matches('\''))
        .filter(|w| !w.is_empty())
    {
        *counts.entry(word.to_lowercase()).or_default() += 1;
    }
    counts
}

fn main() {
    let text = "Joe can't tell between 'large' and large. Go, go, GO! one fish two fish";
    let counts = word_count(text);
    let mut entries: Vec<(&String, &u32)> = counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    for (w, n) in entries {
        println!("{w}: {n}");
    }
}
