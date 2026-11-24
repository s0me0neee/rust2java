fn main() {
    let weights = [3u32, 5, 7, 11, 13];
    let n = weights.len();
    let target = 21;
    let mut hits = Vec::new();
    for mask in 0u32..(1 << n) {
        let sum: u32 = (0..n).filter(|&i| mask >> i & 1 == 1).map(|i| weights[i]).sum();
        if sum == target {
            hits.push(mask);
        }
    }
    for m in &hits {
        println!("{:05b} bits={} low={}", m, m.count_ones(), m.trailing_zeros());
    }
    let mut x: u64 = 0b1011_0000;
    let mut subs = Vec::new();
    let mut s = x;
    loop {
        subs.push(s);
        if s == 0 {
            break;
        }
        s = (s - 1) & x;
    }
    println!("{} subsets of {:#x}", subs.len(), x);
    x ^= 1 << 63;
    println!("{} {} {}", x.leading_zeros(), (x >> 60), u64::MAX.count_ones());
}
