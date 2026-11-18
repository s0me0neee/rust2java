// Reads records from stdin, aggregates, prints a table. Uses lines(), split(','), parse with error handling.
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

#[derive(Debug, Default, Clone)]
struct Stats {
    count: usize,
    sum: f64,
    min: f64,
    max: f64,
}

impl Stats {
    fn add(&mut self, v: f64) {
        if self.count == 0 {
            self.min = v;
            self.max = v;
        } else {
            self.min = self.min.min(v);
            self.max = self.max.max(v);
        }
        self.count += 1;
        self.sum += v;
    }
    fn mean(&self) -> Option<f64> {
        if self.count == 0 { None } else { Some(self.sum / self.count as f64) }
    }
}

fn parse_line(line: &str) -> Result<(String, f64), String> {
    let mut parts = line.split(',').map(str::trim);
    let name = parts.next().filter(|s| !s.is_empty()).ok_or("missing name")?;
    let value = parts
        .next()
        .ok_or_else(|| format!("missing value for {}", name))?
        .parse::<f64>()
        .map_err(|e| format!("bad value for {}: {}", name, e))?;
    if parts.next().is_some() {
        return Err("too many fields".to_string());
    }
    Ok((name.to_lowercase(), value))
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut groups: BTreeMap<String, Stats> = BTreeMap::new();
    let mut bad = Vec::new();
    for (i, line) in stdin.lock().lines().enumerate() {
        let line = line?;
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        match parse_line(&line) {
            Ok((name, v)) => groups.entry(name).or_default().add(v),
            Err(e) => bad.push((i + 1, e)),
        }
    }
    let out = io::stdout();
    let mut out = out.lock();
    writeln!(out, "{:<10}{:>6}{:>10}{:>10}{:>10}", "name", "n", "mean", "min", "max")?;
    writeln!(out, "{}", "-".repeat(46))?;
    for (name, s) in &groups {
        writeln!(
            out,
            "{:<10}{:>6}{:>10.2}{:>10.2}{:>10.2}",
            name,
            s.count,
            s.mean().unwrap_or(f64::NAN),
            s.min,
            s.max
        )?;
    }
    let total: Stats = groups.values().fold(Stats::default(), |mut acc, s| {
        acc.count += s.count;
        acc.sum += s.sum;
        acc
    });
    writeln!(out, "total {} values, mean {:.3}", total.count, total.mean().unwrap_or(0.0))?;
    for (line, err) in &bad {
        writeln!(out, "line {}: {}", line, err)?;
    }
    let best = groups
        .iter()
        .filter_map(|(k, s)| s.mean().map(|m| (k, m)))
        .max_by(|a, b| a.1.total_cmp(&b.1));
    if let Some((k, m)) = best {
        writeln!(out, "best: {} {:.1}", k, m)?;
    }
    out.flush()?;
    Ok(())
}
