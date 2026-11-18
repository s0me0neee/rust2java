// Move closures copy captured values, including inside another closure.
fn main() {
    let mut x = 0;
    let make = || {
        let mut increment = move || {
            x += 1;
            x
        };
        increment()
    };
    println!("{} {} {}", make(), make(), x);

    // The local inside the outer closure is not one of its captures.
    let outer = move || {
        let mut n = 0;
        let mut increment = || {
            n += 1;
            n
        };
        increment();
        increment()
    };
    println!("{} {}", outer(), outer());

    let mut value = 2;
    let mut first = move || {
        value += 1;
        value
    };
    let mut second = move || {
        value += 10;
        value
    };
    println!("{} {} {} {} {}", first(), first(), second(), second(), value);
}
