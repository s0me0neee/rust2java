fn annotate(field: &[&str]) -> Vec<String> {
    let grid: Vec<&[u8]> = field.iter().map(|r| r.as_bytes()).collect();
    let rows = grid.len() as i32;
    grid.iter()
        .enumerate()
        .map(|(r, row)| {
            row.iter()
                .enumerate()
                .map(|(c, &cell)| {
                    if cell == b'*' {
                        return '*';
                    }
                    let mut n = 0;
                    for dr in -1..=1 {
                        for dc in -1..=1 {
                            let (nr, nc) = (r as i32 + dr, c as i32 + dc);
                            if nr >= 0 && nr < rows && nc >= 0 && (nc as usize) < row.len() && grid[nr as usize][nc as usize] == b'*' {
                                n += 1;
                            }
                        }
                    }
                    if n == 0 { ' ' } else { char::from_digit(n, 10).unwrap() }
                })
                .collect()
        })
        .collect()
}

fn main() {
    let field = ["  *  ", "  *  ", "*****", "  *  ", "  *  "];
    for line in annotate(&field) {
        println!("|{line}|");
    }
    println!("{:?}", annotate(&[]));
}
