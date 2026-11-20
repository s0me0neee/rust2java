// Borrowing parser: a struct holding &str slices of the input, lifetimes on impl blocks, a
// Record<'a> table with column lookup, aggregation with BTreeMap, and errors with line numbers.
use std::collections::BTreeMap;

const DATA: &str = "name,city,age,score
alice,paris,31,88.5
bob,berlin,27,91
carol,paris,45,79.25
dave,rome,,60
erin,berlin,38,95.5
frank,rome,52,x";

struct Table<'a> {
    header: Vec<&'a str>,
    rows: Vec<Vec<&'a str>>,
}

#[derive(Debug)]
enum CellError<'a> {
    Missing { line: usize, col: &'a str },
    Bad { line: usize, text: &'a str },
}

impl<'a> Table<'a> {
    fn parse(src: &'a str) -> Table<'a> {
        let mut lines = src.lines();
        let header = lines.next().unwrap_or("").split(',').collect();
        let rows = lines.map(|l| l.split(',').collect()).collect();
        Table { header, rows }
    }
    fn col(&self, name: &str) -> Option<usize> {
        self.header.iter().position(|h| *h == name)
    }
    fn get(&self, row: usize, name: &'a str) -> Result<&'a str, CellError<'a>> {
        let c = self.col(name).ok_or(CellError::Missing { line: row + 2, col: name })?;
        match self.rows[row].get(c) {
            Some(s) if !s.is_empty() => Ok(s),
            _ => Err(CellError::Missing { line: row + 2, col: name }),
        }
    }
    fn number(&self, row: usize, name: &'a str) -> Result<f64, CellError<'a>> {
        let s = self.get(row, name)?;
        s.parse::<f64>().map_err(|_| CellError::Bad { line: row + 2, text: s })
    }
}

fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.len() > a.len() { b } else { a }
}

fn main() {
    let t = Table::parse(DATA);
    println!("{} cols, {} rows, score at {:?}", t.header.len(), t.rows.len(), t.col("score"));
    let mut by_city: BTreeMap<&str, Vec<f64>> = BTreeMap::new();
    let mut errors = Vec::new();
    for i in 0..t.rows.len() {
        let city = t.get(i, "city").unwrap();
        match (t.number(i, "age"), t.number(i, "score")) {
            (Ok(age), Ok(score)) => by_city.entry(city).or_default().push(score / age),
            (Err(e), _) | (_, Err(e)) => errors.push(e),
        }
    }
    for (city, v) in &by_city {
        let mean = v.iter().sum::<f64>() / v.len() as f64;
        println!("{:<8}{:>3} {:.4}", city, v.len(), mean);
    }
    for e in &errors {
        match e {
            CellError::Missing { line, col } => println!("line {}: missing {}", line, col),
            CellError::Bad { line, text } => println!("line {}: bad number {:?}", line, text),
        }
    }
    println!("{:?}", t.get(0, "email").is_err());
    let names: Vec<&str> = (0..t.rows.len()).filter_map(|i| t.get(i, "name").ok()).collect();
    let long = names.iter().fold("", |acc, n| longest(acc, n));
    println!("{} {}", names.join("|"), long);
    let initials: String = names.iter().map(|n| n.chars().next().unwrap().to_ascii_uppercase()).collect();
    println!("{}", initials);
    let first_word = DATA.split(|c: char| c == ',' || c == '\n').nth(5);
    println!("{:?}", first_word);
}
