use std::collections::HashSet;

fn priority(c: char) -> u32 {
    match c {
        'a'..='z' => c as u32 - 'a' as u32 + 1,
        'A'..='Z' => c as u32 - 'A' as u32 + 27,
        _ => 0,
    }
}

fn main() {
    let sacks = vec![
        "vJrwpWtwJgWrhcsFMMfFFhFp",
        "jqHRNqRjqzjGDLGLrsFMfFZSrLrFZsSL",
        "PmmdzqPrVvPwwTWBwg",
        "wMqvLMZHhHMvwLHjbvcjnnSBnvTQFn",
        "ttgJtRGJQctTZtZT",
        "CrZsJsPPZsGzwwsLwLmpwMDw",
    ];
    let mut part1 = 0;
    for s in &sacks {
        let (a, b) = s.split_at(s.len() / 2);
        let left: HashSet<char> = a.chars().collect();
        let common = b.chars().find(|c| left.contains(c)).unwrap();
        part1 += priority(common);
    }
    println!("part1 {}", part1);
    let mut part2 = 0;
    for group in sacks.chunks_exact(3) {
        let sets: Vec<HashSet<char>> = group.iter().map(|s| s.chars().collect()).collect();
        let badge = sets[0]
            .iter()
            .filter(|c| sets[1].contains(c) && sets[2].contains(c))
            .next()
            .copied()
            .unwrap();
        println!("badge {badge}");
        part2 += priority(badge);
    }
    println!("part2 {}", part2);
}
