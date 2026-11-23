fn emit(out: &mut String, depth: usize, word: &str) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push_str(word);
    out.push('\n');
}

fn render(items: &[&str], out: &mut String) {
    for (i, w) in items.iter().enumerate() {
        emit(out, i, w);
    }
    if out.ends_with('\n') {
        out.pop();
    }
}

fn main() {
    let mut s = String::new();
    render(&["a", "b", "c"], &mut s);
    println!("{}", s);
    println!("{}", s.len());
}
