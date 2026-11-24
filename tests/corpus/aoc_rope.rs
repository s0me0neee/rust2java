use std::collections::HashSet;

fn follow(head: (i32, i32), tail: (i32, i32)) -> (i32, i32) {
    let (dx, dy) = (head.0 - tail.0, head.1 - tail.1);
    if dx.abs() <= 1 && dy.abs() <= 1 {
        tail
    } else {
        (tail.0 + dx.signum(), tail.1 + dy.signum())
    }
}

fn simulate(moves: &[(char, u32)], knots: usize) -> usize {
    let mut rope = vec![(0, 0); knots];
    let mut seen = HashSet::new();
    seen.insert((0, 0));
    for &(dir, n) in moves {
        let (dx, dy) = match dir {
            'R' => (1, 0),
            'L' => (-1, 0),
            'U' => (0, 1),
            'D' => (0, -1),
            _ => unreachable!(),
        };
        for _ in 0..n {
            rope[0].0 += dx;
            rope[0].1 += dy;
            for i in 1..knots {
                rope[i] = follow(rope[i - 1], rope[i]);
            }
            seen.insert(*rope.last().unwrap());
        }
    }
    seen.len()
}

fn main() {
    let text = "R 4\nU 4\nL 3\nD 1\nR 4\nD 1\nL 5\nR 2";
    let moves: Vec<(char, u32)> = text
        .lines()
        .map(|l| {
            let (d, n) = l.split_once(' ').unwrap();
            (d.chars().next().unwrap(), n.parse().unwrap())
        })
        .collect();
    println!("{} {}", simulate(&moves, 2), simulate(&moves, 10));
}
