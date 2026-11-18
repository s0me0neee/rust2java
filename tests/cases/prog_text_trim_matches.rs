// `str::trim_matches` with a literal char pattern is one `replaceAll`.
fn main() {
    let s = "***hello***";
    println!("{}", s.trim_matches('*'));
    println!("{}", "\"quoted\"".trim_matches('"'));
}
