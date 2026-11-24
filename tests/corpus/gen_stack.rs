use std::fmt::{self, Display};

#[derive(Debug, Default)]
struct Stack<T> {
    items: Vec<T>,
}

impl<T> Stack<T> {
    fn new() -> Self {
        Stack { items: Vec::new() }
    }
    fn push(&mut self, x: T) -> &mut Self {
        self.items.push(x);
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
    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl<T: Display> Display for Stack<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[")?;
        for (i, x) in self.items.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{x}")?;
        }
        write!(f, "]")
    }
}

fn eval_rpn(expr: &str) -> Result<i64, String> {
    let mut st: Stack<i64> = Stack::new();
    for tok in expr.split_whitespace() {
        if let Ok(n) = tok.parse::<i64>() {
            st.push(n);
            continue;
        }
        let b = st.pop().ok_or("stack underflow")?;
        let a = st.pop().ok_or("stack underflow")?;
        st.push(match tok {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" if b == 0 => return Err("division by zero".into()),
            "/" => a / b,
            other => return Err(format!("unknown token {other}")),
        });
    }
    if st.len() == 1 { Ok(st.pop().unwrap()) } else { Err(format!("leftover {}", st)) }
}

fn main() {
    let mut s = Stack::new();
    s.push(1).push(2).push(3);
    println!("{} peek={:?} len={}", s, s.peek(), s.len());
    while let Some(x) = s.pop() {
        print!("{x} ");
    }
    println!("empty={}", s.is_empty());
    for e in ["3 4 + 2 *", "5 1 2 + 4 * + 3 -", "1 +", "4 0 /", "2 3", "2 x +"] {
        println!("{e:>20} => {:?}", eval_rpn(e));
    }
}
