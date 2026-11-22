fn main() {
    let some: Option<i32> = Some(4);
    let none: Option<i32> = None;
    println!("{:?} {:?}", some.filter(|x| *x > 5), none.filter(|x| *x > 5));
    println!("{:?} {:?}", some.xor(none), some.xor(Some(1)));
    println!("{:?} {:?}", some.and(Some("a")), none.and(Some("a")));
    println!("{:?} {:?}", some.or(Some(9)), none.or(Some(9)));
    println!("{:?} {:?}", some.zip(Some('c')), some.zip(None::<char>));
    println!("{} {}", some.is_some_and(|x| x % 2 == 0), none.is_some_and(|x| x % 2 == 0));
    println!("{} {}", some.is_none_or(|x| x > 10), none.is_none_or(|x| x > 10));
    println!("{} {}", some.map_or(0, |x| x * 3), none.map_or(0, |x| x * 3));
    println!("{} {}", some.map_or_else(|| -1, |x| x * 3), none.map_or_else(|| -1, |x| x * 3));
    println!("{:?} {:?}", some.ok_or("e"), none.ok_or("e"));
    println!("{} {}", some.unwrap_or_default(), none.unwrap_or_default());
    let mut m = Some(3);
    let old = m.replace(7);
    let taken = m.take();
    println!("{:?} {:?} {:?}", old, taken, m);
    let mut g: Option<Vec<i32>> = None;
    g.get_or_insert_with(Vec::new).push(5);
    g.get_or_insert_with(Vec::new).push(6);
    println!("{:?}", g);
    let mut h = Some(10);
    if let Some(x) = h.as_mut() { *x += 1; }
    println!("{:?} {:?}", h, h.iter().count());
    let nested: Option<Option<i32>> = Some(None);
    println!("{:?} {:?}", nested.flatten(), Some(Some(2)).flatten());
    println!("{:?}", [Some(1), None, Some(3)].iter().flatten().sum::<i32>());
    let r: Result<i32, String> = Err("bad".into());
    let k: Result<i32, String> = Ok(2);
    println!("{:?} {:?} {:?}", r.clone().ok(), r.clone().err(), k.clone().err());
    println!("{} {}", r.clone().unwrap_or(0), k.clone().unwrap_or_else(|e| e.len() as i32));
    println!("{:?} {:?}", r.clone().or_else(|e| if e == "bad" { Ok::<i32, String>(-1) } else { Err(e) }), k.clone().and_then(|v| if v > 1 { Err::<i32, String>("big".into()) } else { Ok(v) }));
    println!("{} {}", r.is_err_and(|e| e.starts_with('b')), k.clone().is_ok_and(|v| v == 2));
    println!("{:?}", k.map_err(|e| e.len()));
}
