// Grid search: parse a map from text, run BFS with a VecDeque and DFS with an
// explicit stack, track visited cells in HashSet<(i32, i32)>, and redraw paths.
use std::collections::{HashMap, HashSet, VecDeque};

const MAP: &str = "\
#########
#S..#...#
#.#.#.#.#
#.#...#.#
#.#####.#
#.....#G#
#########";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Cell {
    row: i32,
    col: i32,
}

impl Cell {
    fn new(row: i32, col: i32) -> Cell {
        Cell { row, col }
    }

    fn neighbors(&self) -> Vec<Cell> {
        let mut out = Vec::with_capacity(4);
        for (dr, dc) in [(0, 1), (1, 0), (0, -1), (-1, 0)] {
            out.push(Cell::new(self.row + dr, self.col + dc));
        }
        out
    }
}

struct Grid {
    cells: Vec<Vec<char>>,
    rows: i32,
    cols: i32,
    start: Cell,
    goal: Cell,
}

impl Grid {
    fn parse(text: &str) -> Grid {
        let mut cells = Vec::new();
        let mut start = Cell::new(0, 0);
        let mut goal = Cell::new(0, 0);
        for (r, line) in text.lines().enumerate() {
            let mut row = Vec::new();
            for (c, ch) in line.chars().enumerate() {
                match ch {
                    'S' => start = Cell::new(r as i32, c as i32),
                    'G' => goal = Cell::new(r as i32, c as i32),
                    _ => {}
                }
                row.push(ch);
            }
            cells.push(row);
        }
        let rows = cells.len() as i32;
        let cols = cells.first().map(|row| row.len()).unwrap_or(0) as i32;
        Grid { cells, rows, cols, start, goal }
    }

    fn contains(&self, cell: Cell) -> bool {
        cell.row >= 0 && cell.row < self.rows && cell.col >= 0 && cell.col < self.cols
    }

    fn is_open(&self, cell: Cell) -> bool {
        self.contains(cell) && self.cells[cell.row as usize][cell.col as usize] != '#'
    }

    fn draw(&self, path: &HashSet<Cell>) -> Vec<String> {
        let mut lines = Vec::new();
        for r in 0..self.rows {
            let mut line = String::new();
            for c in 0..self.cols {
                let cell = Cell::new(r, c);
                let ch = self.cells[r as usize][c as usize];
                if path.contains(&cell) && ch != 'S' && ch != 'G' {
                    line.push('*');
                } else {
                    line.push(ch);
                }
            }
            lines.push(line);
        }
        lines
    }
}

fn bfs(grid: &Grid) -> Option<Vec<Cell>> {
    let mut queue = VecDeque::new();
    let mut seen: HashSet<Cell> = HashSet::new();
    let mut parent: HashMap<Cell, Cell> = HashMap::new();
    queue.push_back(grid.start);
    seen.insert(grid.start);
    let mut visit_order = Vec::new();
    while let Some(cell) = queue.pop_front() {
        visit_order.push(cell);
        if cell == grid.goal {
            let mut path = vec![cell];
            let mut current = cell;
            while let Some(prev) = parent.get(&current) {
                path.push(*prev);
                current = *prev;
            }
            path.reverse();
            println!("bfs visited {} cells", visit_order.len());
            return Some(path);
        }
        for next in cell.neighbors() {
            if grid.is_open(next) && !seen.contains(&next) {
                seen.insert(next);
                parent.insert(next, cell);
                queue.push_back(next);
            }
        }
    }
    println!("bfs visited {} cells", visit_order.len());
    None
}

fn dfs_components(grid: &Grid) -> Vec<Vec<Cell>> {
    let mut seen: HashSet<Cell> = HashSet::new();
    let mut components = Vec::new();
    for r in 0..grid.rows {
        for c in 0..grid.cols {
            let start = Cell::new(r, c);
            if !grid.is_open(start) || seen.contains(&start) {
                continue;
            }
            let mut stack = vec![start];
            let mut component = Vec::new();
            seen.insert(start);
            while let Some(cell) = stack.pop() {
                component.push(cell);
                for next in cell.neighbors() {
                    if grid.is_open(next) && !seen.contains(&next) {
                        seen.insert(next);
                        stack.push(next);
                    }
                }
            }
            components.push(component);
        }
    }
    components
}

fn manhattan(a: Cell, b: Cell) -> i32 {
    (a.row - b.row).abs() + (a.col - b.col).abs()
}

fn main() {
    let grid = Grid::parse(MAP);
    println!("grid {}x{}, start {:?}, goal {:?}", grid.rows, grid.cols, grid.start, grid.goal);
    println!("manhattan distance: {}", manhattan(grid.start, grid.goal));
    match bfs(&grid) {
        Some(path) => {
            println!("shortest path: {} steps", path.len() - 1);
            let cells: Vec<String> = path
                .iter()
                .map(|cell| format!("({},{})", cell.row, cell.col))
                .collect();
            println!("{}", cells.join(" "));
            let path_set: HashSet<Cell> = path.iter().copied().collect();
            for line in grid.draw(&path_set) {
                println!("{}", line);
            }
            let turns = path
                .windows(2)
                .filter(|pair| pair[0].row != pair[1].row)
                .count();
            println!("vertical moves: {}", turns);
        }
        None => println!("unreachable"),
    }
    let components = dfs_components(&grid);
    println!("{} open components", components.len());
    let mut sizes: Vec<usize> = components.iter().map(|c| c.len()).collect();
    sizes.sort();
    println!("component sizes: {:?}", sizes);
    let biggest = components
        .iter()
        .max_by_key(|component| component.len())
        .map(|component| component[0]);
    println!("first cell of biggest component: {:?}", biggest);
    let counts: HashMap<char, usize> = MAP.chars().fold(HashMap::new(), |mut acc, ch| {
        if ch != '\n' {
            *acc.entry(ch).or_insert(0) += 1;
        }
        acc
    });
    let mut tally: Vec<(char, usize)> = counts.into_iter().collect();
    tally.sort();
    println!("{:?}", tally);
}
