fn main() {
    let mut v = vec![5, 3, 8, 1, 9, 2];
    v.sort();
    println!("{:?}", v);
    v.sort_by(|a, b| b.cmp(a));
    println!("{:?}", v);
    v.sort_unstable();
    v.reverse();
    println!("{:?} {:?} {:?}", v.first(), v.last(), v.get(10));
    println!("{} {}", v.contains(&8), v.iter().position(|&x| x == 8).unwrap());
    println!("{:?}", v.binary_search(&3).is_ok());
    v.retain(|&x| x % 2 == 1);
    println!("{:?}", v);
    v.insert(1, 7);
    let removed = v.remove(0);
    println!("{} {:?}", removed, v);
    v.extend(vec![1, 1, 2]);
    v.dedup();
    println!("{:?}", v);
    v.swap(0, 1);
    println!("{:?} {}", v, v.len());
    let popped = v.pop();
    println!("{:?}", popped);
    v.truncate(2);
    println!("{:?}", v);
    let mut w = v.clone();
    w.push(100);
    println!("{:?} {:?}", v, w);
    let s = &w[1..];
    println!("{:?} {}", s, s.len());
    println!("{:?}", w.iter().rev().collect::<Vec<_>>());
    let mut people = vec![("bob", 30), ("al", 25), ("cy", 30)];
    people.sort_by_key(|p| p.1);
    println!("{:?}", people);
    people.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    println!("{:?}", people);
    let grid = vec![vec![0; 3]; 2];
    println!("{:?}", grid);
    let mut g2 = grid.clone();
    g2[1][2] = 5;
    println!("{:?} {}", g2, g2[1][2]);
    let total: i32 = w.iter().sum();
    println!("{}", total);
    println!("{}", w.is_empty());
    let joined = vec!["a", "b"].concat();
    println!("{}", joined);
    let mut arr = [3, 1, 2];
    arr.sort();
    println!("{:?} {}", arr, arr.len());
    let flat: Vec<i32> = vec![vec![1, 2], vec![3]].into_iter().flatten().collect();
    println!("{:?}", flat);
    println!("{:?}", flat.iter().copied().max());
    println!("{:?}", [1, 2, 3].iter().map(|x| x * 2).collect::<Vec<i32>>());
    let (left, right) = flat.split_at(1);
    println!("{:?} {:?}", left, right);
    println!("{:?}", flat.chunks(2).map(|c| c.to_vec()).collect::<Vec<_>>());
    println!("{}", flat.iter().any(|&x| x > 2));
    let mut d = vec![1, 2, 3, 4];
    d.drain(..2);
    println!("{:?}", d);
    println!("{:?}", d.split_first());
    println!("{:?}", vec![3, 1].iter().min());
    println!("{}", vec![1, 2] == vec![1, 2]);
    let mut m = vec![vec![1, 2], vec![3, 4]];
    for row in m.iter_mut() {
        row[0] *= 10;
    }
    println!("{:?}", m);
    let mut names = vec![String::from("b"), String::from("a")];
    names.sort();
    println!("{:?}", names);
    println!("{}", names.join(", "));
    let fill = vec![String::new(); 2];
    println!("{:?}", fill);
    let nested_len: usize = m.iter().map(|r| r.len()).sum();
    println!("{}", nested_len);
    v.clear();
    println!("{:?}", v);
    let mut stack = Vec::new();
    stack.push(1);
    while let Some(top) = stack.pop() {
        println!("{}", top);
    }
    println!("{:?}", [1, 5, 2].iter().max_by_key(|&&x| x));
    let mut sorted_f = vec![2.5, 1.5];
    sorted_f.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{:?}", sorted_f);
    println!("{:?}", vec![1, 2, 3].windows(2).map(|w| w[0] + w[1]).collect::<Vec<_>>());
    println!("{:?}", [0u8; 4]);
    let mut v2 = vec![1, 2, 3];
    if let Some(x) = v2.last_mut() {
        *x = 30;
    }
    if let Some(x) = v2.get_mut(0) {
        *x += 10;
    }
    println!("{:?}", v2);
    println!("{:?}", v2.iter().skip(1).step_by(1).collect::<Vec<_>>());
    println!("{}", v2.iter().fold(String::new(), |acc, x| acc + &x.to_string()));
}
