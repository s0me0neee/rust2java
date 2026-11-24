fn main() {
    let grid: Vec<Vec<u8>> = "30373\n25512\n65332\n33549\n35390"
        .lines()
        .map(|l| l.bytes().map(|b| b - b'0').collect())
        .collect();
    let (h, w) = (grid.len(), grid[0].len());
    let mut visible = 0;
    let mut best = 0;
    for r in 0..h {
        for c in 0..w {
            let t = grid[r][c];
            let left = (0..c).rev().map(|x| grid[r][x]);
            let right = (c + 1..w).map(|x| grid[r][x]);
            let up = (0..r).rev().map(|y| grid[y][c]);
            let down = (r + 1..h).map(|y| grid[y][c]);
            let dirs: Vec<Vec<u8>> = vec![left.collect(), right.collect(), up.collect(), down.collect()];
            if dirs.iter().any(|d| d.iter().all(|&x| x < t)) {
                visible += 1;
            }
            let score: usize = dirs
                .iter()
                .map(|d| match d.iter().position(|&x| x >= t) {
                    Some(i) => i + 1,
                    None => d.len(),
                })
                .product();
            best = best.max(score);
        }
    }
    println!("visible={visible} best={best}");
}
