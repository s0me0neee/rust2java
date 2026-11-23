use std::{
    collections::{HashMap, HashSet},
    io::{self, Stdin},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut lines = io::stdin().lines();
    let t = lines.next().unwrap()?.parse::<usize>()?;
    for _ in 0..t {
        let cs = lines
            .next()
            .unwrap()?
            .split(" ")
            .map(|c| c.parse::<u32>().unwrap())
            .collect::<Vec<u32>>();
        let mut hash = HashSet::new();
        for _ in 0..cs[0] {
            hash.insert(lines.next().unwrap()?);
        }
        for _ in 0..cs[1] {
            hash.remove(&lines.next().unwrap()?);
        }
        for s in hash.iter() {
            println!("{}", s);
        }
    }
    Ok(())
}
