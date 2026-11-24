fn find_saddle_points(input: &[Vec<u64>]) -> Vec<(usize, usize)> {
    let mut points = Vec::new();
    for (r, row) in input.iter().enumerate() {
        for (c, &v) in row.iter().enumerate() {
            let row_max = row.iter().max() == Some(&v);
            let col_min = input.iter().map(|row| row[c]).min() == Some(v);
            if row_max && col_min {
                points.push((r, c));
            }
        }
    }
    points
}

fn main() {
    let m = vec![vec![9, 8, 7], vec![5, 3, 2], vec![6, 6, 7]];
    println!("{:?}", find_saddle_points(&m));
    let flat = vec![vec![2, 2], vec![2, 2]];
    println!("{:?}", find_saddle_points(&flat));
    let empty: Vec<Vec<u64>> = vec![vec![]];
    println!("{:?}", find_saddle_points(&empty));
}
