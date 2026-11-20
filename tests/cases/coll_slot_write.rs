// Writes through `get_mut(..).unwrap()`, `last_mut`/`first_mut`, `get_mut(..)?` lets, and
// `IntoIterator` / `impl Iterator` params fed collections and iterator structs.
use std::collections::HashMap;

struct Countdown(u32);

impl Iterator for Countdown {
    type Item = u32;
    fn next(&mut self) -> Option<u32> {
        if self.0 == 0 {
            None
        } else {
            self.0 -= 1;
            Some(self.0 + 1)
        }
    }
}

fn total<I: IntoIterator<Item = i32>>(items: I) -> i32 {
    items.into_iter().sum()
}

fn names(xs: impl IntoIterator<Item = String>) -> String {
    let mut out = String::new();
    for x in xs {
        out.push_str(&x);
    }
    out
}

fn evens(it: impl Iterator<Item = u32>) -> Vec<u32> {
    it.filter(|x| x % 2 == 0).collect()
}

fn bump(m: &mut HashMap<String, i64>, k: &str) -> Option<i64> {
    let v = m.get_mut(k)?;
    *v += 100;
    Some(*v)
}

fn take(m: &mut HashMap<String, i64>, k: &str, n: i64) -> Result<i64, String> {
    let v = m.get_mut(k).ok_or(format!("no {}", k))?;
    *v -= n;
    Ok(*v)
}

fn main() {
    let mut m: HashMap<String, i64> = HashMap::new();
    m.insert("a".to_string(), 1);
    *m.get_mut("a").unwrap() += 4;
    *m.get_mut("a").expect("present") *= 3;
    println!("{:?} {:?}", bump(&mut m, "a"), bump(&mut m, "zz"));
    println!("{:?} {:?}", take(&mut m, "a", 15), take(&mut m, "b", 1));
    let mut v = vec![1, 2, 3];
    *v.last_mut().unwrap() *= 10;
    *v.first_mut().unwrap() -= 1;
    *v.get_mut(1).unwrap() = 7;
    println!("{:?} {}", v, m["a"]);
    println!("{} {} {}", total(v.clone()), total([4, 5]), total(1..=4));
    println!("{}", names(vec!["x".to_string(), "y".to_string()]));
    println!("{:?} {:?}", evens(Countdown(6)), evens(1..9u32));
    let mut s = String::from("abc");
    s.extend(['d', 'e']);
    s.extend(vec!["fg", "h"]);
    s.replace_range(1..3, "XY");
    s.replace_range(..1, "");
    println!("{}", s);
    let r: Result<Option<i32>, String> = Some(Ok::<i32, String>(3)).transpose();
    let e: Result<Option<i32>, String> = Some(Err::<i32, String>("bad".into())).transpose();
    let n: Result<Option<i32>, String> = None.transpose();
    println!("{:?} {:?} {:?}", r, e, n);
    let powers: Vec<f64> = [1.0, 2.0, 3.0].iter().map(|&x| x).map(f64::sqrt).collect();
    let hyp = [3.0f64, 5.0].iter().copied().fold(0.0, f64::hypot);
    println!("{:.3?} {}", powers, hyp);
}
