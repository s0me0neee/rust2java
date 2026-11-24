use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Cell {
    Dead,
    Alive,
}

struct Life {
    w: usize,
    h: usize,
    cells: Vec<Cell>,
}

impl Life {
    fn parse(s: &str) -> Life {
        let rows: Vec<&str> = s.lines().collect();
        let cells = rows
            .iter()
            .flat_map(|r| r.chars().map(|c| if c == '#' { Cell::Alive } else { Cell::Dead }))
            .collect();
        Life { w: rows[0].len(), h: rows.len(), cells }
    }

    fn neighbors(&self, r: usize, c: usize) -> usize {
        let mut n = 0;
        for dr in [self.h - 1, 0, 1] {
            for dc in [self.w - 1, 0, 1] {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let (rr, cc) = ((r + dr) % self.h, (c + dc) % self.w);
                if self.cells[rr * self.w + cc] == Cell::Alive {
                    n += 1;
                }
            }
        }
        n
    }

    fn step(&mut self) {
        let next: Vec<Cell> = (0..self.h * self.w)
            .map(|i| {
                let (r, c) = (i / self.w, i % self.w);
                match (self.cells[i], self.neighbors(r, c)) {
                    (Cell::Alive, 2) | (_, 3) => Cell::Alive,
                    _ => Cell::Dead,
                }
            })
            .collect();
        self.cells = next;
    }

    fn population(&self) -> usize {
        self.cells.iter().filter(|&&c| c == Cell::Alive).count()
    }
}

impl fmt::Display for Life {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for row in self.cells.chunks(self.w) {
            let line: String = row.iter().map(|c| match c {
                Cell::Alive => '#',
                Cell::Dead => '.',
            }).collect();
            writeln!(f, "{line}")?;
        }
        Ok(())
    }
}

fn main() {
    let mut life = Life::parse("......\n..#...\n...#..\n.###..\n......\n......");
    for round in 0..4 {
        println!("gen {round} pop {}", life.population());
        print!("{life}");
        life.step();
    }
}
