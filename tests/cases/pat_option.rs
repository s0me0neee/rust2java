// Option patterns: match, guards, nesting, if-let, let-else and while-let.
fn describe(o: Option<i32>) -> String {
    match o {
        Some(1) => "one".to_string(),
        Some(n) if n > 10 => format!("big {}", n),
        Some(n) => format!("some {}", n),
        None => "none".to_string(),
    }
}

fn sum_pair(p: Option<(i32, i32)>) -> i32 {
    match p {
        Some((a, b)) => a + b,
        None => 0,
    }
}

fn deepen(o: Option<Option<i32>>) -> i32 {
    match o {
        Some(Some(x)) => x,
        Some(None) => -1,
        None => -2,
    }
}

fn main() {
    println!("{}", describe(Some(1)));
    println!("{}", describe(Some(42)));
    println!("{}", describe(Some(7)));
    println!("{}", describe(None));
    println!("{}", sum_pair(Some((2, 3))));
    println!("{}", sum_pair(None));
    println!("{}", deepen(Some(Some(5))));
    println!("{}", deepen(Some(None)));
    println!("{}", deepen(None));
    let o = Some(9);
    if let Some(x) = o {
        println!("iflet {}", x);
    } else {
        println!("iflet none");
    }
    let empty: Option<i32> = None;
    if let Some(_) = empty {
        println!("has value");
    } else {
        println!("empty");
    }
    let Some(v) = Some(4) else {
        println!("unreachable");
        return;
    };
    println!("letelse {}", v);
    let mut stack = Vec::new();
    stack.push(10);
    stack.push(20);
    stack.push(30);
    while let Some(top) = stack.pop() {
        println!("pop {}", top);
    }
    let mut seen = Vec::new();
    while let Some(x) = next(&mut seen) {
        println!("next {}", x);
    }
    println!("seen {}", seen.len());
}

// Yields values while the shared counter is below 3.
fn next(seen: &mut Vec<i32>) -> Option<i32> {
    if seen.len() >= 3 {
        None
    } else {
        seen.push(seen.len() as i32 + 1);
        Some(seen.len() as i32 * 5)
    }
}
