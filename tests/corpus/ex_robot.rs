#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    fn right(self) -> Self {
        match self {
            Direction::North => Direction::East,
            Direction::East => Direction::South,
            Direction::South => Direction::West,
            Direction::West => Direction::North,
        }
    }
    fn left(self) -> Self {
        self.right().right().right()
    }
}

#[derive(Debug)]
pub struct Robot {
    x: i32,
    y: i32,
    d: Direction,
}

impl Robot {
    pub fn new(x: i32, y: i32, d: Direction) -> Self {
        Robot { x, y, d }
    }
    #[must_use]
    pub fn turn_right(self) -> Self {
        Self { d: self.d.right(), ..self }
    }
    #[must_use]
    pub fn turn_left(self) -> Self {
        Self { d: self.d.left(), ..self }
    }
    #[must_use]
    pub fn advance(self) -> Self {
        let (x, y) = match self.d {
            Direction::North => (self.x, self.y + 1),
            Direction::East => (self.x + 1, self.y),
            Direction::South => (self.x, self.y - 1),
            Direction::West => (self.x - 1, self.y),
        };
        Self { x, y, ..self }
    }
    #[must_use]
    pub fn instructions(self, instructions: &str) -> Self {
        instructions.chars().fold(self, |r, c| match c {
            'R' => r.turn_right(),
            'L' => r.turn_left(),
            'A' => r.advance(),
            _ => r,
        })
    }
    pub fn position(&self) -> (i32, i32) {
        (self.x, self.y)
    }
}

fn main() {
    let r = Robot::new(7, 3, Direction::North).instructions("RAALAL");
    println!("{:?} {:?}", r.position(), r.d);
    println!("{:?}", Robot::new(0, 0, Direction::West).advance().turn_left());
}
