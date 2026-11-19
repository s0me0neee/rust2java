// Grid BFS / flood fill with HashMap<(i32, i32), char>, VecDeque, HashSet of tuples, path reconstruction.
use std::collections::{HashMap, HashSet, VecDeque};

const MAZE: &str = "\
#########
#S..#...#
#.#.#.#.#
#.#...#.#
#.#####.#
#...#..E#
#########";

type Pos = (i32, i32);

fn parse(s: &str) -> (HashMap<Pos, char>, Pos, Pos) {
    let mut grid = HashMap::new();
    let (mut start, mut end) = ((0, 0), (0, 0));
    for (r, line) in s.lines().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            let p = (r as i32, c as i32);
            match ch {
                'S' => start = p,
                'E' => end = p,
                _ => {}
            }
            grid.insert(p, ch);
        }
    }
    (grid, start, end)
}

fn neighbors(p: Pos) -> impl Iterator<Item = Pos> {
    [(0, 1), (1, 0), (0, -1), (-1, 0)].into_iter().map(move |(dr, dc)| (p.0 + dr, p.1 + dc))
}

fn bfs(grid: &HashMap<Pos, char>, start: Pos, end: Pos) -> Option<Vec<Pos>> {
    let mut prev: HashMap<Pos, Pos> = HashMap::new();
    let mut seen: HashSet<Pos> = HashSet::from([start]);
    let mut q = VecDeque::from(vec![start]);
    while let Some(cur) = q.pop_front() {
        if cur == end {
            let mut path = vec![end];
            let mut at = end;
            while let Some(&p) = prev.get(&at) {
                path.push(p);
                at = p;
            }
            path.reverse();
            return Some(path);
        }
        for n in neighbors(cur) {
            if grid.get(&n).is_some_and(|&c| c != '#') && seen.insert(n) {
                prev.insert(n, cur);
                q.push_back(n);
            }
        }
    }
    None
}

fn regions(grid: &HashMap<Pos, char>) -> usize {
    let mut seen = HashSet::new();
    let mut count = 0;
    let mut cells: Vec<&Pos> = grid.keys().collect();
    cells.sort();
    for &p in cells {
        if grid[&p] == '#' || seen.contains(&p) {
            continue;
        }
        count += 1;
        let mut stack = vec![p];
        while let Some(x) = stack.pop() {
            if !seen.insert(x) {
                continue;
            }
            stack.extend(neighbors(x).filter(|n| grid.get(n).map_or(false, |&c| c != '#')));
        }
    }
    count
}

fn main() {
    let (mut grid, start, end) = parse(MAZE);
    println!("start {:?} end {:?} cells {}", start, end, grid.len());
    match bfs(&grid, start, end) {
        Some(path) => {
            println!("steps {}", path.len() - 1);
            for p in &path[1..path.len() - 1] {
                grid.insert(*p, '*');
            }
        }
        None => println!("no path"),
    }
    let rows = MAZE.lines().count() as i32;
    let cols = MAZE.lines().next().unwrap().len() as i32;
    for r in 0..rows {
        let line: String = (0..cols).map(|c| grid[&(r, c)]).collect();
        println!("{}", line);
    }
    println!("regions {}", regions(&grid));
    grid.insert((3, 3), '#');
    grid.insert((1, 3), '#');
    println!("{:?}", bfs(&grid, start, end).map(|p| p.len()));
    let walls = grid.values().filter(|&&c| c == '#').count();
    let mut counts: Vec<(char, usize)> = grid.values().fold(HashMap::new(), |mut m, &c| {
        *m.entry(c).or_insert(0usize) += 1;
        m
    }).into_iter().collect();
    counts.sort();
    println!("walls {} {:?}", walls, counts);
}
