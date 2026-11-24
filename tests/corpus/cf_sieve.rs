fn sieve(n: usize) -> Vec<usize> {
    let mut is = vec![true; n + 1];
    is[0] = false;
    if n >= 1 {
        is[1] = false;
    }
    let mut i = 2;
    while i * i <= n {
        if is[i] {
            for j in (i * i..=n).step_by(i) {
                is[j] = false;
            }
        }
        i += 1;
    }
    is.iter().enumerate().filter(|(_, &p)| p).map(|(i, _)| i).collect()
}

fn factorize(mut n: u64) -> Vec<(u64, u32)> {
    let mut out = Vec::new();
    let mut p = 2;
    while p * p <= n {
        let mut e = 0;
        while n % p == 0 {
            n /= p;
            e += 1;
        }
        if e > 0 {
            out.push((p, e));
        }
        p += 1;
    }
    if n > 1 {
        out.push((n, 1));
    }
    out
}

fn main() {
    let primes = sieve(100);
    println!("{} primes, last {:?}", primes.len(), primes.last());
    let twins: Vec<(usize, usize)> = primes.windows(2).filter(|w| w[1] - w[0] == 2).map(|w| (w[0], w[1])).collect();
    println!("{:?}", &twins[..5]);
    for n in [360u64, 97, 1_000_000_007 * 3, 1] {
        let f: Vec<String> = factorize(n).iter().map(|(p, e)| if *e > 1 { format!("{p}^{e}") } else { p.to_string() }).collect();
        println!("{n} = {}", f.join(" * "));
    }
}
