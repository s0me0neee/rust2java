// Hand-written algorithms over slices: quicksort with swap, merge sort with split_at/to_vec,
// insertion sort on &mut [T] generic over Ord, binary search returning Result, and selection.
use std::cmp::Ordering;

fn quicksort(v: &mut [i32]) {
    if v.len() <= 1 {
        return;
    }
    let pivot = v[v.len() - 1];
    let mut store = 0;
    for i in 0..v.len() - 1 {
        if v[i] < pivot {
            v.swap(i, store);
            store += 1;
        }
    }
    let last = v.len() - 1;
    v.swap(store, last);
    let (left, right) = v.split_at_mut(store);
    quicksort(left);
    quicksort(&mut right[1..]);
}

fn merge_sort(v: &[i64]) -> Vec<i64> {
    if v.len() < 2 {
        return v.to_vec();
    }
    let (a, b) = v.split_at(v.len() / 2);
    let (a, b) = (merge_sort(a), merge_sort(b));
    let mut out = Vec::with_capacity(v.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] <= b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    out.extend_from_slice(&a[i..]);
    out.extend_from_slice(&b[j..]);
    out
}

fn insertion_sort<T: Ord + Clone>(v: &mut [T]) {
    for i in 1..v.len() {
        let mut j = i;
        while j > 0 && v[j - 1] > v[j] {
            v.swap(j - 1, j);
            j -= 1;
        }
    }
}

fn search(v: &[i32], target: i32) -> Result<usize, usize> {
    let (mut lo, mut hi) = (0usize, v.len());
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        match v[mid].cmp(&target) {
            Ordering::Less => lo = mid + 1,
            Ordering::Greater => hi = mid,
            Ordering::Equal => return Ok(mid),
        }
    }
    Err(lo)
}

fn kth_smallest(v: &[i32], k: usize) -> Option<i32> {
    let mut c = v.to_vec();
    if k >= c.len() {
        return None;
    }
    c.select_nth_unstable(k);
    Some(c[k])
}

fn main() {
    let mut a = vec![9, -3, 5, 0, 12, 5, -8, 1];
    quicksort(&mut a);
    println!("{:?}", a);
    println!("{:?}", merge_sort(&[5, 2, 9, 1, 5, 6, -1]));
    let mut words = vec!["pear", "apple", "fig", "kiwi", "banana"];
    insertion_sort(&mut words);
    println!("{:?}", words);
    let mut tail = [3, 1, 2, 9, 7];
    insertion_sort(&mut tail[2..]);
    println!("{:?}", tail);
    for t in [5, 6, -10, 100] {
        println!("{} -> {:?} {:?}", t, search(&a, t), a.binary_search(&t).is_ok());
    }
    println!("{:?} {:?}", kth_smallest(&a, 2), kth_smallest(&a, 99));
    let mut pairs = vec![(2, 'b'), (1, 'z'), (2, 'a'), (1, 'y')];
    pairs.sort_by(|x, y| x.0.cmp(&y.0).then_with(|| y.1.cmp(&x.1)));
    println!("{:?}", pairs);
    let pos = a.partition_point(|&x| x < 5);
    println!("partition {} chunks {:?}", pos, a.chunks(3).map(|c| c.iter().sum::<i32>()).collect::<Vec<_>>());
    let (evens, odds): (Vec<i32>, Vec<i32>) = a.iter().partition(|&&x| x % 2 == 0);
    println!("{:?} {:?} rev {:?}", evens, odds, { let mut r = a.clone(); r.reverse(); r });
    a.rotate_left(3);
    println!("{:?} max {:?} min_by_key {:?}", a, a.iter().max(), a.iter().min_by_key(|x| x.abs()));
    let sorted = a.windows(2).all(|w| w[0] <= w[1]);
    a.sort_unstable_by(|x, y| y.cmp(x));
    a.dedup();
    println!("{} {:?} {}", sorted, a, a.contains(&12));
}
