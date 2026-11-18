// `sort()` on a Vec of pairs whose second field is not comparable is not lowered yet.
fn main() {
    let mut rows: Vec<(i32, Vec<String>)> =
        vec![(2, vec!["b".to_string()]), (1, vec!["a".to_string()])];
    rows.sort();
    let labels: Vec<String> = rows
        .iter()
        .map(|(n, words)| format!("{}{}", n, words.len()))
        .collect();
    println!("{:?}", labels);
}
