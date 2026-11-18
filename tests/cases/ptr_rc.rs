// Rc / Arc sharing, Rc<str>, Box<str>, Rc::ptr_eq.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn main() {
    let shared = Rc::new(RefCell::new(vec![1, 2]));
    let other = Rc::clone(&shared);
    other.borrow_mut().push(3);
    shared.borrow_mut().push(4);
    println!("{:?} {}", shared.borrow(), shared.borrow().len());
    println!("{}", Rc::ptr_eq(&shared, &other));
    let v = shared.borrow().len();
    println!("{v}");

    let text: Rc<RefCell<String>> = Rc::new(RefCell::new(String::from("ab")));
    let t2 = Rc::clone(&text);
    t2.borrow_mut().push_str("cd");
    println!("{} {}", text.borrow(), text.borrow().len());

    let s: Rc<str> = Rc::from("abcd");
    let b: Box<str> = Box::from("ef");
    println!("{} {} {}", s.len(), b.len(), &*s);
    let owned: Box<String> = Box::new(String::from("gh"));
    println!("{}", owned.len());

    let arc = Arc::new(vec![7, 8]);
    let arc2 = Arc::clone(&arc);
    println!("{} {} {}", arc.len(), arc2.len(), Arc::ptr_eq(&arc, &arc2));
}
