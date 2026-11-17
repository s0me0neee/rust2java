// Basic control flow and arithmetic.
fn add(a: i32, b: i32) -> i32 {
    a + b
}

fn classify(n: i32) -> &'static str {
    if n < 0 { "negative" } else if n == 0 { "zero" } else { "positive" }
}

fn main() {
    let x = 5;
    let y = add(x, 7); // twelve
    let mut total = 0;
    for i in 0..10 {
        if i % 2 == 0 {
            continue;
        }
        total += i;
    }
    let mut n = 0u64;
    while n < 3 {
        n += 1;
    }
    let x = x * 2;
    let label = classify(y - 20);
    let ratio = 7.0 / 2.0;
    let whole = 3.0;
    println!("x = {}, y = {y}, total = {}", x, total);
    println!("{} {} {} {}", label, ratio, whole, n);
    let v = loop {
        n += 1;
        if n > 5 {
            break n * 10;
        }
    };
    println!("v = {v} {:?} {:?}", "q", 1.5);
}
