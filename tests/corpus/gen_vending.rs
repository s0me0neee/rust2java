#[derive(Debug, Clone, PartialEq)]
enum State {
    Idle,
    Collecting { cents: u32 },
    Dispensing { item: String, change: u32 },
    OutOfOrder(String),
}

#[derive(Debug)]
enum Event {
    Coin(u32),
    Select(&'static str),
    Cancel,
    Kick,
}

fn price(item: &str) -> Option<u32> {
    match item {
        "cola" => Some(150),
        "chips" => Some(100),
        "candy" => Some(65),
        _ => None,
    }
}

fn step(state: State, event: &Event) -> State {
    match (state, event) {
        (State::OutOfOrder(r), _) => State::OutOfOrder(r),
        (_, Event::Kick) => State::OutOfOrder("kicked".to_string()),
        (State::Idle, Event::Coin(c)) => State::Collecting { cents: *c },
        (State::Collecting { cents }, Event::Coin(c)) => State::Collecting { cents: cents + c },
        (State::Collecting { cents }, Event::Select(item)) => match price(item) {
            Some(p) if cents >= p => State::Dispensing { item: item.to_string(), change: cents - p },
            Some(_) | None => State::Collecting { cents },
        },
        (State::Collecting { .. }, Event::Cancel) => State::Idle,
        (State::Dispensing { .. }, Event::Coin(c)) => State::Collecting { cents: *c },
        (s, _) => s,
    }
}

fn main() {
    let events = [
        Event::Coin(100), Event::Select("cola"), Event::Coin(25), Event::Coin(25), Event::Select("cola"),
        Event::Coin(50), Event::Cancel, Event::Select("chips"), Event::Coin(100), Event::Select("gum"), Event::Kick, Event::Coin(5),
    ];
    let mut s = State::Idle;
    for e in &events {
        s = step(s, e);
        println!("{:<20} -> {:?}", format!("{:?}", e), s);
    }
    println!("{}", s == State::OutOfOrder("kicked".into()));
}
