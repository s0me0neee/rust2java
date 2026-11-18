fn main() {
    let mut people = vec![("bob", 30), ("al", 25)];
    people.sort_by_key(|p| p.1);
    let k: Vec<i32> = people.iter().map(|p| p.1).collect();
    let mut nums = vec![(3, 'a'), (1, 'b')];
    nums.sort_by_key(|&(n, _)| n);
    println!("{:?} {:?} {:?}", people, k, nums);
}
