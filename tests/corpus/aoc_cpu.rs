enum Op {
    Noop,
    Addx(i64),
}

fn main() {
    let program = "addx 15\naddx -11\naddx 6\naddx -3\naddx 5\naddx -1\naddx -8\naddx 13\naddx 4\nnoop\naddx -1\naddx 5\naddx -1\naddx 5\naddx -1\naddx 5\naddx -1\naddx 5\naddx -1\naddx -35";
    let ops: Vec<Op> = program
        .lines()
        .map(|l| match l.split_once(' ') {
            Some(("addx", v)) => Op::Addx(v.parse().unwrap()),
            None if l == "noop" => Op::Noop,
            _ => panic!("bad op: {l}"),
        })
        .collect();
    let mut x = 1i64;
    let mut xs = vec![x];
    for op in &ops {
        match op {
            Op::Noop => xs.push(x),
            Op::Addx(v) => {
                xs.push(x);
                x += v;
                xs.push(x);
            }
        }
    }
    let strength: i64 = [20, 40, 60].iter().map(|&c| c as i64 * xs[c - 1]).sum();
    println!("strength {strength}");
    let mut row = String::new();
    for (i, &x) in xs.iter().take(40).enumerate() {
        row.push(if (i as i64 - x).abs() <= 1 { '#' } else { '.' });
    }
    println!("{row}");
}
