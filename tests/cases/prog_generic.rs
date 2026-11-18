// Generic containers, trait with associated types/consts, impl Trait args/returns, closures stored in structs.
use std::collections::HashMap;
use std::fmt::{Debug, Display};

trait Animal {
    const LEGS: u32;
    fn name(&self) -> String;
    fn speak(&self) -> String;
}

struct Dog;
struct Bird {
    song: String,
}

impl Animal for Dog {
    const LEGS: u32 = 4;
    fn name(&self) -> String {
        "dog".into()
    }
    fn speak(&self) -> String {
        "woof".into()
    }
}
impl Animal for Bird {
    const LEGS: u32 = 2;
    fn name(&self) -> String {
        "bird".into()
    }
    fn speak(&self) -> String {
        self.song.repeat(2)
    }
}

fn intro<A: Animal>(a: &A) -> String {
    format!("{} ({} legs) says {}", a.name(), A::LEGS, a.speak())
}

#[derive(Debug)]
struct Stack<T> {
    items: Vec<T>,
}

impl<T: Debug + Clone> Stack<T> {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }
    fn push(&mut self, t: T) -> &mut Self {
        self.items.push(t);
        self
    }
    fn pop(&mut self) -> Option<T> {
        self.items.pop()
    }
    fn peek(&self) -> Option<&T> {
        self.items.last()
    }
    fn len(&self) -> usize {
        self.items.len()
    }
}

struct Counter<F: Fn(i32) -> bool> {
    pred: F,
    hits: u32,
}

impl<F: Fn(i32) -> bool> Counter<F> {
    fn feed(&mut self, xs: &[i32]) {
        for &x in xs {
            if (self.pred)(x) {
                self.hits += 1;
            }
        }
    }
}

fn make_adder(n: i32) -> impl Fn(i32) -> i32 {
    move |x| x + n
}

fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C {
    move |x| g(f(x))
}

fn apply_all(fs: &[Box<dyn Fn(i32) -> i32>], x: i32) -> Vec<i32> {
    fs.iter().map(|f| f(x)).collect()
}

fn show_all<T: Display>(xs: &[T]) -> String {
    xs.iter().map(|x| format!("[{}]", x)).collect::<Vec<_>>().join("")
}

fn memo_fib(n: u64, memo: &mut HashMap<u64, u64>) -> u64 {
    if n < 2 {
        return n;
    }
    if let Some(&v) = memo.get(&n) {
        return v;
    }
    let v = memo_fib(n - 1, memo) + memo_fib(n - 2, memo);
    memo.insert(n, v);
    v
}

#[derive(Debug, Clone, PartialEq)]
enum Tree<T> {
    Leaf,
    Node(Box<Tree<T>>, T, Box<Tree<T>>),
}

impl<T: PartialOrd + Copy + Debug> Tree<T> {
    fn insert(self, v: T) -> Tree<T> {
        match self {
            Tree::Leaf => Tree::Node(Box::new(Tree::Leaf), v, Box::new(Tree::Leaf)),
            Tree::Node(l, x, r) => {
                if v < x {
                    Tree::Node(Box::new(l.insert(v)), x, r)
                } else {
                    Tree::Node(l, x, Box::new(r.insert(v)))
                }
            }
        }
    }
    fn inorder(&self, out: &mut Vec<T>) {
        if let Tree::Node(l, x, r) = self {
            l.inorder(out);
            out.push(*x);
            r.inorder(out);
        }
    }
    fn depth(&self) -> usize {
        match self {
            Tree::Leaf => 0,
            Tree::Node(l, _, r) => 1 + l.depth().max(r.depth()),
        }
    }
}

fn main() {
    println!("{}", intro(&Dog));
    println!("{}", intro(&Bird { song: "tweet".to_string() }));

    let mut s: Stack<&str> = Stack::new();
    s.push("a").push("b").push("c");
    println!("{:?} peek={:?} len={}", s.pop(), s.peek(), s.len());
    println!("{:?}", s);

    let mut evens = Counter { pred: |x| x % 2 == 0, hits: 0 };
    evens.feed(&[1, 2, 3, 4, 6]);
    let limit = 3;
    let mut big = Counter { pred: move |x: i32| x.abs() > limit, hits: 0 };
    big.feed(&[-5, 2, 9]);
    println!("{} {}", evens.hits, big.hits);

    let add5 = make_adder(5);
    let f = compose(add5, |x| x * 3);
    let g = compose(|s: &str| s.len(), |n| n * n);
    println!("{} {}", f(1), g("abcd"));
    let fs: Vec<Box<dyn Fn(i32) -> i32>> = vec![Box::new(|x| x * x), Box::new(make_adder(-1)), Box::new(f)];
    println!("{:?}", apply_all(&fs, 4));
    println!("{} {}", show_all(&[1.5, 2.25]), show_all(&["x", "y"]));

    let mut memo = HashMap::new();
    println!("fib(80)={} memo={}", memo_fib(80, &mut memo), memo.len());

    let mut t = Tree::Leaf;
    for v in [5, 3, 8, 1, 4, 7, 9, 2] {
        t = t.insert(v);
    }
    let mut out = vec![];
    t.inorder(&mut out);
    println!("{:?} depth={}", out, t.depth());
    let small = Tree::Leaf.insert('m').insert('a');
    println!("{:?}", small);
    println!("{}", small == Tree::Leaf);
}
