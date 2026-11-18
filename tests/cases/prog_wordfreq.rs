// Word frequency counter over stdin: read all lines, split on non-letters,
// count with a HashMap, then report the top entries in sorted order.
use std::collections::HashMap;
use std::io::{self, BufRead};

fn normalize(word: &str) -> String {
    let mut out = String::new();
    for c in word.chars() {
        if c.is_alphabetic() {
            out.push(c.to_ascii_lowercase());
        } else if c == '\'' && !out.is_empty() {
            out.push(c);
        }
    }
    while out.ends_with('\'') {
        out.pop();
    }
    out
}

fn count_words(text: &str) -> HashMap<String, i32> {
    let mut counts: HashMap<String, i32> = HashMap::new();
    for chunk in text.split(|c: char| !c.is_alphabetic() && c != '\'') {
        let word = normalize(chunk);
        if word.len() >= 2 {
            *counts.entry(word).or_insert(0) += 1;
        }
    }
    counts
}

fn top_words(counts: &HashMap<String, i32>, n: usize) -> Vec<(String, i32)> {
    let mut ranked: Vec<(String, i32)> = counts
        .iter()
        .map(|(word, count)| (word.clone(), *count))
        .collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(n);
    ranked
}

fn histogram(pairs: &[(String, i32)]) -> Vec<String> {
    let widest = pairs.iter().map(|(w, _)| w.len()).max().unwrap_or(0);
    pairs
        .iter()
        .map(|(word, count)| {
            let bar = "#".repeat(*count as usize);
            format!("{:<width$} | {:>3} {}", word, count, bar, width = widest)
        })
        .collect()
}

fn main() {
    let stdin = io::stdin();
    let mut text = String::new();
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        text.push_str(&line);
        text.push(' ');
    }
    let counts = count_words(&text);
    let mut total = 0;
    for (_, count) in &counts {
        total += *count;
    }
    println!("{} distinct words, {} tokens", counts.len(), total);
    let pairs = top_words(&counts, 5);
    for line in histogram(&pairs) {
        println!("{}", line);
    }
    let mut words: Vec<&String> = counts.keys().collect();
    words.sort();
    println!("first: {}", words.first().map(|w| w.as_str()).unwrap_or("-"));
    println!("last: {}", words.last().map(|w| w.as_str()).unwrap_or("-"));
    let long = words.iter().filter(|w| w.len() > 5).count();
    let sum: i32 = counts.values().sum();
    println!("long words: {}, sum: {}", long, sum);
    let mut lengths: HashMap<usize, Vec<String>> = HashMap::new();
    for word in words {
        lengths.entry(word.len()).or_default().push(word.clone());
    }
    let mut sizes: Vec<(usize, usize)> = lengths
        .iter()
        .map(|(len, group)| (*len, group.len()))
        .collect();
    sizes.sort();
    for (len, count) in &sizes {
        println!("{:>2} letters: {}", len, count);
    }
}
