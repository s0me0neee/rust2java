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
        cur.push_str(word);
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

fn justify(line: &str, width: usize) -> String {
    let words: Vec<&str> = line.split(' ').collect();
    if words.len() == 1 {
        return format!("{:<width$}", line);
    }
    let gaps = words.len() - 1;
    let spaces = width - words.iter().map(|w| w.len()).sum::<usize>();
    let mut out = String::with_capacity(width);
    for (i, w) in words.iter().enumerate() {
        out += w;
        if i < gaps {
            let n = spaces / gaps + usize::from(i < spaces % gaps);
            out.push_str(&" ".repeat(n));
        }
    }
    out
}

fn main() {
    let text = "The quick brown fox jumps over the lazy dog and keeps running far into the distance";
    for w in [20, 32] {
        println!("--- width {w}");
        let lines = wrap(text, w);
        for (i, l) in lines.iter().enumerate() {
            let shown = if i + 1 < lines.len() { justify(l, w) } else { l.clone() };
            println!("|{shown:<w$}|");
        }
    }
    let caps: String = text.split(' ').map(|w| {
        let mut c = w.chars();
        match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => String::new(),
        }
    }).collect::<Vec<_>>().join(" ");
    println!("{caps}");
}
