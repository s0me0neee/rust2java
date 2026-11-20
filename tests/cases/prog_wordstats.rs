// Text statistics from stdin: read_to_string, word frequencies with top-k by (count desc, word asc),
// line/char counts, longest line, sentence splitting, and a histogram rendered with repeat.
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, Read};

fn normalize(w: &str) -> String {
    w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

fn main() {
    let mut text = String::new();
    io::stdin().read_to_string(&mut text).expect("read");
    let lines: Vec<&str> = text.lines().collect();
    let words: Vec<String> = text.split_whitespace().map(normalize).filter(|w| !w.is_empty()).collect();
    println!("lines={} words={} chars={} bytes={}", lines.len(), words.len(), text.chars().count(), text.len());

    let mut freq: HashMap<&str, usize> = HashMap::new();
    for w in &words {
        *freq.entry(w.as_str()).or_insert(0) += 1;
    }
    let mut top: Vec<(&str, usize)> = freq.iter().map(|(k, v)| (*k, *v)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    for (w, n) in top.iter().take(5) {
        println!("{:>8} {:>2} {}", w, n, "#".repeat(*n));
    }

    let longest = lines.iter().enumerate().max_by_key(|(_, l)| l.len()).map(|(i, l)| (i + 1, l.len()));
    println!("longest line {:?}", longest);
    let blank = lines.iter().filter(|l| l.trim().is_empty()).count();
    println!("blank lines {}", blank);

    let sentences: Vec<&str> = text.split(|c| c == '.' || c == '!' || c == '?').map(str::trim).filter(|s| !s.is_empty()).collect();
    println!("sentences {}", sentences.len());
    if let Some(first) = sentences.first() {
        println!("first: {:?}", first.split_whitespace().take(4).collect::<Vec<_>>().join(" "));
    }

    let by_len: BTreeMap<usize, HashSet<&str>> = words.iter().fold(BTreeMap::new(), |mut m, w| {
        m.entry(w.len()).or_insert_with(HashSet::new).insert(w.as_str());
        m
    });
    for (len, set) in by_len.iter().rev().take(3) {
        let mut v: Vec<_> = set.iter().collect();
        v.sort();
        println!("{} letters: {:?}", len, v);
    }
    let caps = text.split_whitespace().filter(|w| w.chars().next().is_some_and(char::is_uppercase)).count();
    let digits: u32 = text.chars().filter_map(|c| c.to_digit(10)).sum();
    println!("capitalized {} digit sum {}", caps, digits);
    let vowels = ['a', 'e', 'i', 'o', 'u'];
    let ratio = words.iter().map(|w| w.chars().filter(|c| vowels.contains(c)).count()).sum::<usize>() as f64
        / words.iter().map(|w| w.len()).sum::<usize>().max(1) as f64;
    println!("vowel ratio {:.3}", ratio);
}
