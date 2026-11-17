use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
#[derive(Debug)]
struct Node {
    val: i32,
    children: Vec<Rc<RefCell<Node>>>,
}
#[derive(Debug)]
enum Expr {
    Num(i64),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
}
fn eval(e: &Expr) -> i64 {
    match e {
        Expr::Num(n) => *n,
        Expr::Add(a, b) => eval(a) + eval(b),
        Expr::Mul(a, b) => eval(a) * eval(b),
    }
}
struct Counter {
    hits: Cell<u32>,
}
fn main() {
    let e = Expr::Add(Box::new(Expr::Num(2)), Box::new(Expr::Mul(Box::new(Expr::Num(3)), Box::new(Expr::Num(4)))));
    println!("{}", eval(&e));
    let b = Box::new(5);
    println!("{}", *b + 1);
    let shared = Rc::new(RefCell::new(vec![1, 2]));
    let other = Rc::clone(&shared);
    other.borrow_mut().push(3);
    println!("{:?} {}", shared.borrow(), Rc::strong_count(&shared));
    let leaf = Rc::new(RefCell::new(Node { val: 3, children: vec![] }));
    let root = Rc::new(RefCell::new(Node { val: 1, children: vec![leaf.clone()] }));
    leaf.borrow_mut().val = 30;
    println!("{}", root.borrow().children[0].borrow().val);
    let total: i32 = root.borrow().children.iter().map(|c| c.borrow().val).sum();
    println!("{}", total);
    let c = Counter { hits: Cell::new(0) };
    c.hits.set(c.hits.get() + 1);
    println!("{}", c.hits.get());
    let cache: RefCell<HashMap<u64, u64>> = RefCell::new(HashMap::new());
    cache.borrow_mut().insert(1, 1);
    println!("{:?}", cache.borrow().get(&1));
    let s: Rc<str> = Rc::from("abc");
    println!("{}", s.len());
    let list: Option<Box<i32>> = Some(Box::new(4));
    if let Some(bx) = &list {
        println!("{}", **bx * 2);
    }
    let v = shared.borrow().len();
    println!("{}", v);
    let mut x = 5;
    {
        let r = &mut x;
        *r += 1;
    }
    println!("{}", x);
    let fns: Vec<Box<dyn Fn(i32) -> i32>> = vec![Box::new(|x| x + 1), Box::new(|x| x * 10)];
    println!("{:?}", fns.iter().map(|f| f(2)).collect::<Vec<_>>());
    let add = make_adder(5);
    println!("{}", add(1));
    println!("{}", apply(|x| x - 1, 10));
}
fn make_adder(n: i32) -> impl Fn(i32) -> i32 {
    move |x| x + n
}
fn apply<F: Fn(i32) -> i32>(f: F, v: i32) -> i32 {
    f(v)
}
