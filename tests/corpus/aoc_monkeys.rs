use std::collections::VecDeque;

struct Monkey {
    items: VecDeque<u64>,
    op: Box<dyn Fn(u64) -> u64>,
    div: u64,
    if_true: usize,
    if_false: usize,
    inspected: u64,
}

fn monkeys() -> Vec<Monkey> {
    vec![
        Monkey { items: VecDeque::from(vec![79, 98]), op: Box::new(|x| x * 19), div: 23, if_true: 2, if_false: 3, inspected: 0 },
        Monkey { items: VecDeque::from(vec![54, 65, 75, 74]), op: Box::new(|x| x + 6), div: 19, if_true: 2, if_false: 0, inspected: 0 },
        Monkey { items: VecDeque::from(vec![79, 60, 97]), op: Box::new(|x| x * x), div: 13, if_true: 1, if_false: 3, inspected: 0 },
        Monkey { items: VecDeque::from(vec![74]), op: Box::new(|x| x + 3), div: 17, if_true: 0, if_false: 1, inspected: 0 },
    ]
}

fn run(rounds: usize, relief: bool) -> u64 {
    let mut ms = monkeys();
    let modulus: u64 = ms.iter().map(|m| m.div).product();
    for _ in 0..rounds {
        for i in 0..ms.len() {
            while let Some(item) = ms[i].items.pop_front() {
                ms[i].inspected += 1;
                let mut w = (ms[i].op)(item);
                if relief {
                    w /= 3;
                } else {
                    w %= modulus;
                }
                let target = if w % ms[i].div == 0 { ms[i].if_true } else { ms[i].if_false };
                ms[target].items.push_back(w);
            }
        }
    }
    let mut counts: Vec<u64> = ms.iter().map(|m| m.inspected).collect();
    counts.sort_unstable_by(|a, b| b.cmp(a));
    counts[0] * counts[1]
}

fn main() {
    println!("{}", run(20, true));
    println!("{}", run(10000, false));
}
