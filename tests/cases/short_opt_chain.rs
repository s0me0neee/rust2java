// Option plumbing: zip/and/or/xor/filter and target-typed `Optional.empty()`.
fn pick(a: Option<i32>) -> Option<i32> {
    a.filter(|x| x % 2 == 0)
}

fn pair(a: Option<i32>, b: Option<char>) -> Option<(i32, char)> {
    a.zip(b)
}

fn first_even(v: &[i32], start: usize) -> Option<i32> {
    if start < v.len() && v[start] % 2 == 0 {
        Some(v[start])
    } else {
        None
    }
}

fn main() {
    let a = Some(3);
    let b = Some('x');
    println!("{:?} {:?}", a.zip(b), a.zip(None::<char>));
    println!("{:?} {:?}", a.and(Some("hi")), None::<i32>.and(Some("hi")));
    println!("{:?} {:?}", a.and(None::<char>), a.or(Some(9)));
    println!("{:?} {:?}", None.or(Some(9)), a.or_else(|| Some(4)));
    println!("{:?} {:?}", a.xor(Some(2)), None.xor(Some(2)));
    println!("{:?} {:?}", a.filter(|x| *x > 2), a.filter(|x| *x > 9));
    println!("{:?} {:?}", pick(Some(4)), pick(Some(5)));
    println!("{:?}", pair(Some(1), Some('y')));

    let mut c = Some(1);
    let old = c.take();
    println!("{:?} {:?}", old, c);
    let mut d = Some(2);
    let prev = d.replace(9);
    println!("{:?} {:?}", prev, d);

    let values = [2, 3, 4];
    let hit = first_even(&values, 1);
    let miss = first_even(&values, 0);
    println!("{:?} {:?}", hit, miss);
    println!("{:?} {:?}", values.iter().find(|&&x| x > 2), values.iter().find(|&&x| x > 9));
}
