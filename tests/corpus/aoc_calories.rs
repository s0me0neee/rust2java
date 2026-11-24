use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let mut elves: Vec<u32> = input
        .split("\n\n")
        .map(|group| group.lines().map(|l| l.trim().parse::<u32>().unwrap()).sum())
        .collect();
    elves.sort_unstable_by(|a, b| b.cmp(a));
    println!("max: {}", elves[0]);
    println!("top3: {}", elves.iter().take(3).sum::<u32>());
    let (idx, _) = elves.iter().enumerate().min_by_key(|(_, c)| **c).unwrap();
    println!("smallest at rank {}", idx + 1);
}
