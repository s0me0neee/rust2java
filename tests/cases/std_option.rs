// Option and Result methods (std_map, §9.2).
fn main() {
    let a: Option<i32> = Some(10);
    let b: Option<i32> = None;
    println!("{} {} {}", a.is_some(), b.is_none(), a.unwrap());
    println!("{} {} {}", a.unwrap_or(0), b.unwrap_or(0), b.unwrap_or_else(|| 7));
    println!("{:?} {:?}", b.unwrap_or_default(), a.unwrap_or_default());
    println!("{:?} {:?}", a.map(|x| x * 2), b.map(|x| x * 2));
    println!("{:?} {:?}", a.filter(|x| *x > 5), a.filter(|x| *x > 50));
    println!("{:?} {:?}", b.or(Some(3)), a.or(Some(3)));
    println!("{:?}", a.and_then(|x| if x > 5 { Some(x + 1) } else { None }));
    println!("{:?} {:?}", b.ok_or(0), a.ok_or(0));
    println!("{} {}", a.map_or(0, |x| x + 1), b.map_or(0, |x| x + 1));
    println!("{} {}", a.is_some_and(|x| x > 5), b.is_none_or(|x| x > 5));
    println!("{:?}", b.or_else(|| Some(4)));
    println!("{:?}", a.map_or_else(|| 0, |x| x * 3));

    let mut c = Some(1);
    let old = c.take();
    println!("{:?} {:?}", old, c);
    let mut d = Some(2);
    let prev = d.replace(9);
    println!("{:?} {:?}", prev, d);

    let ok: Result<i32, i32> = Ok(5);
    let err: Result<i32, i32> = Err(7);
    println!("{} {}", ok.is_ok(), err.is_err());
    println!("{:?} {:?}", ok, err);
    println!("{:?} {:?} {:?} {:?}", ok.map(|x| x + 1), err.map(|x| x + 1), ok.ok(), err.ok());
    println!("{:?} {:?}", ok.err(), err.err());
    println!("{} {}", ok.unwrap_or(0), err.unwrap_or(0));
    println!("{}", err.unwrap_or_else(|e| e * 2));
    println!("{} {}", ok.is_ok_and(|x| x > 3), err.is_err_and(|e| e > 5));
    println!("{:?} {:?}", ok.map_err(|e| e + 1), err.map_err(|e| e + 1));
    println!("{:?}", ok.and_then(|x| if x > 3 { Ok(x * 2) } else { Err(x) }));
    println!("{}", ok.expect("ok value"));
}
