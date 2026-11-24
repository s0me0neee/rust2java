use std::fmt::{Display, Formatter, Result};

const TABLE: [(u32, &str); 13] = [
    (1000, "M"), (900, "CM"), (500, "D"), (400, "CD"), (100, "C"), (90, "XC"),
    (50, "L"), (40, "XL"), (10, "X"), (9, "IX"), (5, "V"), (4, "IV"), (1, "I"),
];

pub struct Roman(u32);

impl Display for Roman {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        let mut n = self.0;
        for &(value, sym) in TABLE.iter() {
            while n >= value {
                f.write_str(sym)?;
                n -= value;
            }
        }
        Ok(())
    }
}

impl From<u32> for Roman {
    fn from(n: u32) -> Self {
        Roman(n)
    }
}

fn parse_roman(s: &str) -> u32 {
    let value = |c| TABLE.iter().find(|(_, sym)| sym.len() == 1 && sym.starts_with(c)).unwrap().0;
    let vals: Vec<u32> = s.chars().map(value).collect();
    let mut total = 0;
    for i in 0..vals.len() {
        if i + 1 < vals.len() && vals[i] < vals[i + 1] {
            total -= vals[i] as i32;
        } else {
            total += vals[i] as i32;
        }
    }
    total as u32
}

fn main() {
    for n in [1, 4, 9, 14, 40, 1990, 2024, 3999] {
        let r = Roman::from(n).to_string();
        println!("{n} = {r} = {}", parse_roman(&r));
    }
}
