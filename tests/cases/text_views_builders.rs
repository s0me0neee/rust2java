// Strings built in loops (StringBuilder), suffix cursors (base + offset) and byte reads.
use std::fmt::Write;

fn shout(s: &str) -> String {
    s.to_uppercase()
}

fn tokens(text: &str) -> Vec<String> {
    let mut rest = text.trim();
    let mut out = Vec::new();
    while !rest.is_empty() {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        out.push(rest[..end].to_string());
        rest = rest[end..].trim_start();
    }
    out
}

fn main() {
    // Suffix cursor: find (char, str, predicate), rfind, starts_with, slices, trim_start.
    let text = "  key=value; next=item;last=end  ";
    let mut rest = text.trim();
    let mut pairs = Vec::new();
    while !rest.is_empty() {
        let eq = rest.find('=').unwrap();
        let end = rest.find(';').unwrap_or(rest.len());
        pairs.push((rest[..eq].to_string(), rest[eq + 1..end].to_string()));
        rest = if end < rest.len() { &rest[end + 1..] } else { &rest[end..] };
        rest = rest.trim_start();
    }
    println!("{:?}", pairs);
    println!("{:?}", tokens("  alpha beta\tgamma\n delta "));

    let line = "abc,def,,ghi";
    let tail = &line[4..];
    println!(
        "{} {} {:?} {:?} {:?} {}",
        tail.len(),
        tail.is_empty(),
        tail.find(","),
        tail.rfind(','),
        tail.find(|c: char| c == 'h'),
        tail.starts_with("def")
    );
    println!("{} {} {}", &tail[..3], &tail[1..=2], shout(tail));

    // StringBuilder: push, push_str, +=, write!, writeln!, clear, len, is_empty, reassign.
    let mut out = String::new();
    for i in 0..5 {
        if out.is_empty() {
            out.push('[');
        } else {
            out += ", ";
        }
        write!(out, "{}", i * i).unwrap();
        out.push_str(&"!".repeat(i % 2));
    }
    out.push(']');
    println!("{} {} {}", out, out.len(), shout(&out));
    let mut report = String::from("start\n");
    for word in ["x", "y"] {
        writeln!(report, "- {}", word).unwrap();
    }
    print!("{}", report);
    report.clear();
    for n in 1..=3 {
        report += &n.to_string();
    }
    println!("{} {}", report, report == "123");
    report = String::from("reset");
    for c in "ab".chars() {
        report.push(c);
    }
    println!("{}", report);

    // Byte reads without copying the string.
    let s = "hello";
    let mut same = 0;
    for i in 1..s.as_bytes().len() {
        if s.as_bytes()[i] == s.as_bytes()[i - 1] {
            same += 1;
        }
    }
    println!("{} {}", same, s.as_bytes()[0]);
}
