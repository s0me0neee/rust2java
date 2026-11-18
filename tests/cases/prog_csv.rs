// CSV-ish report: read records from stdin, split with `split(',')`, parse fields
// with `?`, collect typed rows, then group and print a formatted summary.
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, BufRead};

#[derive(Debug, Clone, PartialEq)]
enum CsvError {
    MissingField(&'static str),
    BadNumber { column: String, text: String },
    BadRow(usize, String),
}

impl fmt::Display for CsvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CsvError::MissingField(name) => write!(f, "missing field `{}`", name),
            CsvError::BadNumber { column, text } => {
                write!(f, "column `{}` is not a number: {:?}", column, text)
            }
            CsvError::BadRow(line, reason) => write!(f, "row {}: {}", line, reason),
        }
    }
}

#[derive(Debug, Clone)]
struct Record {
    line: usize,
    name: String,
    department: String,
    hours: f64,
    rate: f64,
}

impl Record {
    fn pay(&self) -> f64 {
        self.hours * self.rate
    }
}

fn parse_number(column: &str, text: &str) -> Result<f64, CsvError> {
    text.trim()
        .parse::<f64>()
        .map_err(|_| CsvError::BadNumber {
            column: column.to_string(),
            text: text.to_string(),
        })
}

fn parse_record(line: usize, text: &str) -> Result<Record, CsvError> {
    let fields: Vec<&str> = text.split(',').collect();
    let name = fields
        .first()
        .map(|field| field.trim())
        .filter(|field| !field.is_empty())
        .ok_or(CsvError::MissingField("name"))?;
    let department = fields
        .get(1)
        .map(|field| field.trim())
        .filter(|field| !field.is_empty())
        .ok_or(CsvError::MissingField("department"))?;
    let hours_text = fields.get(2).ok_or(CsvError::MissingField("hours"))?;
    let rate_text = fields.get(3).ok_or(CsvError::MissingField("rate"))?;
    let hours = parse_number("hours", hours_text)?;
    let rate = parse_number("rate", rate_text)?;
    if hours < 0.0 || rate < 0.0 {
        return Err(CsvError::BadRow(line, "negative hours or rate".to_string()));
    }
    Ok(Record {
        line,
        name: name.to_string(),
        department: department.to_string(),
        hours,
        rate,
    })
}

fn parse_all(text: &str) -> Vec<Result<Record, CsvError>> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| parse_record(index + 1, line))
        .collect()
}

struct Summary {
    department: String,
    people: usize,
    hours: f64,
    pay: f64,
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<12} {:>3} {:>8.1} {:>11.2}",
            self.department, self.people, self.hours, self.pay
        )
    }
}

fn summarize(records: &[Record]) -> Vec<Summary> {
    let mut groups: BTreeMap<String, Summary> = BTreeMap::new();
    for record in records {
        let slot = groups
            .entry(record.department.clone())
            .or_insert(Summary {
                department: record.department.clone(),
                people: 0,
                hours: 0.0,
                pay: 0.0,
            });
        slot.people += 1;
        slot.hours += record.hours;
        slot.pay += record.pay();
    }
    groups.into_values().collect()
}

fn main() {
    let stdin = io::stdin();
    let mut text = String::new();
    for line in stdin.lock().lines() {
        match line {
            Ok(line) => {
                text.push_str(&line);
                text.push('\n');
            }
            Err(_) => break,
        }
    }
    let results = parse_all(&text);
    let mut records: Vec<Record> = Vec::new();
    let mut errors = 0;
    for result in results {
        match result {
            Ok(record) => records.push(record),
            Err(error) => {
                errors += 1;
                println!("skipped: {}", error);
            }
        }
    }
    println!("parsed {} rows, {} errors", records.len(), errors);
    let total_pay: f64 = records.iter().map(|record| record.pay()).sum();
    let total_hours: f64 = records.iter().map(|record| record.hours).sum();
    println!("total hours {:.1}, total pay {:.2}", total_hours, total_pay);
    for summary in summarize(&records) {
        println!("{}", summary);
    }
    let mut top: Vec<&Record> = records.iter().collect();
    top.sort_by(|a, b| b.pay().partial_cmp(&a.pay()).unwrap());
    let names: Vec<String> = top
        .iter()
        .take(3)
        .map(|record| format!("{} ({:.2})", record.name, record.pay()))
        .collect();
    println!("top earners: {}", names.join(", "));
    let high = records.iter().filter(|record| record.hours > 20.0).count();
    println!("over 20 hours: {}", high);
    let raises: Vec<Record> = records
        .iter()
        .map(|record| Record {
            line: record.line,
            name: record.name.clone(),
            department: record.department.clone(),
            hours: record.hours,
            rate: (record.rate * 1.1 * 100.0).round() / 100.0,
        })
        .collect();
    let mut by_name: Vec<&Record> = raises.iter().collect();
    by_name.sort_by(|a, b| a.name.cmp(&b.name));
    for record in by_name {
        println!(
            "line {:>2}: {:<8} {:<10} {:.1}h at {:.2} => {:.2}",
            record.line,
            record.name,
            record.department,
            record.hours,
            record.rate,
            record.pay()
        );
    }
    let average = total_pay / records.len() as f64;
    println!("average pay: {:.2}", average);
    let formatted: Vec<String> = records
        .iter()
        .map(|record| format!("{0:<8}|{1:>6.1}|{2:>8.2}", record.name, record.hours, record.pay()))
        .collect();
    for row in formatted.iter().take(2) {
        println!("[{}]", row);
    }
    let departments: Vec<&str> = records.iter().map(|r| r.department.as_str()).collect();
    let unique = departments.iter().filter(|d| **d == "eng").count();
    println!("eng rows: {}", unique);
    match parse_record(99, "solo,eng,10") {
        Ok(record) => println!("unexpected: {:?}", record),
        Err(error) => println!("expected error: {}", error),
    }
}
