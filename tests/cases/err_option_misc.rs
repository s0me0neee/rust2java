// Option methods added for w10: zip, xor, flatten, insert, get_or_insert*, inspect, iter, as_mut.
fn main() {
    let a = Some(3);
    let b = Some('x');
    println!("{:?}", a.zip(b));
    println!("{:?}", a.zip(None::<char>));
    println!("{:?}", a.xor(Some(9)));
    println!("{:?}", a.xor(None::<i32>));
    println!("{:?}", None::<i32>.xor(Some(4)));
    println!("{:?}", None::<i32>.xor(None::<i32>));

    let nested: Option<Option<i32>> = Some(Some(5));
    println!("{:?}", nested.flatten());
    let hole: Option<Option<i32>> = Some(None);
    println!("{:?}", hole.flatten());
    let none: Option<Option<i32>> = None;
    println!("{:?}", none.flatten());

    let mut n: Option<i32> = None;
    println!("{:?}", n.get_or_insert(7));
    println!("{:?}", n);
    let mut m: Option<i32> = None;
    let v = m.get_or_insert_with(|| 9);
    *v += 10;
    println!("{:?}", m);

    let mut k = Some(2);
    let old = k.insert(8);
    *old += 1;
    println!("{:?}", k);

    let o = Some(1);
    o.inspect(|x| println!("inspected {}", x));
    None::<i32>.inspect(|x| println!("not {}", x));

    let mut list: Vec<i32> = vec![];
    for x in Some(4).iter() {
        list.push(*x);
    }
    for x in None::<i32>.iter() {
        list.push(*x);
    }
    println!("{:?}", list);

    let r: Option<String> = Some("hi".to_string());
    println!("{:?}", r.as_deref());
    println!("{:?}", r.as_ref().map(|s| s.len()));

    let mut q = Some(1);
    if let Some(ref mut x) = q {
        *x += 2;
    }
    println!("{:?}", q);
    let mut z = Some(10);
    if let Some(x) = z.as_mut() {
        *x *= 2;
    }
    println!("{:?}", z);
}
