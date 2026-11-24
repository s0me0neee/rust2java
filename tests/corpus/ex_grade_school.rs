use std::collections::BTreeMap;

#[derive(Default)]
pub struct School {
    grades: BTreeMap<u32, Vec<String>>,
}

impl School {
    pub fn new() -> School {
        School::default()
    }

    pub fn add(&mut self, grade: u32, student: &str) {
        let names = self.grades.entry(grade).or_default();
        names.push(student.to_string());
        names.sort();
    }

    pub fn grades(&self) -> Vec<u32> {
        self.grades.keys().copied().collect()
    }

    pub fn grade(&self, grade: u32) -> Vec<String> {
        self.grades.get(&grade).cloned().unwrap_or_default()
    }
}

fn main() {
    let mut s = School::new();
    for (g, n) in [(2, "Blair"), (1, "Zoe"), (2, "James"), (1, "Anna"), (5, "Paul")] {
        s.add(g, n);
    }
    println!("{:?}", s.grades());
    for g in 1..=5 {
        let names = s.grade(g);
        if !names.is_empty() {
            println!("grade {g}: {}", names.join(", "));
        }
    }
    println!("{:?}", s.grade(9));
}
