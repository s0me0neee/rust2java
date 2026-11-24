fn parse_stacks(drawing: &str) -> Vec<Vec<char>> {
    let lines: Vec<&str> = drawing.lines().collect();
    let count = lines.last().unwrap().split_whitespace().count();
    let mut stacks = vec![Vec::new(); count];
    for line in lines.iter().rev().skip(1) {
        let bytes = line.as_bytes();
        for (i, stack) in stacks.iter_mut().enumerate() {
            let pos = 1 + i * 4;
            if pos < bytes.len() && bytes[pos] != b' ' {
                stack.push(bytes[pos] as char);
            }
        }
    }
    stacks
}

fn main() {
    let drawing = "    [D]    \n[N] [C]    \n[Z] [M] [P]\n 1   2   3 ";
    let moves = "move 1 from 2 to 1\nmove 3 from 1 to 3\nmove 2 from 2 to 1\nmove 1 from 1 to 2";
    let mut a = parse_stacks(drawing);
    let mut b = a.clone();
    for m in moves.lines() {
        let nums: Vec<usize> = m.split(' ').filter_map(|w| w.parse().ok()).collect();
        let (n, from, to) = (nums[0], nums[1] - 1, nums[2] - 1);
        for _ in 0..n {
            let c = a[from].pop().unwrap();
            a[to].push(c);
        }
        let at = b[from].len() - n;
        let moved = b[from].split_off(at);
        b[to].extend(moved);
    }
    let tops = |s: &Vec<Vec<char>>| -> String { s.iter().filter_map(|st| st.last()).collect() };
    println!("{} {}", tops(&a), tops(&b));
}
