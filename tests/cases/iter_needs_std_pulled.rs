fn tokens(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = s.chars().peekable();
    while let Some(&c) = it.peek() {
        if c.is_ascii_digit() {
            let mut num = String::new();
            while let Some(&d) = it.peek() {
                if d.is_ascii_digit() {
                    num.push(d);
                    it.next();
                } else {
                    break;
                }
            }
            out.push(num);
        } else {
            if c != ' ' {
                out.push(c.to_string());
            }
            it.next();
        }
    }
    out
}

fn main() {
    println!("{:?}", tokens("12+ 345*6"));
    let v = vec![10, 20, 30];
    let mut it = v.iter();
    println!("{:?}", it.next());
    println!("{:?}", it.next());
}
