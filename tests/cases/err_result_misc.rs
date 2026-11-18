// Result methods added for w10: unwrap_or_default, is_*_and, inspect_err, ok_or_else,
// expect_err, iter, transpose, copied/cloned, flatten.
fn main() {
    let ok: Result<i32, i32> = Ok(3);
    let err: Result<i32, i32> = Err(5);
    println!("{:?} {:?}", ok.unwrap_or_default(), err.unwrap_or_default());
    println!("{} {}", ok.is_ok_and(|x| x > 2), err.is_err_and(|e| e > 4));

    let tagged = err.inspect_err(|e| println!("saw error {}", e));
    println!("{:?}", tagged);
    let kept = ok.inspect_err(|e| println!("never {}", e));
    println!("{:?}", kept);

    let none: Option<i32> = None;
    let some: Option<i32> = Some(2);
    println!("{:?}", none.ok_or_else(|| 42));
    println!("{:?}", some.ok_or_else(|| 42));

    let bad: Result<i32, String> = Err("boom".to_string());
    println!("{:?}", bad.expect_err("must fail"));
    let empty: Result<i32, String> = Err(String::new());
    println!("{:?}", empty.expect_err("also fails"));

    let doubled: Vec<i32> = ok.iter().map(|x| x + 1).collect();
    println!("{:?}", doubled);

    let good: Result<Option<i32>, String> = Ok(Some(4));
    let hollow: Result<Option<i32>, String> = Ok(None);
    let wrong: Result<Option<i32>, String> = Err("x".into());
    println!("{:?}", good.transpose());
    println!("{:?}", hollow.transpose());
    println!("{:?}", wrong.transpose());

    let nested: Result<Result<i32, String>, String> = Ok(Ok(7));
    println!("{:?}", nested.flatten());
    let inner_bad: Result<Result<i32, String>, String> = Ok(Err("deep".into()));
    println!("{:?}", inner_bad.flatten());
    let outer_bad: Result<Result<i32, String>, String> = Err("top".into());
    println!("{:?}", outer_bad.flatten());

    let n = 3;
    let refs: Result<&i32, &str> = Ok(&n);
    println!("{:?}", refs.copied());
    let owned = String::from("a");
    let text: Result<&String, i32> = Ok(&owned);
    println!("{:?}", text.cloned());
}
