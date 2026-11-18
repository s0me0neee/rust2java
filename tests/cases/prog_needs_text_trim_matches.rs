// `str::trim_matches` with a char pattern is not lowered yet.
fn main() {
    let s = "***hello***";
    println!("{}", s.trim_matches('*'));
    println!("{}", "\"quoted\"".trim_matches('"'));
}
