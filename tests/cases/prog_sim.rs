// Cellular simulation / state machine: 2D Vec<Vec<char>>, matches!, labeled breaks, wrapping math, chunks, zip, rev.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dir {
    N,
    E,
    S,
    W,
}

impl Dir {
    fn turn_right(self) -> Dir {
        match self {
            Dir::N => Dir::E,
            Dir::E => Dir::S,
            Dir::S => Dir::W,
            Dir::W => Dir::N,
        }
    }
    fn delta(self) -> (i32, i32) {
        match self {
            Dir::N => (-1, 0),
            Dir::E => (0, 1),
            Dir::S => (1, 0),
            Dir::W => (0, -1),
        }
    }
}

struct Robot {
    r: i32,
    c: i32,
    dir: Dir,
    steps: usize,
}

fn walk(grid: &mut Vec<Vec<char>>, bot: &mut Robot) -> bool {
    let h = grid.len() as i32;
    let w = grid[0].len() as i32;
    'outer: loop {
        for _ in 0..4 {
            let (dr, dc) = bot.dir.delta();
            let (nr, nc) = (bot.r + dr, bot.c + dc);
            if nr < 0 || nc < 0 || nr >= h || nc >= w {
                break 'outer true;
            }
            if grid[nr as usize][nc as usize] == '#' {
                bot.dir = bot.dir.turn_right();
                continue;
            }
            bot.r = nr;
            bot.c = nc;
            bot.steps += 1;
            grid[nr as usize][nc as usize] = 'X';
            if bot.steps > 100 {
                break 'outer false;
            }
            continue 'outer;
        }
        break false;
    }
}

fn checksum(data: &[u8]) -> u32 {
    let mut h: u32 = 2166136261;
    for &b in data {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

fn main() {
    let map = "\
.....#....
....#....#
..........
..#.......
.......#..
..........
.#..^.....
........#.
#.........
......#...";
    let mut grid: Vec<Vec<char>> = map.lines().map(|l| l.chars().collect()).collect();
    let (r, c) = grid
        .iter()
        .enumerate()
        .find_map(|(r, row)| row.iter().position(|&ch| ch == '^').map(|c| (r, c)))
        .unwrap();
    let mut bot = Robot { r: r as i32, c: c as i32, dir: Dir::N, steps: 0 };
    grid[r][c] = 'X';
    let escaped = walk(&mut grid, &mut bot);
    let visited = grid.iter().flatten().filter(|&&ch| ch == 'X').count();
    println!("escaped={} steps={} visited={} final={:?}", escaped, bot.steps, visited, bot.dir);
    for row in grid.iter().take(3) {
        println!("{}", row.iter().collect::<String>());
    }

    let bytes = map.as_bytes();
    println!("{:08x} {}", checksum(bytes), checksum(b"hello"));
    let sums: Vec<u32> = bytes.chunks(17).map(|ch| ch.iter().map(|&b| b as u32).sum()).collect();
    println!("{:?}", &sums[..4]);
    let pairs: Vec<String> = sums.iter().zip(sums.iter().skip(1)).map(|(a, b)| format!("{}", *b as i64 - *a as i64)).collect();
    println!("{}", pairs.join(" "));
    let rev: String = "stressed".chars().rev().collect();
    println!("{} {}", rev, u8::MAX.wrapping_add(3));
    let x: i32 = -17;
    println!("{} {} {} {}", x / 5, x % 5, x.rem_euclid(5), x.div_euclid(5));
    println!("{} {:?} {:?}", i32::MAX.saturating_add(5), 250u8.checked_add(10), 5u32.checked_sub(7));
    println!("{} {}", (-8i32).pow(3), 2u64.pow(40));
    let walls: usize = grid.iter().map(|row| row.iter().filter(|c| matches!(c, '#')).count()).sum();
    println!("walls {}", walls);
}
