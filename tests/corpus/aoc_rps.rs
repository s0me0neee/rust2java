#[derive(Clone, Copy, PartialEq, Debug)]
enum Shape {
    Rock,
    Paper,
    Scissors,
}

impl Shape {
    fn parse(c: char) -> Shape {
        match c {
            'A' | 'X' => Shape::Rock,
            'B' | 'Y' => Shape::Paper,
            'C' | 'Z' => Shape::Scissors,
            _ => panic!("bad shape {c}"),
        }
    }

    fn score(self) -> u32 {
        self as u32 + 1
    }

    fn beats(self) -> Shape {
        match self {
            Shape::Rock => Shape::Scissors,
            Shape::Paper => Shape::Rock,
            Shape::Scissors => Shape::Paper,
        }
    }
}

fn outcome(them: Shape, me: Shape) -> u32 {
    if me.beats() == them {
        6
    } else if me == them {
        3
    } else {
        0
    }
}

fn main() {
    let rounds = "A Y\nB X\nC Z\nA Z\nC X";
    let mut total = 0;
    for line in rounds.lines() {
        let mut chars = line.chars().filter(|c| !c.is_whitespace());
        let them = Shape::parse(chars.next().unwrap());
        let me = Shape::parse(chars.next().unwrap());
        let s = me.score() + outcome(them, me);
        println!("{:?} vs {:?}: {}", me, them, s);
        total += s;
    }
    println!("total {total}");
}
