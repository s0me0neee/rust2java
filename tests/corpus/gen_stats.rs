use std::io::{self, BufRead};

#[derive(Default, Debug)]
struct Stats {
    count: usize,
    sum: f64,
    min: Option<f64>,
    max: Option<f64>,
    values: Vec<f64>,
}

impl Stats {
    fn add(&mut self, v: f64) {
        self.count += 1;
        self.sum += v;
        self.min = Some(self.min.map_or(v, |m| m.min(v)));
        self.max = Some(match self.max {
            Some(m) if m >= v => m,
            _ => v,
        });
        self.values.push(v);
    }

    fn mean(&self) -> Option<f64> {
        (self.count > 0).then(|| self.sum / self.count as f64)
    }

    fn median(&mut self) -> Option<f64> {
        if self.values.is_empty() {
            return None;
        }
        self.values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let n = self.values.len();
        Some(if n % 2 == 1 { self.values[n / 2] } else { (self.values[n / 2 - 1] + self.values[n / 2]) / 2.0 })
    }

    fn stddev(&self) -> Option<f64> {
        let mean = self.mean()?;
        let var = self.values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / self.count as f64;
        Some(var.sqrt())
    }
}

fn main() {
    let mut stats = Stats::default();
    let mut skipped = Vec::new();
    for (n, line) in io::stdin().lock().lines().enumerate() {
        let line = line.expect("read error");
        for tok in line.split([',', ' ', ';']).filter(|t| !t.is_empty()) {
            match tok.parse::<f64>() {
                Ok(v) => stats.add(v),
                Err(_) => skipped.push(format!("{}:{}", n + 1, tok)),
            }
        }
    }
    println!("count={} sum={:.2}", stats.count, stats.sum);
    println!("min={:?} max={:?}", stats.min, stats.max);
    println!("mean={:.3} median={:.3} sd={:.3}", stats.mean().unwrap_or(0.0), stats.median().unwrap_or(0.0), stats.stddev().unwrap_or(0.0));
    println!("skipped {}", skipped.join(" "));
}
