// Java 21 SequencedCollection accessors for Vec: first/last/get/pop.
fn main() {
    let mut v = vec![10, 20, 30];
    println!("{:?} {:?} {:?}", v.first(), v.last(), v.get(1));
    println!("{:?} {:?}", v.get(3), v.get(0));

    let empty: Vec<i32> = Vec::new();
    println!("{:?} {:?} {:?}", empty.first(), empty.last(), empty.get(0));

    while let Some(x) = v.pop() {
        println!("{}", x);
    }
    println!("{:?} {:?} {:?}", v, v.first(), v.last());

    let mut w = vec![1, 2, 3];
    if let Some(x) = w.last_mut() {
        *x += 10;
    }
    if let Some(x) = w.first_mut() {
        *x += 1;
    }
    println!("{:?}", w);

    let idx = 1;
    println!("{:?} {:?}", w.get(idx), w.get(idx + 5));
    println!("{} {}", w[w.len() - 1], w[0]);
    let picked: Option<&i32> = w.get(2);
    println!("{:?}", picked);

    let mut names = vec![String::from("alice"), String::from("bob")];
    println!("{:?} {:?}", names.first(), names.last());
    if let Some(n) = names.last_mut() {
        n.push('!');
    }
    println!("{:?}", names);
}
