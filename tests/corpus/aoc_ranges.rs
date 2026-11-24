use std::io::{self, BufRead};

struct Range {
    lo: u32,
    hi: u32,
}

impl Range {
    fn parse(s: &str) -> Option<Range> {
        let (a, b) = s.split_once('-')?;
        Some(Range { lo: a.parse().ok()?, hi: b.parse().ok()? })
    }
    fn contains(&self, o: &Range) -> bool {
        self.lo <= o.lo && o.hi <= self.hi
    }
    fn overlaps(&self, o: &Range) -> bool {
        self.lo <= o.hi && o.lo <= self.hi
    }
}

fn main() {
    let stdin = io::stdin();
    let (mut full, mut any, mut bad) = (0, 0, 0);
    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let Some((l, r)) = line.split_once(',') else {
            bad += 1;
            continue;
        };
        match (Range::parse(l), Range::parse(r)) {
            (Some(a), Some(b)) => {
                if a.contains(&b) || b.contains(&a) {
                    full += 1;
                }
                if a.overlaps(&b) {
                    any += 1;
                }
            }
            _ => bad += 1,
        }
    }
    println!("{full} {any} {bad}");
}
