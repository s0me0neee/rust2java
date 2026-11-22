fn main() {
    let n = 2_000usize;
    let v: Vec<u64> = (0..n as u64).collect();
    let mut s = 0u64;
    for i in 0..n {
        s += v.iter().nth(i).unwrap();
        s += v.iter().last().unwrap();
        s += *v.iter().rev().next().unwrap();
        s += v[i..].len() as u64;
        s += v.iter().skip(i).next().unwrap();
        s += v.chunks(1000).count() as u64;
    }
    println!("{s}");
}
