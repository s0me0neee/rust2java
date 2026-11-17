// Edition-2024 let chains in `if` and `while`, plus `let … else`.
fn parse(x: i32) -> Result<i32, String> {
    if x > 0 {
        Ok(x * 2)
    } else {
        Err(format!("{} is not positive", x))
    }
}

fn chain(a: Option<i32>) -> String {
    if let Some(x) = a && let Ok(y) = parse(x) && y > 2 {
        return format!("ok {}", y);
    }
    "no".to_string()
}

fn classify(a: Option<i32>, b: Option<i32>) -> String {
    if let Some(x) = a && x > 3 && let Some(y) = b && y > x {
        return format!("both {} {}", x, y);
    }
    if let Some(x) = a {
        return format!("only {}", x);
    }
    "neither".to_string()
}

fn peek(v: &Vec<i32>, i: usize) -> Option<&i32> {
    if i < v.len() {
        Some(&v[i])
    } else {
        None
    }
}

fn first_even(v: &Vec<i32>) -> Option<i32> {
    let mut i = 0;
    while let Some(x) = peek(v, i) && i < v.len() {
        if *x % 2 == 0 {
            return Some(*x);
        }
        i += 1;
    }
    None
}

fn main() {
    println!("{}", chain(Some(5)));
    println!("{}", chain(Some(-5)));
    println!("{}", chain(None));
    println!("{}", classify(Some(5), Some(9)));
    println!("{}", classify(Some(5), Some(2)));
    println!("{}", classify(Some(1), None));
    println!("{}", classify(None, None));
    let mut v = Vec::new();
    v.push(1);
    v.push(3);
    v.push(8);
    match first_even(&v) {
        Some(x) => println!("even {}", x),
        None => println!("no even"),
    }
    let o: Option<i32> = Some(7);
    let Some(n) = o else {
        println!("missing");
        return;
    };
    if n > 5 && n < 10 {
        println!("mid {}", n);
    }
    println!("done");
}
