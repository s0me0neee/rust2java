#[derive(Debug)]
pub struct ChessPosition {
    rank: i32,
    file: i32,
}

#[derive(Debug)]
pub struct Queen {
    pos: ChessPosition,
}

impl ChessPosition {
    pub fn new(rank: i32, file: i32) -> Option<Self> {
        if (0..8).contains(&rank) && (0..8).contains(&file) {
            Some(ChessPosition { rank, file })
        } else {
            None
        }
    }
}

impl Queen {
    pub fn new(pos: ChessPosition) -> Self {
        Queen { pos }
    }

    pub fn can_attack(&self, other: &Queen) -> bool {
        let dr = self.pos.rank.abs_diff(other.pos.rank);
        let df = self.pos.file.abs_diff(other.pos.file);
        dr == 0 || df == 0 || dr == df
    }
}

fn main() {
    println!("{:?}", ChessPosition::new(8, 2));
    println!("{:?}", ChessPosition::new(-1, 2).is_none());
    let pairs = [((2, 4), (6, 6)), ((2, 4), (2, 6)), ((0, 4), (7, 3)), ((2, 2), (0, 4))];
    for ((r1, f1), (r2, f2)) in pairs {
        let a = Queen::new(ChessPosition::new(r1, f1).unwrap());
        let b = Queen::new(ChessPosition::new(r2, f2).unwrap());
        println!("({r1},{f1}) vs ({r2},{f2}): {}", a.can_attack(&b));
    }
}
