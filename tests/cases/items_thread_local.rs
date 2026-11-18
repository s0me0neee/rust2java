// thread_local! statics: plain Java statics; `with` closures are inlined.
use std::cell::{Cell, RefCell};
thread_local! {
    static COUNTER: Cell<u32> = Cell::new(0);
    static LOG: RefCell<Vec<String>> = RefCell::new(Vec::new());
}
fn tick(tag: &str) -> u32 {
    COUNTER.with(|c| c.set(c.get() + 1));
    LOG.with(|log| log.borrow_mut().push(tag.to_string()));
    COUNTER.with(|c| c.get())
}
fn main() {
    tick("a");
    let n = tick("b");
    println!("{} {:?}", n, LOG.with(|log| log.borrow().join(",")));
}
