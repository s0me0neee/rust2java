// Conway's Game of Life on a torus: fixed-size grids, a glider and a blinker,
// neighbor counting with wrap-around, and ASCII generations.
const WIDTH: i32 = 12;
const HEIGHT: i32 = 8;

type Grid = Vec<Vec<bool>>;

fn empty() -> Grid {
    vec![vec![false; WIDTH as usize]; HEIGHT as usize]
}

fn glider(grid: &mut Grid, top: i32, left: i32) {
    let cells = [(0, 1), (1, 2), (2, 0), (2, 1), (2, 2)];
    for (dr, dc) in cells {
        let r = (top + dr) % HEIGHT;
        let c = (left + dc) % WIDTH;
        grid[r as usize][c as usize] = true;
    }
}

fn blinker(grid: &mut Grid, row: i32, col: i32) {
    for dc in -1..=1 {
        let c = (col + dc + WIDTH) % WIDTH;
        grid[row as usize][c as usize] = true;
    }
}

fn neighbors(grid: &Grid, row: i32, col: i32) -> usize {
    let mut count = 0;
    for dr in -1..=1 {
        for dc in -1..=1 {
            if dr == 0 && dc == 0 {
                continue;
            }
            let r = (row + dr + HEIGHT) % HEIGHT;
            let c = (col + dc + WIDTH) % WIDTH;
            if grid[r as usize][c as usize] {
                count += 1;
            }
        }
    }
    count
}

fn step(grid: &Grid) -> Grid {
    let mut next = empty();
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            let alive = grid[row as usize][col as usize];
            let count = neighbors(grid, row, col);
            let survives = alive && (count == 2 || count == 3);
            let born = !alive && count == 3;
            next[row as usize][col as usize] = survives || born;
        }
    }
    next
}

fn population(grid: &Grid) -> usize {
    grid.iter()
        .map(|row| row.iter().filter(|cell| **cell).count())
        .sum()
}

fn render(grid: &Grid) -> Vec<String> {
    let mut lines = Vec::new();
    let border = format!("+{}+", "-".repeat(WIDTH as usize));
    lines.push(border.clone());
    for row in grid {
        let mut line = String::from("|");
        for cell in row {
            line.push(if *cell { '#' } else { '.' });
        }
        line.push('|');
        lines.push(line);
    }
    lines.push(border);
    lines
}

fn main() {
    let mut grid = empty();
    glider(&mut grid, 1, 1);
    blinker(&mut grid, 6, 7);
    grid[0][10] = true;
    grid[0][11] = true;
    grid[1][10] = true;
    for generation in 0..8 {
        let alive = population(&grid);
        println!("generation {}: {} alive", generation, alive);
        if generation < 2 || generation == 7 {
            for line in render(&grid) {
                println!("{}", line);
            }
        }
        grid = step(&grid);
    }
    let mut live_cells: Vec<(i32, i32)> = Vec::new();
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            if grid[row as usize][col as usize] {
                live_cells.push((row, col));
            }
        }
    }
    live_cells.sort();
    let coords: Vec<String> = live_cells
        .iter()
        .map(|(r, c)| format!("({},{})", r, c))
        .collect();
    println!("live cells: {}", coords.join(" "));
    println!("center is {} after 8 steps", grid[(HEIGHT / 2) as usize][(WIDTH / 2) as usize]);
    let stable = step(&grid) == grid;
    println!("stable: {}", stable);
    let mut histogram: Vec<usize> = Vec::new();
    for row in 0..HEIGHT {
        let alive = grid[row as usize].iter().filter(|cell| **cell).count();
        histogram.push(alive);
    }
    println!("row populations: {:?}", histogram);
    let densest = histogram.iter().max().copied().unwrap_or(0);
    println!("densest row has {} cells", densest);
    let mut counts = [0usize; 9];
    for row in 0..HEIGHT {
        for col in 0..WIDTH {
            counts[neighbors(&grid, row, col)] += 1;
        }
    }
    for (i, count) in counts.iter().enumerate() {
        println!("{} neighbors: {}", i, count);
    }
}
