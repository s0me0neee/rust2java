// Singly linked list with Option<Box<Node>>: push/pop/reverse/iter/peek_mut, plus a stack-based RPN.
#[derive(Debug)]
struct Node<T> {
    val: T,
    next: Option<Box<Node<T>>>,
}

#[derive(Debug)]
struct List<T> {
    head: Option<Box<Node<T>>>,
    len: usize,
}

impl<T: std::fmt::Debug + PartialOrd + Copy> List<T> {
    fn new() -> Self {
        List { head: None, len: 0 }
    }
    fn push(&mut self, val: T) {
        let old = self.head.take();
        self.head = Some(Box::new(Node { val, next: old }));
        self.len += 1;
    }
    fn pop(&mut self) -> Option<T> {
        self.head.take().map(|node| {
            self.head = node.next;
            self.len -= 1;
            node.val
        })
    }
    fn peek(&self) -> Option<&T> {
        self.head.as_ref().map(|n| &n.val)
    }
    fn reverse(&mut self) {
        let mut prev: Option<Box<Node<T>>> = None;
        let mut cur = self.head.take();
        while let Some(mut node) = cur {
            cur = node.next.take();
            node.next = prev;
            prev = Some(node);
        }
        self.head = prev;
    }
    fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::new();
        let mut cur = &self.head;
        while let Some(node) = cur {
            out.push(node.val);
            cur = &node.next;
        }
        out
    }
    fn max(&self) -> Option<T> {
        let mut cur = self.head.as_deref();
        let mut best: Option<T> = None;
        while let Some(n) = cur {
            if best.map_or(true, |b| n.val > b) {
                best = Some(n.val);
            }
            cur = n.next.as_deref();
        }
        best
    }
}

fn rpn(expr: &str) -> Result<i64, String> {
    let mut stack: Vec<i64> = Vec::new();
    for tok in expr.split_whitespace() {
        if let Ok(n) = tok.parse::<i64>() {
            stack.push(n);
            continue;
        }
        let b = stack.pop().ok_or("stack underflow")?;
        let a = stack.pop().ok_or("stack underflow")?;
        let r = match tok {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => {
                if b == 0 {
                    return Err(format!("division by zero at '{}'", tok));
                }
                a / b
            }
            op => return Err(format!("unknown operator {:?}", op)),
        };
        stack.push(r);
    }
    match stack.as_slice() {
        [x] => Ok(*x),
        [] => Err("empty".into()),
        [.., last] => Err(format!("{} values left, top {}", stack.len(), last)),
    }
}

fn main() {
    let mut list = List::new();
    for x in [3, 1, 4, 1, 5, 9, 2, 6] {
        list.push(x);
    }
    println!("{:?} len={} peek={:?} max={:?}", list.to_vec(), list.len, list.peek(), list.max());
    list.reverse();
    println!("{:?}", list.to_vec());
    let popped: Vec<i32> = (0..3).filter_map(|_| list.pop()).collect();
    println!("popped {:?} rest {:?} len={}", popped, list.to_vec(), list.len);
    if let Some(node) = list.head.as_mut() {
        node.val *= 100;
    }
    println!("{:?}", list.to_vec());
    let mut empty: List<f64> = List::new();
    println!("{:?} {:?} {:?}", empty.pop(), empty.peek(), empty.max());
    empty.push(2.5);
    println!("{:?}", empty);

    for e in ["3 4 + 2 *", "5 1 2 + 4 * + 3 -", "1 0 /", "2 +", "1 2 3", "4 2 ^", ""] {
        match rpn(e) {
            Ok(v) => println!("{:<20} = {}", e, v),
            Err(msg) => println!("{:<20} ! {}", e, msg),
        }
    }
}
