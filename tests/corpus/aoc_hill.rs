use std::collections::VecDeque;

fn bfs(grid: &[Vec<u8>], starts: &[(usize, usize)], end: (usize, usize)) -> Option<usize> {
    let (h, w) = (grid.len(), grid[0].len());
    let mut dist = vec![vec![usize::MAX; w]; h];
    let mut q = VecDeque::new();
    for &s in starts {
        dist[s.0][s.1] = 0;
        q.push_back(s);
    }
    while let Some((r, c)) = q.pop_front() {
        if (r, c) == end {
            return Some(dist[r][c]);
        }
        let dirs: [(isize, isize); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];
        for (dr, dc) in dirs {
            let nr = r as isize + dr;
            let nc = c as isize + dc;
            if nr < 0 || nc < 0 || nr >= h as isize || nc >= w as isize {
                continue;
            }
            let (nr, nc) = (nr as usize, nc as usize);
            if grid[nr][nc] <= grid[r][c] + 1 && dist[nr][nc] == usize::MAX {
                dist[nr][nc] = dist[r][c] + 1;
                q.push_back((nr, nc));
            }
        }
    }
    None
}

fn main() {
    let text = "Sabqponm\nabcryxxl\naccszExk\nacctuvwj\nabdefghi";
    let mut start = (0, 0);
    let mut end = (0, 0);
    let mut grid: Vec<Vec<u8>> = Vec::new();
    for (r, line) in text.lines().enumerate() {
        let mut row = Vec::new();
        for (c, ch) in line.chars().enumerate() {
            let h = match ch {
                'S' => {
                    start = (r, c);
                    b'a'
                }
                'E' => {
                    end = (r, c);
                    b'z'
                }
                _ => ch as u8,
            };
            row.push(h);
        }
        grid.push(row);
    }
    let lows: Vec<(usize, usize)> = (0..grid.len())
        .flat_map(|r| (0..grid[0].len()).map(move |c| (r, c)))
        .filter(|&(r, c)| grid[r][c] == b'a')
        .collect();
    println!("{:?} {:?}", bfs(&grid, &[start], end), bfs(&grid, &lows, end));
}
