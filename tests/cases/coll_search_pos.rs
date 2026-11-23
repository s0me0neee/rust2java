// `binary_search` read for its position: insertion point, found/missing test, match on Ok/Err.
fn main() {
    let mut v = vec![1, 3, 5, 7];
    for x in [0, 3, 4, 9] {
        let at = v.binary_search(&x).unwrap_or_else(|i| i);
        println!("{x} -> {at} {}", v.binary_search(&x).is_err());
    }
    for x in [4, 5] {
        match v.binary_search(&x) {
            Ok(i) => println!("found {x} at {i}"),
            Err(i) => {
                v.insert(i, x);
                println!("inserted {x} at {i}");
            }
        }
    }
    println!("{:?}", v);
}
