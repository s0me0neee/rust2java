// `impl Index` / `impl IndexMut` for user types: reads, assignment and compound assignment.
use std::ops::{Index, IndexMut};

struct Grid {
    w: usize,
    cells: Vec<char>,
}
impl Index<(usize, usize)> for Grid {
    type Output = char;
    fn index(&self, (r, c): (usize, usize)) -> &char {
        &self.cells[r * self.w + c]
    }
}

struct Registry {
    items: Vec<String>,
}
impl Index<usize> for Registry {
    type Output = String;
    fn index(&self, i: usize) -> &String {
        &self.items[i]
    }
}
impl IndexMut<usize> for Registry {
    fn index_mut(&mut self, i: usize) -> &mut String {
        &mut self.items[i]
    }
}

fn main() {
    let g = Grid { w: 2, cells: vec!['a', 'b', 'c', 'd'] };
    println!("{}{}{}", g[(0, 1)], g[(1, 0)], g[(1, 1)]);
    let mut r = Registry { items: vec!["x".to_string(), "y".to_string()] };
    r[0] = "z".to_string();
    println!("{}", r[0]);
    r[1] += "!";
    println!("{} {}", r[1], r[0]);
}
