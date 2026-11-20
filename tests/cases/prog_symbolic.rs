// Expression tree: Box<Expr> recursion, simplification by nested pattern matching with @ bindings
// and guards, Display with precedence-aware parentheses, environment lookups, derivative.
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
enum Expr {
    Num(f64),
    Var(String),
    Add(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Neg(Box<Expr>),
    Pow(Box<Expr>, u32),
}

use Expr::*;

fn num(n: f64) -> Expr {
    Num(n)
}
fn var(s: &str) -> Expr {
    Var(s.to_string())
}
fn add(a: Expr, b: Expr) -> Expr {
    Add(Box::new(a), Box::new(b))
}
fn mul(a: Expr, b: Expr) -> Expr {
    Mul(Box::new(a), Box::new(b))
}

impl Expr {
    fn prec(&self) -> u8 {
        match self {
            Add(..) => 1,
            Mul(..) => 2,
            Neg(_) => 3,
            Pow(..) => 4,
            _ => 5,
        }
    }

    fn eval(&self, env: &HashMap<&str, f64>) -> Result<f64, String> {
        Ok(match self {
            Num(n) => *n,
            Var(v) => *env.get(v.as_str()).ok_or(format!("unbound {}", v))?,
            Add(a, b) => a.eval(env)? + b.eval(env)?,
            Mul(a, b) => a.eval(env)? * b.eval(env)?,
            Neg(a) => -a.eval(env)?,
            Pow(a, k) => a.eval(env)?.powi(*k as i32),
        })
    }

    fn simplify(self) -> Expr {
        match self {
            Add(a, b) => match (a.simplify(), b.simplify()) {
                (Num(x), Num(y)) => Num(x + y),
                (Num(z), e) | (e, Num(z)) if z == 0.0 => e,
                (x, y) if x == y => mul(num(2.0), x),
                (x, y) => add(x, y),
            },
            Mul(a, b) => match (a.simplify(), b.simplify()) {
                (Num(x), Num(y)) => Num(x * y),
                (Num(z), _) | (_, Num(z)) if z == 0.0 => Num(0.0),
                (Num(o), e) | (e, Num(o)) if o == 1.0 => e,
                (x, y) => mul(x, y),
            },
            Neg(inner) => match inner.simplify() {
                Num(n) => Num(-n),
                Neg(e) => *e,
                e => Neg(Box::new(e)),
            },
            Pow(base, 0) => {
                let _ = base;
                Num(1.0)
            }
            Pow(base, 1) => base.simplify(),
            Pow(base, k) => Pow(Box::new(base.simplify()), k),
            e => e,
        }
    }

    fn derive(&self, x: &str) -> Expr {
        match self {
            Num(_) => num(0.0),
            Var(v) if v == x => num(1.0),
            Var(_) => num(0.0),
            Add(a, b) => add(a.derive(x), b.derive(x)),
            Mul(a, b) => add(mul(a.derive(x), (**b).clone()), mul((**a).clone(), b.derive(x))),
            Neg(a) => Neg(Box::new(a.derive(x))),
            Pow(a, k) => mul(mul(num(*k as f64), Pow(a.clone(), k - 1)), a.derive(x)),
        }
    }

    fn vars(&self, out: &mut Vec<String>) {
        match self {
            Var(v) if !out.contains(v) => out.push(v.clone()),
            Add(a, b) | Mul(a, b) => {
                a.vars(out);
                b.vars(out);
            }
            Neg(a) | Pow(a, _) => a.vars(out),
            _ => {}
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wrap = |e: &Expr, f: &mut fmt::Formatter<'_>| -> fmt::Result {
            if e.prec() < self.prec() { write!(f, "({})", e) } else { write!(f, "{}", e) }
        };
        match self {
            Num(n) => write!(f, "{}", n),
            Var(v) => write!(f, "{}", v),
            Add(a, b) => {
                wrap(a, f)?;
                write!(f, " + ")?;
                wrap(b, f)
            }
            Mul(a, b) => {
                wrap(a, f)?;
                write!(f, "*")?;
                wrap(b, f)
            }
            Neg(a) => {
                write!(f, "-")?;
                wrap(a, f)
            }
            Pow(a, k) => {
                wrap(a, f)?;
                write!(f, "^{}", k)
            }
        }
    }
}

fn classify(e: &Expr) -> String {
    match e {
        Num(n @ 0.0..=9.0) => format!("digit {}", n),
        Num(n) if n.fract() == 0.0 => format!("int {}", n),
        Num(_) => "real".into(),
        Add(l, r) if matches!(**l, Num(_)) && matches!(**r, Num(_)) => "constant sum".into(),
        whole @ Mul(..) => format!("product of depth {}", depth(whole)),
        other => format!("{:?}", other).chars().take(12).collect(),
    }
}

fn depth(e: &Expr) -> usize {
    match e {
        Add(a, b) | Mul(a, b) => 1 + depth(a).max(depth(b)),
        Neg(a) | Pow(a, _) => 1 + depth(a),
        _ => 1,
    }
}

fn main() {
    let e = add(mul(num(3.0), Pow(Box::new(var("x")), 2)), add(mul(num(1.0), var("y")), num(0.0)));
    println!("{}", e);
    let s = e.clone().simplify();
    println!("{}", s);
    let mut env = HashMap::new();
    env.insert("x", 2.0);
    println!("{:?}", s.eval(&env));
    env.insert("y", -1.5);
    println!("{:?}", s.eval(&env));
    let d = e.derive("x").simplify();
    println!("d/dx = {}", d);
    println!("d/dy = {}", e.derive("y").simplify());
    let mut vs = Vec::new();
    e.vars(&mut vs);
    println!("{:?} depth {}", vs, depth(&e));
    let n = Neg(Box::new(Neg(Box::new(add(var("a"), var("a"))))));
    println!("{} => {}", n, n.clone().simplify());
    for t in [num(7.0), num(12.0), num(2.5), add(num(1.0), num(2.0)), mul(var("q"), num(4.0)), var("zz")] {
        println!("{:<10} {}", t.to_string(), classify(&t));
    }
    println!("{}", Pow(Box::new(add(var("a"), num(1.0))), 3));
}
