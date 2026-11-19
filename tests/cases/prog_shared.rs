// Shared mutable state: Rc<RefCell<..>> graph of accounts, observers via trait objects, Cell counters.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug)]
struct Account {
    id: u32,
    owner: String,
    balance: i64,
    history: Vec<i64>,
}

type Shared = Rc<RefCell<Account>>;

trait Observer {
    fn notify(&self, event: &str);
    fn count(&self) -> usize;
}

struct Logger {
    lines: RefCell<Vec<String>>,
}

struct Counter {
    n: Cell<usize>,
}

impl Observer for Logger {
    fn notify(&self, event: &str) {
        self.lines.borrow_mut().push(event.to_uppercase());
    }
    fn count(&self) -> usize {
        self.lines.borrow().len()
    }
}

impl Observer for Counter {
    fn notify(&self, _event: &str) {
        self.n.set(self.n.get() + 1);
    }
    fn count(&self) -> usize {
        self.n.get()
    }
}

struct Bank {
    accounts: HashMap<u32, Shared>,
    observers: Vec<Rc<dyn Observer>>,
    next_id: u32,
}

impl Bank {
    fn new() -> Self {
        Bank { accounts: HashMap::new(), observers: Vec::new(), next_id: 1 }
    }
    fn open(&mut self, owner: &str, initial: i64) -> Shared {
        let acc = Rc::new(RefCell::new(Account { id: self.next_id, owner: owner.to_string(), balance: initial, history: vec![initial] }));
        self.accounts.insert(self.next_id, Rc::clone(&acc));
        self.next_id += 1;
        self.emit(&format!("open {} {}", owner, initial));
        acc
    }
    fn emit(&self, e: &str) {
        for o in &self.observers {
            o.notify(e);
        }
    }
    fn transfer(&self, from: u32, to: u32, amount: i64) -> Result<(), String> {
        let a = self.accounts.get(&from).ok_or(format!("no account {}", from))?;
        let b = self.accounts.get(&to).ok_or(format!("no account {}", to))?;
        if a.borrow().balance < amount {
            return Err(format!("{} has insufficient funds", a.borrow().owner));
        }
        a.borrow_mut().balance -= amount;
        a.borrow_mut().history.push(-amount);
        {
            let mut bm = b.borrow_mut();
            bm.balance += amount;
            bm.history.push(amount);
        }
        self.emit(&format!("transfer {}->{} {}", from, to, amount));
        Ok(())
    }
    fn total(&self) -> i64 {
        self.accounts.values().map(|a| a.borrow().balance).sum()
    }
}

fn main() {
    let logger = Rc::new(Logger { lines: RefCell::new(vec![]) });
    let counter = Rc::new(Counter { n: Cell::new(0) });
    let mut bank = Bank::new();
    bank.observers.push(logger.clone());
    bank.observers.push(counter.clone());
    let alice = bank.open("alice", 100);
    let bob = bank.open("bob", 20);
    println!("{:?}", bank.transfer(1, 2, 30));
    println!("{:?}", bank.transfer(2, 1, 500));
    println!("{:?}", bank.transfer(1, 9, 1));
    alice.borrow_mut().balance += 1;
    println!("alice={} bob={} total={}", alice.borrow().balance, bob.borrow().balance, bank.total());
    println!("{:?} {:?}", alice.borrow().history, bob.borrow().history);
    println!("logged={} counted={}", logger.count(), counter.count());
    for l in logger.lines.borrow().iter() {
        println!("  {}", l);
    }
    let shared_list = Rc::new(RefCell::new(Vec::<i32>::new()));
    let pushers: Vec<Box<dyn Fn(i32)>> = (0..3)
        .map(|k| {
            let list = Rc::clone(&shared_list);
            Box::new(move |x: i32| list.borrow_mut().push(x * 10 + k)) as Box<dyn Fn(i32)>
        })
        .collect();
    for (i, p) in pushers.iter().enumerate() {
        p(i as i32 + 1);
    }
    println!("{:?}", shared_list.borrow());
    let tally = Cell::new(0);
    let add = |n: i32| tally.set(tally.get() + n);
    add(5);
    add(7);
    println!("tally={}", tally.get());
    let owner = alice.borrow().owner.clone();
    let id = alice.borrow().id;
    println!("{} #{}", owner, id);
}
