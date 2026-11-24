use std::fmt;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct Clock {
    minutes: i32,
}

impl Clock {
    fn new(hours: i32, minutes: i32) -> Self {
        Clock { minutes: (hours * 60 + minutes).rem_euclid(24 * 60) }
    }

    fn add_minutes(&self, minutes: i32) -> Self {
        Clock::new(0, self.minutes + minutes)
    }
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:02}:{:02}", self.minutes / 60, self.minutes % 60)
    }
}

fn main() {
    let c = Clock::new(10, 37);
    println!("{} {} {}", c, c.add_minutes(-1000), Clock::new(-25, -160));
    println!("{}", Clock::new(24, 0) == Clock::new(0, 0));
    let times: Vec<String> = (0..4).map(|i| c.add_minutes(i * 400).to_string()).collect();
    println!("{}", times.join(" | "));
}
