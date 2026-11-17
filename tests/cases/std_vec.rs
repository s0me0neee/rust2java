// Vec, slice and array methods (std_map / §11.1).
fn main() {
    let mut v: Vec<i32> = Vec::new();
    v.push(3);
    v.push(1);
    v.push(4);
    println!("{} {}", v.len(), v.is_empty());
    println!("{:?} {:?}", v.get(1), v.get(9));
    println!("{:?} {:?}", v.first(), v.last());
    println!("{:?}", v.pop());
    println!("{:?}", v);
    v.insert(0, 9);
    println!("{:?}", v);
    println!("{}", v.remove(1));
    println!("{:?}", v);
    v.swap(0, 1);
    println!("{:?}", v);
    v.extend(vec![7, 8]);
    println!("{:?}", v);
    v.sort();
    println!("{:?}", v);
    v.reverse();
    println!("{:?}", v);
    v.retain(|x| *x % 2 == 0);
    println!("{:?}", v);

    let mut d = vec![1, 1, 2, 2, 2, 3];
    d.dedup();
    println!("{:?}", d);
    println!("{} {}", v.contains(&4), v.contains(&99));

    let mut s = vec![5, 2, 9, 1];
    s.sort_by(|a, b| b.cmp(a));
    println!("{:?}", s);
    let sorted = vec![1, 2, 5, 9];
    println!("{:?} {:?}", sorted.binary_search(&5), sorted.binary_search(&3));
    let (left, right) = s.split_at(2);
    println!("{:?} {:?}", left, right);

    let c = s.clone();
    println!("{:?}", c);
    println!("{:?} {:?}", &s[1..3], &s[2..]);

    let arr = [1, 2, 3];
    println!("{:?} {}", arr, arr.len());
    let zeros = [0u8; 3];
    println!("{:?}", zeros);
    let grid = [[0i32; 2]; 3];
    println!("{} {} {}", grid.len(), grid[0].len(), grid[2][1]);
    let mut g2 = [[1, 2], [3, 4]];
    g2[0][1] = 9;
    println!("{} {} {} {}", g2[0][0], g2[0][1], g2[1][0], g2[1][1]);

    let mut names = vec!["ann", "bob", "cy"];
    names.sort_by_key(|n| n.len());
    println!("{:?}", names);
    let total: i32 = s.iter().sum();
    println!("{}", total);
    println!("{:?}", s.iter().max());
    let mut counts = vec![1, 2, 3];
    for x in counts.iter_mut() {
        *x *= 2;
    }
    println!("{:?}", counts);
    counts.truncate(1);
    println!("{:?}", counts);
    counts.clear();
    println!("{}", counts.is_empty());
}
