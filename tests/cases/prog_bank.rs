// A bank with fallible operations: custom error enum, `Result` plumbing with `?`,
// transaction history and a per-customer summary.
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum BankError {
    NotFound(u32),
    InsufficientFunds { needed: f64, available: f64 },
    InvalidAmount(f64),
    Frozen(u32),
}

impl fmt::Display for BankError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BankError::NotFound(id) => write!(f, "account {} not found", id),
            BankError::InsufficientFunds { needed, available } => {
                write!(f, "need {:.2} but only {:.2} available", needed, available)
            }
            BankError::InvalidAmount(amount) => write!(f, "invalid amount: {:.2}", amount),
            BankError::Frozen(id) => write!(f, "account {} is frozen", id),
        }
    }
}

#[derive(Debug, Clone)]
enum Kind {
    Deposit,
    Withdraw,
    TransferTo(u32),
    TransferFrom(u32),
    Fee,
}

impl Kind {
    fn name(&self) -> &'static str {
        match self {
            Kind::Deposit => "deposit",
            Kind::Withdraw => "withdraw",
            Kind::TransferTo(_) => "transfer-out",
            Kind::TransferFrom(_) => "transfer-in",
            Kind::Fee => "fee",
        }
    }
}

#[derive(Debug, Clone)]
struct KindStat {
    count: usize,
    total: f64,
}

#[derive(Debug, Clone)]
struct Entry {
    account: u32,
    kind: Kind,
    amount: f64,
    balance_after: f64,
}

#[derive(Debug, Clone)]
struct Account {
    id: u32,
    owner: String,
    balance: f64,
    frozen: bool,
}

impl Account {
    fn new(id: u32, owner: &str, balance: f64) -> Account {
        Account {
            id,
            owner: owner.to_string(),
            balance,
            frozen: false,
        }
    }

    fn available(&self) -> f64 {
        if self.frozen { 0.0 } else { self.balance }
    }
}

struct Bank {
    accounts: BTreeMap<u32, Account>,
    history: Vec<Entry>,
    fees: f64,
}

impl Bank {
    fn new(fee: f64) -> Bank {
        Bank {
            accounts: BTreeMap::new(),
            history: Vec::new(),
            fees: fee,
        }
    }

    fn open(&mut self, id: u32, owner: &str, balance: f64) -> Result<(), BankError> {
        if balance < 0.0 {
            return Err(BankError::InvalidAmount(balance));
        }
        self.accounts.insert(id, Account::new(id, owner, balance));
        Ok(())
    }

    fn account(&self, id: u32) -> Result<&Account, BankError> {
        self.accounts.get(&id).ok_or(BankError::NotFound(id))
    }

    fn account_mut(&mut self, id: u32) -> Result<&mut Account, BankError> {
        self.accounts.get_mut(&id).ok_or(BankError::NotFound(id))
    }

    fn record(&mut self, account: u32, kind: Kind, amount: f64, balance_after: f64) {
        self.history.push(Entry {
            account,
            kind,
            amount,
            balance_after,
        });
    }

    fn deposit(&mut self, id: u32, amount: f64) -> Result<f64, BankError> {
        if amount <= 0.0 || !amount.is_finite() {
            return Err(BankError::InvalidAmount(amount));
        }
        let account = self.account_mut(id)?;
        if account.frozen {
            return Err(BankError::Frozen(id));
        }
        account.balance += amount;
        let balance = account.balance;
        self.record(id, Kind::Deposit, amount, balance);
        Ok(balance)
    }

    fn withdraw(&mut self, id: u32, amount: f64) -> Result<f64, BankError> {
        if amount <= 0.0 {
            return Err(BankError::InvalidAmount(amount));
        }
        let account = self.account_mut(id)?;
        if account.frozen {
            return Err(BankError::Frozen(id));
        }
        if account.available() < amount {
            return Err(BankError::InsufficientFunds {
                needed: amount,
                available: account.available(),
            });
        }
        account.balance -= amount;
        let balance = account.balance;
        self.record(id, Kind::Withdraw, amount, balance);
        Ok(balance)
    }

    fn transfer(&mut self, from: u32, to: u32, amount: f64) -> Result<(), BankError> {
        if self.account(from)?.id == self.account(to)?.id {
            return Err(BankError::InvalidAmount(amount));
        }
        self.withdraw(from, amount)?;
        self.deposit(to, amount)
            .map_err(|error| match error {
                BankError::NotFound(_) => BankError::NotFound(to),
                other => other,
            })?;
        self.record(from, Kind::TransferTo(to), amount, self.account(from)?.balance);
        self.record(to, Kind::TransferFrom(from), amount, self.account(to)?.balance);
        Ok(())
    }

    fn charge_fee(&mut self, id: u32) -> Result<(), BankError> {
        let fee = self.fees;
        if fee <= 0.0 {
            return Ok(());
        }
        let account = self.account_mut(id)?;
        let taken = fee.min(account.balance);
        account.balance -= taken;
        let balance = account.balance;
        self.record(id, Kind::Fee, taken, balance);
        Ok(())
    }

    fn freeze(&mut self, id: u32) -> Result<(), BankError> {
        let account = self.account_mut(id)?;
        account.frozen = true;
        Ok(())
    }

    fn report(&self) {
        println!("{:<4} {:<10} {:>10}  {}", "id", "owner", "balance", "status");
        println!("{}", "-".repeat(38));
        for account in self.accounts.values() {
            let status = if account.frozen { "frozen" } else { "open" };
            println!(
                "{:<4} {:<10} {:>10.2}  {}",
                account.id, account.owner, account.balance, status
            );
        }
        let total: f64 = self.accounts.values().map(|a| a.balance).sum();
        println!("total holdings: {:.2}", total);
    }

    fn history_for(&self, id: u32) -> Vec<&Entry> {
        self.history.iter().filter(|e| e.account == id).collect()
    }

    fn summary(&self) {
        let mut by_kind: BTreeMap<&str, KindStat> = BTreeMap::new();
        for entry in &self.history {
            let slot = by_kind
                .entry(entry.kind.name())
                .or_insert(KindStat { count: 0, total: 0.0 });
            slot.count += 1;
            slot.total += entry.amount;
        }
        for (kind, stat) in &by_kind {
            println!("{:<13} {:>3} entries, {:>9.2}", kind, stat.count, stat.total);
        }
        let largest = self
            .history
            .iter()
            .max_by(|a, b| a.amount.partial_cmp(&b.amount).unwrap());
        match largest {
            Some(entry) => println!(
                "largest: {} {:.2} on account {}",
                entry.kind.name(),
                entry.amount,
                entry.account
            ),
            None => println!("largest: none"),
        }
    }
}

fn main() {
    let mut bank = Bank::new(1.25);
    let setup = [
        (1, "alice", 100.0),
        (2, "bob", 50.0),
        (3, "carol", -5.0),
        (4, "dave", 10.0),
    ];
    for (id, owner, balance) in setup {
        match bank.open(id, owner, balance) {
            Ok(()) => println!("opened {} for {}", id, owner),
            Err(error) => println!("cannot open {}: {}", id, error),
        }
    }
    let operations: Vec<(&str, u32, u32, f64)> = vec![
        ("deposit", 1, 0, 25.0),
        ("withdraw", 1, 0, 30.0),
        ("transfer", 2, 1, 20.0),
        ("transfer", 2, 1, 500.0),
        ("fee", 4, 0, 0.0),
        ("deposit", 9, 0, 5.0),
        ("withdraw", 9, 0, 5.0),
        ("deposit", 1, 0, -3.0),
    ];
    for (op, a, b, amount) in operations {
        let result = match op {
            "deposit" => bank.deposit(a, amount).map(|balance| format!("balance {:.2}", balance)),
            "withdraw" => bank.withdraw(a, amount).map(|balance| format!("balance {:.2}", balance)),
            "transfer" => bank
                .transfer(a, b, amount)
                .map(|()| format!("moved {:.2} from {} to {}", amount, a, b)),
            "fee" => bank.charge_fee(a).map(|()| "fee charged".to_string()),
            other => {
                println!("skipping unknown operation {:?}", other);
                Err(BankError::InvalidAmount(0.0))
            }
        };
        match result {
            Ok(message) => println!("ok: {}", message),
            Err(error) => println!("error: {}", error),
        }
    }
    if let Err(error) = bank.freeze(4) {
        println!("freeze failed: {}", error);
    }
    println!("{:?}", bank.deposit(4, 5.0));
    bank.report();
    bank.summary();
    for entry in bank.history_for(1) {
        println!(
            "  {:<12} {:>8.2} -> {:>8.2}",
            entry.kind.name(),
            entry.amount,
            entry.balance_after
        );
    }
    let missing = bank.account(42);
    println!("{:?}", missing.err());
    println!("zero: {}", BankError::InsufficientFunds { needed: 0.0, available: -0.0 });
}
