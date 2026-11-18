// A bound written only on the impl: comparisons cast the Java `T` to Comparable.
struct Bag<T> {
    items: Vec<T>,
}

impl<T: Ord> Bag<T> {
    fn max_item(&self) -> Option<&T> {
        let mut best: Option<&T> = None;
        for item in &self.items {
            if best.is_none() || item > best.unwrap() {
                best = Some(item);
            }
        }
        best
    }
}

fn main() {
    let bag = Bag { items: vec![3, 1, 2] };
    println!("{:?}", bag.max_item());
}
