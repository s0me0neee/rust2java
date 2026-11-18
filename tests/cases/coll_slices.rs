fn main() {
    let mut values = vec![1, 2, 3, 4];
    println!("{:?}", values.split_first());
    println!("{:?}", values.split_last());
    let (left, right) = values.split_at(2);
    println!("{:?} {:?}", left, right);
    println!("{:?}", right.to_vec());
    println!("{:?} {:?}", values.binary_search(&3), values.binary_search(&5));
    println!("{:?}", vec![1, 2, 2, 2, 3].binary_search(&2));
    values.rotate_left(1);
    println!("{:?}", values);
    values.fill(7);
    println!("{:?}", values);
    let empty: Vec<i32> = vec![];
    println!("{:?} {:?}", empty.split_first(), empty.split_last());
    let nested = vec![vec![1, 2], vec![3]];
    println!("{:?}", nested.concat());
    println!("{:?}", nested[0].chunks(1).map(|c| c.to_vec()).collect::<Vec<_>>());
    let mut copy = nested.clone();
    copy[0][0] = 99;
    println!("{:?} {:?}", nested, copy);
    let mut filled = vec![vec![0], vec![0]];
    filled.fill(vec![1, 2]);
    filled[0][0] = 8;
    println!("{:?}", filled);
}
