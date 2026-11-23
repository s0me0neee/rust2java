// A single `&mut` primitive parameter becomes a return value: `a = bump(a);`.
struct Stats {
    hits: i32,
}

fn bump(n: &mut i32) {
    *n += 1;
}

fn clamp_to(v: &mut i64, max: i64) {
    if *v > max {
        *v = max;
        return;
    }
    *v += 1;
}

fn main() {
    let mut a = 1;
    bump(&mut a);
    bump(&mut a);

    let mut stats = Stats { hits: 0 };
    bump(&mut stats.hits);

    let mut big = 10i64;
    clamp_to(&mut big, 5);
    clamp_to(&mut big, 100);
    println!("{} {} {}", a, stats.hits, big);
}
