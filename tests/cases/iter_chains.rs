use std::collections::{HashMap, HashSet};

fn main() {
    let words = vec!["apple", "bob", "kiwi", "banana", "fig"];
    let long: Vec<usize> = words.iter().filter(|w| w.len() > 3).map(|w| w.len()).collect();
    println!("{:?}", long);
    let lens: HashMap<&str, usize> = words.iter().map(|w| (*w, w.len())).collect();
    println!("{}", lens["kiwi"]);
    let letters: HashSet<char> = words.iter().flat_map(|w| w.chars()).collect();
    let n_letters = letters.iter().count();
    println!("{}", n_letters);
    let joined = words.iter().map(|w| w.to_string()).collect::<Vec<String>>().join("-");
    println!("{joined}");
    let initials: String = words.iter().flat_map(|w| w.chars().next()).collect();
    println!("{initials}");
    let longest = words.iter().max_by_key(|w| w.len());
    let shortest = words.iter().min_by_key(|w| w.len());
    println!("{:?} {:?}", longest, shortest);
    let pairs: Vec<(i32, char)> = vec![1, 2, 3].into_iter().zip("xyz".chars()).collect();
    println!("{:?}", pairs.len());
    for (n, c) in pairs {
        print!("{n}{c} ");
    }
    println!();
    let parsed: Vec<usize> = "4 8 15 16".split(' ').filter_map(|s| if s.len() > 1 { Some(s.len()) } else { None }).collect();
    println!("{:?}", parsed);
    let nested = vec![vec![1, 2], vec![3], vec![]];
    let flat: Vec<i32> = nested.into_iter().flatten().collect();
    println!("{:?}", flat);
    let chained: Vec<i32> = (1..3).chain(7..9).collect();
    println!("{:?}", chained);
    let mut log = Vec::new();
    let out: Vec<i32> = (1..=4)
        .map(|x| {
            log.push(format!("map {x}"));
            x * 10
        })
        .filter(|x| *x != 20)
        .collect();
    println!("{:?} {:?}", out, log);
    let windows: Vec<i32> = [1, 4, 9, 16].windows(2).map(|w| w[1] - w[0]).collect();
    println!("{:?}", windows);
    let chunks: Vec<usize> = [1, 2, 3, 4, 5].chunks(2).map(|c| c.len()).collect();
    println!("{:?}", chunks);
    let total: f64 = vec![0.1, 0.2, 0.3].iter().sum();
    println!("{total}");
    let indexed: Vec<(usize, char)> = "abc".char_indices().collect();
    println!("{}", indexed.len());
}
