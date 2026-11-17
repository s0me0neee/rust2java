fn bump(n: &mut i32) {
    *n += 1;
}

fn bump_twice(n: &mut i32) {
    bump(n);
    bump(n);
}

fn append(s: &mut String, suffix: &str) {
    *s += suffix;
}

fn main() {
    let mut count = 0;
    bump(&mut count);
    bump_twice(&mut count);
    println!("count = {count}");

    let mut total = 0;
    let mut add = |x: i32| total += x;
    add(5);
    add(7);
    println!("total = {}", total);

    let mut name = String::from("rust");
    append(&mut name, "2java");
    println!("{name}");

    let mut hits = 0;
    for c in 0..10 {
        let mut check = |limit: i32| {
            if c > limit {
                hits += 1;
            }
        };
        check(4);
    }
    println!("hits = {hits}");

    let mut base = 1;
    base += 1;
    let scale = |x: i32| x * base;
    println!("{} {}", scale(3), scale(5));
    let mut a = 1;
    let mut b = 2;
    std_swap(&mut a, &mut b);
    println!("{a} {b}");
}

fn std_swap(x: &mut i32, y: &mut i32) {
    let t = *x;
    *x = *y;
    *y = t;
}
