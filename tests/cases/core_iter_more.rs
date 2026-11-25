// Iterator / DoubleEndedIterator / ExactSizeIterator methods, Sum / Product, and the free
// functions of `iter` and `cmp`.
use std::cmp;
use std::iter;

fn main() {
    let a = [1, 2, 3];
    let b = [1, 2, 4];
    println!("{:?} {:?}", a.iter().cmp(b.iter()), b.iter().cmp(a.iter().take(2)));
    println!("{} {} {} {}", a.iter().lt(b.iter()), a.iter().le(a.iter()), a.iter().gt(b.iter()), b.iter().ge(a.iter()));
    println!("{} {}", a.iter().ne(b.iter()), a.iter().ne(a.iter()));
    let f = [1.0, f64::NAN];
    println!("{:?} {:?}", [1.0, 2.0].iter().partial_cmp([1.0, 3.0].iter()), f.iter().partial_cmp([1.0, 2.0].iter()));

    let cycled: Vec<i32> = a.iter().cycle().take(7).copied().collect();
    println!("{:?}", cycled);
    let mut fused = a.iter().fuse();
    fused.next();
    println!("{:?} {:?}", fused.next(), a.iter().fuse().size_hint());
    let text = "héllo";
    println!("{:?} {:?} {:?}", text.chars().size_hint(), text.bytes().skip(2).size_hint(), (0..10).step_by(3).size_hint());
    println!("{:?} {:?}", a.iter().chain(b.iter()).size_hint(), a.iter().zip(1..).take(2).size_hint());
    let mut seen = Vec::new();
    let total: i32 = a.iter().inspect(|x| seen.push(**x * 10)).sum();
    println!("{} {:?}", total, seen);
    println!("{} {} {}", a.iter().is_sorted(), [3, 1].iter().is_sorted(), Vec::<i32>::new().iter().is_sorted());
    let words = ["bb", "a", "ccc"];
    println!("{} {}", words.iter().is_sorted_by(|x, y| x.len() <= y.len()), words.iter().is_sorted_by_key(|w| w.len()));
    println!("{:?} {:?}", words.iter().min_by(|x, y| x.len().cmp(&y.len())), words.iter().min_by(|x, y| y.cmp(x)));
    println!("{:?} {:?}", (1..10).size_hint(), a.iter().filter(|x| **x > 1).size_hint());
    let summed = a.iter().try_fold(0i32, |acc, &x| acc.checked_add(x));
    let overflow = [i32::MAX, 1].iter().try_fold(0i32, |acc, &x| acc.checked_add(x));
    println!("{:?} {:?}", summed, overflow);
    let parsed: Result<i32, String> = ["1", "2", "x"].iter().try_fold(0, |acc, s| {
        s.parse::<i32>().map(|n| acc + n).map_err(|_| format!("bad {s}"))
    });
    println!("{:?}", parsed);
    let mut printed = Vec::new();
    let stopped = [5, 6, 7].iter().try_for_each(|&x| {
        if x == 7 {
            return Err(x);
        }
        printed.push(x);
        Ok(())
    });
    println!("{:?} {:?}", stopped, printed);

    // DoubleEndedIterator, ExactSizeIterator
    let v = vec![10, 20, 30, 40];
    println!("{:?} {:?}", v.iter().nth_back(1), v.iter().nth_back(9));
    println!("{:?} {:?}", v.iter().rfind(|x| **x < 35), v.iter().rfind(|x| **x > 99));
    println!("{}", v.iter().rfold(String::new(), |s, x| format!("{s}{x},")));
    println!("{:?}", v.iter().try_rfold(0i32, |acc, &x| if x > 15 { Some(acc + x) } else { None }));
    println!("{} {}", v.iter().len(), v.iter().skip(1).len());

    // Sum / Product called through the traits
    let s: i32 = iter::Sum::sum(v.iter());
    let p: i64 = iter::Product::product([2i64, 3, 4].iter());
    println!("{s} {p}");

    // iter:: functions
    let joined: Vec<i32> = iter::chain([1, 2], [3]).collect();
    let pairs: Vec<(i32, char)> = iter::zip([1, 2, 3], ['a', 'b']).collect();
    println!("{:?} {:?}", joined, pairs);
    let once: Vec<String> = iter::once_with(|| "lazy".to_string()).collect();
    let repeated: Vec<&str> = iter::repeat_n("ab", 3).collect();
    let mut n = 0;
    let counted: Vec<i32> = iter::repeat_with(|| {
        n += 2;
        n
    })
    .take(4)
    .collect();
    println!("{:?} {:?} {:?}", once, repeated, counted);

    // cmp:: functions
    println!("{} {}", cmp::max_by(3, -5, |x: &i32, y: &i32| x.abs().cmp(&y.abs())), cmp::min_by(3, -5, |x: &i32, y: &i32| x.abs().cmp(&y.abs())));
    println!("{} {}", cmp::max_by_key("pear", "fig", |s| s.len()), cmp::min_by_key(-7, 4, |x: &i32| x.abs()));
}
