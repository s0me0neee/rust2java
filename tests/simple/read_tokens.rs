// Contest-style token reader: a byte at a time from stdin.
macro_rules! read_token {
    () => {{
        use std::io::{self, Read};
        let mut token = Vec::new();
        let mut buf = [0u8; 1];

        loop {
            let n = io::stdin().read(&mut buf).unwrap();
            if n == 0 || !buf[0].is_ascii_whitespace() {
                break;
            }
        }
        token.push(buf[0]);

        loop {
            let n = io::stdin().read(&mut buf).unwrap();
            if n == 0 || buf[0].is_ascii_whitespace() {
                break;
            }
            token.push(buf[0]);
        }

        String::from_utf8(token).unwrap()
    }};

    ($t:ty) => {{
        let s = read_token!();
        s.parse::<$t>().unwrap()
    }};
}

fn main() {
    let n = read_token!(usize);
    let mut total: u64 = 0;
    for _ in 0..n {
        let s: String = read_token!(String);
        let digits: u64 = s.chars().filter(|c| c.is_ascii_digit()).count() as u64;
        total += digits * read_token!(u64);
    }
    println!("{}", total);
}
