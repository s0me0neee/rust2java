// A local that a closure reads and a later statement reassigns is not a cell: javac
// rejects the lambda capture ("must be effectively final").
fn main() {
    let mut rows = vec![vec![1, 2], vec![3, 4]];
    let sums: Vec<i32> = (0..2).map(|r| rows[r].iter().sum()).collect();
    println!("{:?}", sums);
    rows = vec![vec![9]];
    println!("{:?}", rows);
}
