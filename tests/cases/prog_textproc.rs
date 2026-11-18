// Text processing: wrapping, char_indices, splitn/rsplit, find with closures, byte slicing, caesar, run-length.
use std::collections::HashMap;

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut cur = String::new();
    for word in text.split_whitespace() {
        if !cur.is_empty() && cur.len() + 1 + word.len() > width {
            lines.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur += word;
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn rle(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        let mut n = 1;
        while chars.peek() == Some(&c) {
            chars.next();
            n += 1;
        }
        if n > 1 {
            out.push_str(&n.to_string());
        }
        out.push(c);
    }
    out
}

fn caesar(s: &str, k: u8) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + k) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + k) % 26) + b'A') as char,
            _ => c,
        })
        .collect()
}

fn is_palindrome(s: &str) -> bool {
    let cleaned: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
    cleaned.iter().eq(cleaned.iter().rev())
}

fn parse_kv(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once('=')?;
    Some((k.trim(), v.trim()))
}

fn main() {
    let text = "The quick brown fox jumps over the lazy dog. Pack my box with five dozen liquor jugs!";
    for (i, l) in wrap(text, 24).iter().enumerate() {
        println!("{:02}|{:<24}|{}", i, l, l.len());
    }
    println!("{} {} {}", rle("aaabccddddde"), rle(""), rle("xyz"));
    let enc = caesar("Hello, World!", 3);
    println!("{} -> {}", enc, caesar(&enc, 23));
    for s in ["A man, a plan, a canal: Panama", "hello", "Was it a car or a cat I saw?"] {
        println!("{:?} {}", s, is_palindrome(s));
    }
    let config = "name = demo\nversion=1.2.3\n# comment\nbad line\n  path =  /usr/local  ";
    let parsed: Vec<(&str, &str)> = config.lines().filter(|l| !l.starts_with('#')).filter_map(parse_kv).collect();
    println!("{:?}", parsed);
    let version = parsed.iter().find(|(k, _)| *k == "version").map(|(_, v)| *v).unwrap_or("0");
    let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
    println!("{:?} major={}", parts, parts[0]);
    let path = "/home/user/docs/report.final.txt";
    let file = path.rsplit('/').next().unwrap();
    let (stem, ext) = file.rsplit_once('.').unwrap();
    let dirs: Vec<&str> = path.splitn(4, '/').collect();
    println!("{} {} {} {:?}", file, stem, ext, dirs);
    let first_upper = text.char_indices().filter(|(_, c)| c.is_uppercase()).map(|(i, _)| i).collect::<Vec<_>>();
    println!("{:?} {:?} {:?}", first_upper, text.find("fox"), text.find(|c: char| c == '!'));
    println!("{} | {} | {}", &text[4..9], &text[text.len() - 6..], text.to_uppercase().matches("THE").count());
    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in text.chars().filter(|c| c.is_alphabetic()) {
        *freq.entry(c.to_ascii_lowercase()).or_insert(0) += 1;
    }
    let mut top: Vec<(char, usize)> = freq.into_iter().collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("{:?}", &top[..5]);
    let acronym: String = "portable network graphics".split(' ').map(|w| w.chars().next().unwrap().to_ascii_uppercase()).collect();
    println!("{} {}", acronym, "ab".repeat(3).replace("ba", "-"));
    let bytes = "hello world".bytes().filter(|b| *b > b'n').count();
    println!("{} {} {}", bytes, "hello".len(), "hello".chars().count());
}
