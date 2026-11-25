// Option, Result, bool and Ordering methods, and explicit comparison / conversion trait calls.
use std::cmp::Ordering;
use std::convert::identity;
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq)]
struct Tag {
    name: String,
    n: i32,
}

fn halve(x: i32) -> Result<i32, String> {
    if x % 2 == 0 { Ok(x / 2) } else { Err(format!("{x} is odd")) }
}

fn bump(v: &mut Vec<i32>) {
    v.push(v.len() as i32);
}

fn main() {
    // Option
    let mut nums: Option<Vec<i32>> = Some(vec![3, 1, 2]);
    if let Some(s) = nums.as_deref_mut() {
        s.sort();
    }
    println!("{:?}", nums);
    let mut name: Option<String> = Some("ada".to_string());
    if let Some(s) = name.as_mut() {
        s.push_str("!");
        s.push('?');
    }
    println!("{:?}", name);
    let one = Some(7);
    println!("{:?} {:?}", one.as_slice(), None::<i32>.as_slice());
    let mut two = Some(vec![8]);
    for v in two.as_mut_slice() {
        v.push(1);
    }
    for v in two.iter_mut() {
        v.push(2);
    }
    println!("{:?}", two);
    let mut empty: Option<Vec<i32>> = None;
    empty.get_or_insert_default().push(3);
    println!("{:?}", empty);
    println!("{} {}", one.map_or_default(|x| x * 3), None::<i32>.map_or_default(|x| x * 3));
    let mut big = Some(50);
    println!("{:?} {:?}", big.take_if(|x| *x > 10), big);
    let mut small = Some(5);
    println!("{:?} {:?}", small.take_if(|x| *x > 10), small);
    let pair = Some((1, "one"));
    let (a, b) = pair.unzip();
    println!("{:?} {:?}", a, b);
    let none: Option<(i32, char)> = None;
    println!("{:?}", none.unzip());

    // Result
    let ok = halve(10);
    let err = halve(7);
    println!("{:?} {:?}", ok.clone().and(halve(4)), err.clone().and(halve(4)));
    println!("{:?} {:?}", ok.clone().or(halve(3)), err.clone().or::<String>(Ok(0)));
    let text: Result<String, i32> = Ok("hi".to_string());
    println!("{:?}", text.as_deref());
    let mut owned: Result<Vec<i32>, i32> = Ok(vec![5, 4]);
    if let Ok(s) = owned.as_deref_mut() {
        s.reverse();
    }
    println!("{:?}", owned);
    let mut c = 'q';
    c.make_ascii_uppercase();
    let mut byte = b'Z';
    byte.make_ascii_lowercase();
    println!("{c} {byte}");
    let mut counts: Result<Vec<i32>, String> = Ok(vec![1]);
    if let Ok(v) = counts.as_mut() {
        bump(v);
    }
    println!("{:?} {:?}", counts.as_ref().map(|v| v.len()), counts);
    for v in counts.iter_mut() {
        v.push(9);
    }
    println!("{:?}", counts);
    let seen = halve(6).inspect(|v| println!("inspect saw {v}"));
    println!("{:?}", seen);
    println!("{} {}", ok.clone().map_or(-1, |v| v * 10), err.clone().map_or(-1, |v| v * 10));
    println!("{} {}", ok.clone().map_or_default(|v| v + 1), err.clone().map_or_default(|v| v + 1));

    // bool
    let mut flag = false;
    flag.toggle();
    println!("{flag} {:?} {:?}", flag.ok_or("no"), (!flag).ok_or("no"));
    let mut calls = 0;
    let r = false.ok_or_else(|| {
        calls += 1;
        format!("call {calls}")
    });
    println!("{:?} {}", r, true.ok_or_else(|| 0).is_ok());

    // Ordering
    for o in [Ordering::Less, Ordering::Equal, Ordering::Greater] {
        println!("{:?}: ge={} le={} ne={}", o, o.is_ge(), o.is_le(), o.is_ne());
    }

    // Explicit trait method calls
    let t1 = Tag { name: "x".into(), n: 1 };
    let t2 = Tag { name: "x".into(), n: 2 };
    println!("{} {}", t1.eq(&t1.clone()), t1.eq(&t2));
    println!("{} {} {} {}", 3.lt(&4), 3.le(&3), 5.gt(&9), "b".ge("a"));
    println!("{} {}", 2.5f64.lt(&1.0), 'a'.gt(&'Z'));
    let mut target = Tag { name: "old".into(), n: 0 };
    target.clone_from(&t2);
    println!("{:?}", target);
    let mut list = vec![3, 1, 2];
    let slice: &mut [i32] = list.as_mut();
    slice.sort();
    println!("{:?}", list);
    println!("{:?} {:?}", i32::from_str("42"), f64::from_str("2.5"));
    println!("{:?}", bool::from_str("maybe").is_err());
    println!("{} {:?}", identity(5), [Some(1), None, Some(3)].into_iter().filter_map(identity).collect::<Vec<_>>());
}
