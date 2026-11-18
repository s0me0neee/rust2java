fn main() {
    let mut numbers = vec![1, 2, 3, 4];
    numbers.retain_mut(|x| {
        *x += 1;
        *x % 2 == 0
    });
    println!("{:?}", numbers);
    let mut grouped = vec![11, 12, 21, 22, 23, 31];
    grouped.dedup_by_key(|x| *x / 10);
    println!("{:?}", grouped);
    let mut sorted = vec![12, 3, 21, 14];
    sorted.sort_unstable_by_key(|x| *x % 10);
    println!("{:?}", sorted);
    let mut words = vec![String::from("a"), String::from("b")];
    words.retain_mut(|word| {
        word.push_str("!");
        word.len() == 2
    });
    println!("{:?}", words);
}
