// Literal, range, or-patterns, guards, bindings; match as a statement and as a value.
fn classify(n: i32) -> &'static str {
    match n {
        0 => "zero",
        1 | 2 | 3 => "small",
        4..=9 => "medium",
        x if x < 0 => "negative",
        _ => "large",
    }
}

fn label(n: i32) -> &'static str {
    match n {
        1 if n > 0 => "positive one",
        1 => "one",
        2 | 3 if n % 2 == 1 => "odd two or three",
        2 | 3 => "two or three",
        _ => "other",
    }
}

fn grade(score: i32) -> char {
    match score {
        90..=100 => 'A',
        80..=89 => 'B',
        70..=79 => 'C',
        _ => 'F',
    }
}

fn main() {
    for n in -2..12 {
        println!("{} {}", n, classify(n));
    }
    println!("{} {} {} {}", grade(95), grade(85), grade(75), grade(10));
    for n in 0..5 {
        println!("label {} {}", n, label(n));
    }
    let guarded = match 2 {
        2 if 2 > 5 => "big",
        2 => "exactly two",
        _ => "other",
    };
    println!("{}", guarded);
    let word = "hello";
    match word {
        "hi" | "hello" => println!("greeting"),
        other => println!("other {}", other),
    }
    let c = 'q';
    match c {
        'a'..='m' => println!("first half"),
        'n'..='z' => println!("second half"),
        _ => println!("not a letter"),
    }
    let flag = 3 > 2;
    match flag {
        true => println!("yes"),
        false => println!("no"),
    }
    let n = match 7 {
        1 => 10,
        2 | 3 => 20,
        x if x > 5 => x * 3,
        _ => 0,
    };
    println!("{}", n);
    let pair = match (1, "a") {
        (1, s) => format!("first {}", s),
        (_, s) => format!("other {}", s),
    };
    println!("{}", pair);
    let m = match 4 {
        1..=3 => 1,
        4..=6 => 2,
        _ => 3,
    };
    println!("{}", m);
    match 9 {
        9 => {
            println!("nine");
            println!("still nine");
        }
        _ => println!("not nine"),
    }
}
