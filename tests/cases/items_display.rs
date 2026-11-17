// `impl Display` becomes `toString()` with a StringBuilder.
use std::fmt;

struct Money {
    cents: i64,
}

impl Money {
    fn new(cents: i64) -> Money {
        Money { cents }
    }
}

impl fmt::Display for Money {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.cents < 0 {
            write!(f, "-{} cents", -self.cents)?;
        } else {
            write!(f, "{} cents", self.cents)?;
        }
        Ok(())
    }
}

fn main() {
    let m = Money::new(-250);
    println!("{}", m);
    println!("{}", Money::new(99).to_string());
    print!("[{}]", Money::new(1));
    println!("[{}]", Money::new(2));
}
