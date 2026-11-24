fn reply(message: &str) -> &'static str {
    let m = message.trim();
    let question = m.ends_with('?');
    let yelling = m.chars().any(|c| c.is_alphabetic()) && m == m.to_uppercase();
    match (m.is_empty(), question, yelling) {
        (true, _, _) => "Fine. Be that way!",
        (_, true, true) => "Calm down, I know what I'm doing!",
        (_, true, false) => "Sure.",
        (_, false, true) => "Whoa, chill out!",
        _ => "Whatever.",
    }
}

fn main() {
    let inputs = ["Tom-ay-to, tom-aaaah-to.", "WATCH OUT!", "Does this cryogenic chamber make me look fat?", "WHAT'S GOING ON?", "   ", "1, 2, 3", "4?"];
    for i in inputs {
        println!("{:>48} -> {}", i, reply(i));
    }
}
