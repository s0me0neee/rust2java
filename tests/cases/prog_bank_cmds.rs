// Command interpreter over stdin: accounts in a HashMap, error enum with Display, ? on custom errors.
use std::collections::HashMap;
use std::fmt;
use std::io::{self, BufRead};

#[derive(Debug)]
enum BankError {
    NoAccount(String),
    Insufficient { need: u64, have: u64 },
    Parse(String),
    Unknown(String),
}

impl fmt::Display for BankError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BankError::NoAccount(n) => write!(f, "no account '{}'", n),
            BankError::Insufficient { need, have } => write!(f, "need {} have {}", need, have),
            BankError::Parse(s) => write!(f, "bad number: {}", s),
            BankError::Unknown(c) => write!(f, "unknown command {}", c),
        }
    }
}

struct Bank {
    accounts: HashMap<String, u64>,
    history: Vec<String>,
}

fn amount(s: Option<&str>) -> Result<u64, BankError> {
    let s = s.ok_or_else(|| BankError::Parse("<missing>".to_string()))?;
    s.parse().map_err(|_| BankError::Parse(s.to_string()))
}

impl Bank {
    fn balance_mut(&mut self, name: &str) -> Result<&mut u64, BankError> {
        self.accounts.get_mut(name).ok_or(BankError::NoAccount(name.to_string()))
    }

    fn exec(&mut self, line: &str) -> Result<Option<String>, BankError> {
        let mut parts = line.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => return Ok(None),
        };
        match cmd {
            "open" => {
                let name = parts.next().unwrap_or("anon").to_string();
                self.accounts.entry(name.clone()).or_insert(0);
                self.history.push(format!("open:{}", name));
                Ok(Some(format!("opened {}", name)))
            }
            "dep" => {
                let name = parts.next().unwrap_or("");
                let n = amount(parts.next())?;
                let bal = self.accounts.get_mut(name).ok_or_else(|| BankError::NoAccount(name.to_string()))?;
                *bal += n;
                Ok(Some(format!("{} = {}", name, bal)))
            }
            "wd" => {
                let name = parts.next().unwrap_or("");
                let n = amount(parts.next())?;
                let bal = self.accounts.get_mut(name).ok_or(BankError::NoAccount(name.to_string()))?;
                if *bal < n {
                    return Err(BankError::Insufficient { need: n, have: *bal });
                }
                *bal -= n;
                Ok(Some(format!("{} = {}", name, *bal)))
            }
            "xfer" => {
                let (a, b) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                let n = amount(parts.next())?;
                let have = *self.balance_mut(a)?;
                self.balance_mut(b)?;
                if have < n {
                    return Err(BankError::Insufficient { need: n, have });
                }
                *self.accounts.get_mut(a).unwrap() -= n;
                *self.accounts.get_mut(b).expect("checked") += n;
                self.history.push(format!("xfer:{}->{}:{}", a, b, n));
                Ok(None)
            }
            "show" => {
                let mut names: Vec<_> = self.accounts.iter().collect();
                names.sort_by(|x, y| y.1.cmp(x.1).then(x.0.cmp(y.0)));
                Ok(Some(names.iter().map(|(k, v)| format!("{}:{}", k, v)).collect::<Vec<_>>().join(" ")))
            }
            other => Err(BankError::Unknown(other.to_uppercase())),
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut bank = Bank { accounts: HashMap::new(), history: vec![] };
    let mut errors = 0;
    for (i, line) in stdin.lock().lines().enumerate() {
        let line = line.unwrap();
        let line = line.trim();
        if line.starts_with('#') {
            continue;
        }
        match bank.exec(line) {
            Ok(Some(msg)) => println!("{:>3}: {}", i + 1, msg),
            Ok(None) => {}
            Err(e) => {
                errors += 1;
                println!("{:>3}! {}", i + 1, e);
                if let BankError::Insufficient { need, .. } = &e {
                    println!("     short by {}", need);
                }
            }
        }
    }
    println!("{} errors; history {:?}", errors, bank.history);
    let total: u64 = bank.accounts.values().sum();
    println!("total {}", total);
}
