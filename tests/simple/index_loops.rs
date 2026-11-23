// Position-based iteration: enumerate, zip with a shifted copy, windows, chunks.
fn main() {
    let v = vec![3, 1, 4, 1, 5, 9, 2, 6];
    let w = vec![10, 20, 30];
    let diffs: Vec<i32> = v.iter().zip(v.iter().skip(1)).map(|(a, b)| b - a).collect();
    let dot: i32 = v.iter().zip(&w).map(|(a, b)| a * b).sum();
    let tagged: Vec<String> = w.iter().enumerate().map(|(i, x)| format!("{i}:{x}")).collect();
    let sorted = v.iter().zip(v.iter().skip(1)).all(|(a, b)| a <= b);
    println!("{:?} {} {:?} {}", diffs, dot, tagged, sorted);

    let s = "hello";
    for (i, c) in s.chars().enumerate() {
        if c == 'l' {
            print!("{i} ");
        }
    }
    println!();

    let text = "a b\nc d e";
    for (row, line) in text.lines().enumerate() {
        println!("{row}: {}", line.split_whitespace().count());
    }

    for pair in v.windows(2) {
        print!("{} ", pair[0] + pair[1]);
    }
    println!();
    for chunk in v.chunks(3) {
        print!("{} ", chunk.len());
    }
    println!();
    for (a, b) in v.iter().zip(v.iter().skip(1)) {
        if a > b {
            print!("{a}>{b} ");
        }
    }
    println!();
}
