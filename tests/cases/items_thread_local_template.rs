// The Codeforces template: a thread_local stdin buffer read through a read_token! macro.
thread_local! {
    static INPUT: (String, std::cell::Cell<usize>) = {
        use std::io::Read;
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s).unwrap();
        (s, std::cell::Cell::new(0))
    };
}

macro_rules! read_token {
    () => {{
        INPUT.with(|(s, pos)| {
            let rest = s[pos.get()..].trim_start();
            let start = s.len() - rest.len();
            let len = rest.find(char::is_whitespace).unwrap_or(rest.len());
            pos.set(start + len);
            rest[..len].to_string()
        })
    }};

    ($t:ty) => {{
        read_token!().parse::<$t>().unwrap()
    }};
}

fn solution() {
    let n = read_token!(usize);
    let mut total: i64 = 0;
    for _ in 0..n {
        total += read_token!(i64);
    }
    let name = read_token!();
    let ratio = read_token!(f64);
    println!("{} {} {:.2}", total, name, ratio);
}

fn main() {
    solution();
}

#[test]
fn test() {
    solution();
}
