// Vec/slice API sweep: retain, dedup, drain, extend, truncate, split_off, swap, rotate, chunks_exact,
// partition, unzip, windows, concat, iter().rev(), contains, binary_search, sort_by partial_cmp.
fn main() {
    let mut v: Vec<i32> = (1..=12).collect();
    v.retain(|x| x % 3 != 0);
    println!("{:?}", v);
    let drained: Vec<i32> = v.drain(1..3).collect();
    println!("{:?} {:?}", drained, v);
    v.extend([20, 20, 21, 21, 21, 5].iter());
    v.dedup();
    println!("{:?}", v);
    let tail = v.split_off(5);
    println!("{:?} | {:?}", v, tail);
    v.truncate(3);
    v.swap(0, 2);
    v.insert(1, 99);
    let removed = v.remove(0);
    println!("{:?} removed={}", v, removed);
    let mut r: Vec<char> = "abcdef".chars().collect();
    r.rotate_left(2);
    let s1: String = r.iter().collect();
    r.rotate_right(3);
    let s2: String = r.iter().rev().collect();
    println!("{} {}", s1, s2);
    let data = [4, 8, 15, 16, 23, 42, 7];
    let sums: Vec<i32> = data.chunks_exact(3).map(|c| c.iter().sum()).collect();
    let rem = data.chunks_exact(3).remainder();
    println!("{:?} rem={:?}", sums, rem);
    let (even, odd): (Vec<i32>, Vec<i32>) = data.iter().partition(|&&x| x % 2 == 0);
    println!("{:?} {:?}", even, odd);
    let (idx, vals): (Vec<usize>, Vec<i32>) = data.iter().enumerate().filter(|(_, x)| **x > 10).map(|(i, &x)| (i, x)).unzip();
    println!("{:?} {:?}", idx, vals);
    let increasing = data.windows(2).filter(|w| w[0] < w[1]).count();
    let nested = vec![vec![1, 2], vec![], vec![3]];
    println!("{} {:?} {:?}", increasing, nested.concat(), nested.iter().map(|v| v.len()).collect::<Vec<_>>());
    let mut sorted = data.to_vec();
    sorted.sort_unstable();
    println!("{:?} {:?} {:?}", sorted.binary_search(&16), sorted.binary_search(&5).unwrap_err(), sorted.contains(&42));
    let mut fs = vec![2.5, -1.0, 9.75, 0.0, 3.25];
    fs.sort_by(|a, b| b.partial_cmp(a).unwrap());
    println!("{:?} {:?}", fs, fs.iter().cloned().fold(f64::INFINITY, f64::min));
    let total: f64 = fs.iter().sum();
    let prod: i64 = (1..=15).product();
    println!("{:.2} {} {}", total, prod, (1..=5).map(|x| x * x).rev().map(|x| x.to_string()).collect::<Vec<_>>().join("<"));
    let words = vec!["apple", "bob", "kayak", "level", "rust"];
    let (pal, non): (Vec<&str>, Vec<&str>) = words.iter().partition(|w| w.chars().eq(w.chars().rev()));
    println!("{:?} {:?} first>4={:?} last={:?}", pal, non, words.iter().find(|w| w.len() > 4), words.last());
    println!("{} {} {:?}", words.iter().any(|w| w.starts_with('k')), words.iter().all(|w| w.len() >= 3), words.iter().map(|w| w.len()).max());
    let mut grid = vec![vec!['.'; 4]; 3];
    grid[1][2] = '#';
    grid[2].fill('~');
    for row in &grid {
        println!("{}", row.iter().collect::<String>());
    }
    let flat: Vec<(usize, usize)> = (0..3).flat_map(|r| (0..2).map(move |c| (r, c))).collect();
    println!("{:?} {}", flat, flat.len());
    let mut stack = vec![1];
    while let Some(top) = stack.pop() {
        if top < 20 {
            stack.push(top * 3);
            stack.push(top * 2);
        } else {
            print!("{} ", top);
        }
    }
    println!();
    println!("{:?}", v.first().zip(v.last()));
    println!("{:?}", data.iter().skip_while(|&&x| x < 15).take_while(|&&x| x != 42).collect::<Vec<_>>());
    println!("{:?}", data.iter().step_by(2).copied().collect::<Vec<i32>>());
    println!("{:?}", data.iter().rposition(|&x| x > 10));
}
