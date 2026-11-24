fn find<T: Ord, A: AsRef<[T]>>(array: A, key: T) -> Option<usize> {
    let arr = array.as_ref();
    let (mut lo, mut hi) = (0, arr.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match arr[mid].cmp(&key) {
            std::cmp::Ordering::Equal => return Some(mid),
            std::cmp::Ordering::Less => lo = mid + 1,
            std::cmp::Ordering::Greater => hi = mid,
        }
    }
    None
}

fn main() {
    let v = vec![1, 3, 4, 6, 8, 9, 11];
    for k in [6, 1, 11, 7, 0, 12] {
        println!("{k}: {:?}", find(&v, k));
    }
    println!("{:?}", find(["a", "c", "e"], "c"));
    println!("{:?}", find(Vec::<i32>::new(), 1));
}
