// Recursive data: Box in enum variants, Option<Box<Node>> walks with as_ref/as_mut/take.
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

struct Node {
    val: i32,
    next: Option<Box<Node>>,
}

impl Node {
    fn new(val: i32) -> Node {
        Node { val, next: None }
    }
    fn push(&mut self, val: i32) {
        self.next = Some(Box::new(Node { val, next: self.next.take() }));
    }
    fn len(&self) -> i32 {
        let mut n = 0;
        let mut cur = self.next.as_ref();
        while let Some(node) = cur {
            n += 1;
            cur = node.next.as_ref();
        }
        n
    }
    fn sum(&self) -> i32 {
        let mut s = 0;
        let mut cur = self.next.as_ref();
        while let Some(node) = cur {
            s += node.val;
            cur = node.next.as_ref();
        }
        s
    }
    fn scale(&mut self, by: i32) {
        let mut cur = self.next.as_mut();
        while let Some(node) = cur {
            node.val *= by;
            cur = node.next.as_mut();
        }
    }
}

fn main() {
    let e = Expr::Add(
        Box::new(Expr::Num(2)),
        Box::new(Expr::Mul(Box::new(Expr::Num(3)), Box::new(Expr::Num(4)))),
    );
    println!("{}", eval(&e));
    let b = Box::new(5);
    println!("{}", *b + 1);

    let mut list = Node::new(0);
    list.push(3);
    list.push(1);
    list.push(2);
    println!("{} {}", list.len(), list.sum());
    list.scale(10);
    println!("{}", list.sum());
    let taken = list.next.take();
    println!("{} {:?}", list.len(), taken.as_ref().map(|n| n.val));
    list.next = taken;
    println!("{}", list.sum());

    let list2: Option<Box<Node>> = Some(Box::new(Node { val: 4, next: None }));
    match list2 {
        Some(node) => println!("{}", node.val + node.next.is_none() as i32),
        None => println!("none"),
    }
}
