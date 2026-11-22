fn main() {
    let grid = vec![vec![1, 2, 3], vec![4, 5, 6]];
    let dirs = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)];
    for r in 0..2usize {
        for c in 0..3usize {
            let mut s = 0;
            for (dr, dc) in dirs {
                let nr = (r as i32 + dr) as usize;
                if let Some(row) = grid.get(nr) {
                    if let Some(v) = row.get((c as i32 + dc) as usize) {
                        s += v;
                    }
                }
            }
            print!("{s} ");
        }
    }
    println!();
    let v = vec![10, 20, 30];
    let i: usize = 0;
    println!("{:?} {:?} {:?}", v.get(i.wrapping_sub(1)), v.get(i + 1), v.get(v.len() - 1));
    let mut k = 2usize;
    let mut out = vec![];
    while let Some(x) = v.get(k) { out.push(*x); k = k.wrapping_sub(1); }
    println!("{:?}", out);
}
