// Cell / RefCell: fields, locals, shared through Rc, replace/take/update.
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

struct Counter {
    hits: Cell<u32>,
    label: String,
}

impl Counter {
    fn bump(&self) {
        self.hits.set(self.hits.get() + 1);
    }
    fn drain(&self) -> u32 {
        self.hits.replace(0)
    }
}

fn main() {
    let c = Counter { hits: Cell::new(0), label: String::from("site") };
    c.bump();
    c.bump();
    c.bump();
    println!("{} {}", c.hits.get(), c.label);
    println!("{} {}", c.drain(), c.hits.get());

    let n = Cell::new(10);
    n.set(n.get() * 2);
    println!("{}", n.get());
    let old = n.replace(7);
    println!("{old} {}", n.take());
    n.update(|v| v + 1);
    println!("{}", n.get());

    let shared = Rc::new(RefCell::new(0u64));
    let alias = Rc::clone(&shared);
    *alias.borrow_mut() += 1;
    *shared.borrow_mut() += 2;
    println!("{}", shared.borrow().clone());
    println!("{}", *shared.borrow() * 10);

    let cache: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    cache.borrow_mut().insert(1, 10);
    cache.borrow_mut().insert(2, 20);
    println!("{:?} {:?}", cache.borrow().get(&1), cache.borrow().get(&9));
    let total: u64 = cache.borrow().values().sum();
    println!("{total}");

    let text = RefCell::new(String::from("ab"));
    text.borrow_mut().push('c');
    text.borrow_mut().push('d');
    println!("{} {}", text.borrow(), text.borrow().len());
}
