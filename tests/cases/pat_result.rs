// Result patterns and the `?` operator.
fn positive(n: i32) -> Result<i32, String> {
    if n > 0 {
        Ok(n)
    } else {
        Err(format!("{} is not positive", n))
    }
}

fn describe(r: Result<i32, String>) -> String {
    match r {
        Ok(1) => "one".to_string(),
        Ok(n) if n > 100 => format!("huge {}", n),
        Ok(n) => format!("ok {}", n),
        Err(e) => format!("error {}", e),
    }
}

fn add(a: i32, b: i32) -> Result<i32, String> {
    let x = positive(a)?;
    let y = positive(b)?;
    Ok(x + y)
}

fn check(n: i32) -> Result<(), String> {
    if n > 0 {
        Ok(())
    } else {
        Err(format!("neg {}", n))
    }
}

fn both(n: i32) -> Result<(), String> {
    check(n)?;
    check(n + 1)?;
    Ok(())
}

fn tail(n: i32) -> Result<i32, String> {
    let v = positive(n)?;
    Ok(v + 1)
}

fn classify(r: Result<i32, String>) -> &'static str {
    match r {
        Ok(n) if n > 10 => "big",
        Ok(n) if n > 0 => "small",
        Ok(_) => "non-positive",
        Err(_) => "error",
    }
}

fn pick(o: Option<i32>) -> Result<i32, String> {
    let v = match o {
        Some(x) => positive(x)?,
        None => 0,
    };
    Ok(v + 1)
}

fn main() {
    println!("{}", describe(positive(1)));
    println!("{}", describe(positive(7)));
    println!("{}", describe(positive(500)));
    println!("{}", describe(positive(-3)));
    println!("{}", describe(add(2, 3)));
    println!("{}", describe(add(2, -3)));
    match tail(8) {
        Ok(n) => println!("tail {}", n),
        Err(e) => println!("tail failed {}", e),
    }
    match both(1) {
        Ok(()) => println!("both ok"),
        Err(e) => println!("both failed {}", e),
    }
    match both(-1) {
        Ok(()) => println!("both ok"),
        Err(e) => println!("both failed {}", e),
    }
    match add(5, 5) {
        Ok(n) => println!("sum {}", n),
        Err(e) => println!("sum failed {}", e),
    }
    let r = positive(2);
    if let Ok(v) = r {
        println!("iflet ok {}", v);
    } else {
        println!("iflet err");
    }
    println!(
        "{} {} {} {}",
        classify(Ok(50)),
        classify(Ok(5)),
        classify(Ok(0)),
        classify(Err("no".to_string()))
    );
    match pick(Some(4)) {
        Ok(n) => println!("pick {}", n),
        Err(e) => println!("pick failed {}", e),
    }
    match pick(None) {
        Ok(n) => println!("pick {}", n),
        Err(e) => println!("pick failed {}", e),
    }
    let Ok(v) = positive(3) else {
        println!("unreachable");
        return;
    };
    println!("letelse ok {}", v);
}
