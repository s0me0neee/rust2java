// split by predicate, char set, and char function values.
fn main() {
    let hyph: Vec<&str> = "a-b_c d".split(|c: char| c == '-' || c == '_').collect();
    println!("{:?}", hyph);

    let set: Vec<&str> = "one-two_three".split(&['-', '_'][..]).collect();
    println!("{:?}", set);

    let ws: Vec<&str> = "a b\tc".split(char::is_whitespace).collect();
    println!("{:?}", ws);

    let punct: Vec<&str> = "ab.cd,ef".split(|c: char| c.is_ascii_punctuation()).collect();
    println!("{:?}", punct);

    let pieces: Vec<&str> = "a,,b".split(',').collect();
    println!("{:?}", pieces);

    let limited: Vec<&str> = "x-y-z".splitn(2, '-').collect();
    println!("{:?}", limited);

    let two: Vec<&str> = "a1b2c3".splitn(3, |c: char| c.is_ascii_digit()).collect();
    println!("{:?}", two);

    for part in "k1=2;k2=3".split(|c: char| c == ';' || c == '=') {
        println!("[{}]", part);
    }
}
