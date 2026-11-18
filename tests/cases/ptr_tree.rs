// Rc<RefCell<Node>> trees: shared ownership, mutation through &self, borrow walks.
use std::cell::RefCell;
use std::rc::Rc;

struct Node {
    val: i32,
    children: Vec<Rc<RefCell<Node>>>,
}

fn node(val: i32, children: Vec<Rc<RefCell<Node>>>) -> Rc<RefCell<Node>> {
    Rc::new(RefCell::new(Node { val, children }))
}

fn sum(n: &Rc<RefCell<Node>>) -> i32 {
    let mut total = n.borrow().val;
    let kids = n.borrow().children.clone();
    for kid in &kids {
        total += sum(kid);
    }
    total
}

fn bump_all(n: &Rc<RefCell<Node>>) {
    n.borrow_mut().val += 1;
    let kids = n.borrow().children.clone();
    for kid in &kids {
        bump_all(kid);
    }
}

fn main() {
    let leaf = node(3, vec![]);
    let mid = node(2, vec![Rc::clone(&leaf)]);
    let root = node(1, vec![Rc::clone(&mid), Rc::clone(&leaf)]);
    println!("{}", sum(&root));
    bump_all(&root);
    println!("{} {} {}", root.borrow().val, mid.borrow().val, leaf.borrow().val);
    let same = Rc::clone(&leaf);
    println!("{}", Rc::ptr_eq(&leaf, &same));
    let total: i32 = root.borrow().children.iter().map(|c| c.borrow().val).sum();
    println!("{}", total);
    let names: Vec<String> =
        root.borrow().children.iter().map(|c| format!("n{}", c.borrow().val)).collect();
    println!("{:?}", names);
}
