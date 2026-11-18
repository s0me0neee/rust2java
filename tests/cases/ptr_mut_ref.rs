// `let r = &mut x;` bindings: the local is the one-element array for primitives and Strings.
fn bump(n: &mut i32) {
    *n += 1;
}

fn main() {
    let mut x = 5;
    {
        let r = &mut x;
        *r += 1;
        *r *= 2;
        bump(r);
    }
    println!("{x}");

    let mut s = String::from("a");
    {
        let r = &mut s;
        r.push('b');
        *r += "c";
    }
    println!("{s}");

    let mut v = vec![1, 2];
    {
        let r = &mut v;
        r.push(3);
    }
    println!("{:?} {}", v, v.len());
}
