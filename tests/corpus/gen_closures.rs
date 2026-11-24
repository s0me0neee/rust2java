fn make_counter() -> impl FnMut() -> u32 {
    let mut n = 0;
    move || {
        n += 1;
        n
    }
}

fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |x| g(f(x))
}

fn apply_all(fs: &[Box<dyn Fn(i32) -> i32>], x: i32) -> Vec<i32> {
    fs.iter().map(|f| f(x)).collect()
}

fn main() {
    let mut c = make_counter();
    let mut d = make_counter();
    println!("{} {} {} {}", c(), c(), d(), c());
    let inc_then_double = compose(|x: i32| x + 1, |x| x * 2);
    let to_text = compose(inc_then_double, |x| format!("<{x}>"));
    println!("{}", to_text(4));
    let k = 10;
    let fs: Vec<Box<dyn Fn(i32) -> i32>> = vec![Box::new(|x| x + 1), Box::new(move |x| x * k), Box::new(i32::abs)];
    println!("{:?}", apply_all(&fs, -3));
    let mut log = Vec::new();
    let mut record = |msg: &str| log.push(msg.to_uppercase());
    record("start");
    record("stop");
    println!("{:?}", log);
    let mut words = vec!["pear", "fig", "banana", "kiwi", "apple"];
    words.sort_by_key(|w| (w.len(), *w));
    println!("{:?}", words);
    let lens: Vec<usize> = words.iter().map(|w| w.len()).collect();
    let total = lens.iter().fold(0, |acc, l| acc + l);
    println!("{total} {:?}", lens.iter().rposition(|&l| l == 4));
}
