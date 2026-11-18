fn main() {
    let mut people = vec![("bob", 30), ("al", 25), ("cy", 30)];
    people.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    println!("{:?}", people);
}
