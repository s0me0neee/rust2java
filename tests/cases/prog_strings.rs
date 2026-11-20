// String algorithms: run-length encoding, caesar cipher, anagram groups, palindromes, char_indices,
// bytes windows, String capacity ops, char arithmetic, and format width/precision from arguments.
use std::collections::BTreeMap;

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
            out += &n.to_string();
        }
        out.push(c);
    }
    out
}

fn unrle(s: &str) -> String {
    let mut out = String::new();
    let mut num = 0usize;
    for c in s.chars() {
        if let Some(d) = c.to_digit(10) {
            num = num * 10 + d as usize;
        } else {
            out.extend(std::iter::repeat(c).take(num.max(1)));
            num = 0;
        }
    }
    out
}

fn caesar(s: &str, k: u8) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => ((c as u8 - b'a' + k) % 26 + b'a') as char,
            'A'..='Z' => ((c as u8 - b'A' + k) % 26 + b'A') as char,
            _ => c,
        })
        .collect()
}

fn is_pal(s: &str) -> bool {
    let cleaned: Vec<char> = s.chars().filter(|c| c.is_alphanumeric()).map(|c| c.to_ascii_lowercase()).collect();
    cleaned.iter().eq(cleaned.iter().rev())
}

fn longest_pal(s: &str) -> &str {
    let b = s.as_bytes();
    let (mut best_lo, mut best_len) = (0, 0);
    for center in 0..b.len() * 2 {
        let (mut lo, mut hi) = (center / 2, center / 2 + center % 2);
        while hi < b.len() && b[lo] == b[hi] {
            if hi + 1 - lo > best_len {
                best_lo = lo;
                best_len = hi + 1 - lo;
            }
            if lo == 0 {
                break;
            }
            lo -= 1;
            hi += 1;
        }
    }
    &s[best_lo..best_lo + best_len]
}

fn main() {
    for s in ["aaabccddddde", "abc", "", "zzzzzzzzzzzz"] {
        let e = rle(s);
        println!("{:?} -> {:?} -> ok={}", s, e, unrle(&e) == s);
    }
    let secret = caesar("Hello, World! xyz", 3);
    println!("{} | {}", secret, caesar(&secret, 23));

    let words = ["listen", "silent", "enlist", "google", "gogole", "cat", "act", "tac", "dog"];
    let mut groups: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    for w in words {
        let mut k: Vec<char> = w.chars().collect();
        k.sort_unstable();
        groups.entry(k.into_iter().collect()).or_default().push(w);
    }
    for (k, g) in &groups {
        if g.len() > 1 {
            println!("{:<8}{}", k, g.join("/"));
        }
    }

    for s in ["A man, a plan, a canal: Panama", "race a car", "No 'x' in Nixon"] {
        println!("{:>32} {}", s, is_pal(s));
    }
    println!("{:?} {:?}", longest_pal("forgeeksskeegfor"), longest_pal("abacdfgdcaba"));

    let text = "the quick brown fox";
    let caps: Vec<usize> = text.char_indices().filter(|(_, c)| *c == ' ').map(|(i, _)| i + 1).collect();
    let mut title = text.to_string();
    for &i in caps.iter().chain(std::iter::once(&0)) {
        let up = title[i..i + 1].to_uppercase();
        title.replace_range(i..i + 1, &up);
    }
    println!("{}", title);
    let doubles = text.as_bytes().windows(2).filter(|w| w[0] == w[1]).count();
    let vowels = text.bytes().filter(|b| b"aeiou".contains(b)).count();
    println!("doubles {} vowels {}", doubles, vowels);
    let shifted: String = ('a'..='e').map(|c| ((c as u8) + 1) as char).rev().collect();
    println!("{} {}", shifted, char::from_digit(7, 10).unwrap());
    let (w, p) = (10, 3);
    println!("[{:>w$.p$}] [{:<width$}] [{:^1$}]", 3.14159, 6, "mid", width = 4);
    let mut buf = String::with_capacity(16);
    for i in (0..20).step_by(4) {
        buf.push_str(&format!("{:02x}", i * 13));
    }
    buf.truncate(8);
    let last = buf.pop();
    println!("{} {} {:?}", buf, buf.len(), last);
    let pos = text.find("brown").map(|i| &text[i..]);
    println!("{:?} {:?} {:?}", pos, text.rfind('o'), text.split_once(' '));
    let counts = text.split(' ').map(str::len).fold(String::new(), |acc, n| {
        if acc.is_empty() { n.to_string() } else { acc + "+" + &n.to_string() }
    });
    println!("{}", counts);
}
