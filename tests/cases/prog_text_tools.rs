use std::io::{self, BufRead};
fn caesar(s: &str, k: u8) -> String {
    s.chars().map(|c| {
        if c.is_ascii_lowercase() { (((c as u8 - b'a' + k) % 26) + b'a') as char }
        else if c.is_ascii_uppercase() { (((c as u8 - b'A' + k) % 26) + b'A') as char }
        else { c }
    }).collect()
}
fn is_palindrome(s: &str) -> bool {
    let v: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
    v.iter().eq(v.iter().rev())
}
fn rle(s: &str) -> String {
    let mut out = String::new();
    let cs: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        let mut j = i;
        while j < cs.len() && cs[j] == cs[i] { j += 1; }
        if j - i > 1 { out += &(j - i).to_string(); }
        out.push(cs[i]);
        i = j;
    }
    out
}
fn main() {
    let stdin = io::stdin();
    let mut longest = String::new();
    let mut total_words = 0usize;
    for (n, line) in stdin.lock().lines().enumerate() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() { continue; }
        let words: Vec<&str> = line.split_whitespace().collect();
        total_words += words.len();
        if let Some(w) = words.iter().max_by_key(|w| w.len()) {
            if w.len() > longest.len() { longest = w.to_string(); }
        }
        println!("{:>3}| {} | {} | pal={} | {}", n + 1, caesar(line, 3), rle(line), is_palindrome(line), words.iter().rev().cloned().collect::<Vec<_>>().join(" "));
        let digits: u32 = line.chars().filter_map(|c| c.to_digit(10)).sum();
        if digits > 0 { println!("    digit sum {digits}"); }
    }
    println!("words={total_words} longest={longest:?}");
}
