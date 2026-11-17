// Tuple destructuring and slice patterns.
fn unpack(t: (i32, &str)) -> String {
    let (a, b) = t;
    format!("{}:{}", a, b)
}

fn three(t: (i32, i32, i32)) -> i32 {
    match t {
        (a, b, c) => a + b + c,
    }
}

fn swap(t: (i32, &str)) -> (&str, i32) {
    match t {
        (a, b) => (b, a),
    }
}

fn kind(s: &[i32]) -> &'static str {
    match s {
        [] => "empty",
        [_x] => "one",
        [_x, _y] => "two",
        [first, .., last] => {
            println!("ends {} {}", first, last);
            "many"
        }
    }
}

fn head(s: &[i32]) -> i32 {
    match s {
        [x, ..] => *x,
        _ => -1,
    }
}

fn main() {
    println!("{}", unpack((3, "x")));
    println!("{}", three((1, 2, 3)));
    let (a, b) = swap((1, "y"));
    println!("{} {}", a, b);
    let mut empty = Vec::new();
    println!("{}", kind(&empty));
    empty.push(1);
    println!("{}", kind(&empty));
    empty.push(2);
    println!("{}", kind(&empty));
    empty.push(3);
    println!("{}", kind(&empty));
    println!("{}", head(&empty));
    let mut nothing: Vec<i32> = Vec::new();
    println!("{}", head(&nothing));
    nothing.push(42);
    match nothing {
        ref v => println!("one element {}", v.len()),
    }
    let triple = (1, 2, 3);
    match triple {
        (x, _, z) => println!("first {} last {}", x, z),
    }
}
