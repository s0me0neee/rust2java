// RefCell<String> sharing, Cell floats/bools, replace/take/into_inner, RefCell fields.
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct Store {
    items: RefCell<Vec<String>>,
    count: Cell<usize>,
}

impl Store {
    fn add(&self, s: &str) {
        self.items.borrow_mut().push(s.to_string());
        self.count.set(self.count.get() + 1);
    }
    fn drained(&self) -> Vec<String> {
        self.items.replace(Vec::new())
    }
}

fn main() {
    let shared: Rc<RefCell<String>> = Rc::new(RefCell::new(String::new()));
    let a = Rc::clone(&shared);
    let b = Rc::clone(&shared);
    a.borrow_mut().push_str("ab");
    b.borrow_mut().push_str("cd");
    println!("{} {}", shared.borrow(), shared.borrow().len());
    let old = shared.replace(String::from("x"));
    println!("{old} {}", shared.borrow());
    let taken = shared.take();
    println!("[{taken}] [{}]", shared.borrow());

    let pi = Cell::new(3.5f64);
    pi.set(pi.get() * 2.0);
    println!("{:.2}", pi.get());
    let flag = Cell::new(false);
    flag.set(!flag.get());
    println!("{} {}", flag.get(), flag.replace(true));

    let deferred: Cell<i32>;
    deferred = Cell::new(7);
    println!("{}", deferred.get());
    let x = Cell::new(1);
    let y = Cell::new(2);
    x.swap(&y);
    println!("{} {}", x.get(), y.get());
    let old = x.replace({ x.set(9); 10 });
    println!("{} {}", old, x.get());

    let store = Store { items: RefCell::new(vec![String::from("a")]), count: Cell::new(1) };
    store.add("b");
    store.add("c");
    println!("{} {:?}", store.count.get(), store.items.borrow());
    let drained = store.drained();
    println!("{:?} {} {:?}", drained, store.items.borrow().len(), store.items.borrow());

    let cell_vec = RefCell::new(vec![1, 2, 3]);
    cell_vec.borrow_mut().push(4);
    let inner = cell_vec.into_inner();
    println!("{:?} {}", inner, inner.len());
}
