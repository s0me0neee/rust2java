use std::collections::HashMap;
use std::fmt;

#[derive(Debug)]
enum LedgerError {
    NoSuchAccount(u32),
    Insufficient { id: u32, balance: i64, wanted: i64 },
    Frozen(u32),
}

impl fmt::Display for LedgerError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LedgerError::NoSuchAccount(id) => write!(f, "no account #{id}"),
            LedgerError::Insufficient { id, balance, wanted } => {
                write!(f, "#{id} has {balance}, wanted {wanted}")
            }
            LedgerError::Frozen(id) => write!(f, "#{id} is frozen"),
        }
    }
}

struct Account {
    owner: String,
    balance: i64,
    frozen: bool,
}

struct Ledger {
    accounts: HashMap<u32, Account>,
    next_id: u32,
}

impl Ledger {
    fn open(&mut self, owner: &str, deposit: i64) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.accounts.insert(id, Account { owner: owner.to_string(), balance: deposit, frozen: false });
        id
    }

    fn get_mut(&mut self, id: u32) -> Result<&mut Account, LedgerError> {
        let acct = self.accounts.get_mut(&id).ok_or(LedgerError::NoSuchAccount(id))?;
        if acct.frozen {
            return Err(LedgerError::Frozen(id));
        }
        Ok(acct)
    }

    fn withdraw(&mut self, id: u32, amount: i64) -> Result<i64, LedgerError> {
        let acct = self.get_mut(id)?;
        if acct.balance < amount {
            return Err(LedgerError::Insufficient { id, balance: acct.balance, wanted: amount });
        }
        acct.balance -= amount;
        Ok(acct.balance)
    }

    fn transfer(&mut self, from: u32, to: u32, amount: i64) -> Result<(), LedgerError> {
        self.get_mut(to)?;
        self.withdraw(from, amount)?;
        self.get_mut(to)?.balance += amount;
        Ok(())
    }
}

fn main() {
    let mut l = Ledger { accounts: HashMap::new(), next_id: 1 };
    let a = l.open("ann", 100);
    let b = l.open("bob", 20);
    l.accounts.get_mut(&b).unwrap().frozen = false;
    let ops: Vec<(u32, u32, i64)> = vec![(a, b, 30), (b, a, 100), (a, 9, 1), (b, a, 50)];
    for (from, to, amt) in ops {
        match l.transfer(from, to, amt) {
            Ok(()) => println!("moved {amt} from {from} to {to}"),
            Err(e) => println!("error: {e}"),
        }
    }
    l.accounts.get_mut(&a).unwrap().frozen = true;
    if let Err(e) = l.withdraw(a, 1) {
        println!("error: {e} ({e:?})");
    }
    let mut ids: Vec<&u32> = l.accounts.keys().collect();
    ids.sort();
    for id in ids {
        let acct = &l.accounts[id];
        println!("#{id} {} {}", acct.owner, acct.balance);
    }
}
