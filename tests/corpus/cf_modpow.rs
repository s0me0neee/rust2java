const MOD: u64 = 1_000_000_007;

fn pow_mod(mut b: u64, mut e: u64, m: u64) -> u64 {
    let mut r = 1;
    b %= m;
    while e > 0 {
        if e & 1 == 1 {
            r = r * b % m;
        }
        b = b * b % m;
        e >>= 1;
    }
    r
}

type Mat = [[u64; 2]; 2];

fn mul(a: &Mat, b: &Mat) -> Mat {
    let mut c = [[0; 2]; 2];
    for i in 0..2 {
        for j in 0..2 {
            for k in 0..2 {
                c[i][j] = (c[i][j] + a[i][k] * b[k][j]) % MOD;
            }
        }
    }
    c
}

fn fib(mut n: u64) -> u64 {
    let mut result: Mat = [[1, 0], [0, 1]];
    let mut base: Mat = [[1, 1], [1, 0]];
    while n > 0 {
        if n % 2 == 1 {
            result = mul(&result, &base);
        }
        base = mul(&base, &base);
        n /= 2;
    }
    result[0][1]
}

fn main() {
    println!("{}", pow_mod(2, 1_000_000, MOD));
    println!("{}", pow_mod(3, MOD - 2, MOD) * 3 % MOD);
    for n in [1, 2, 10, 50, 90, 1000] {
        println!("fib({n}) mod p = {}", fib(n));
    }
    let fact: Vec<u64> = (1..=20).scan(1u64, |acc, i| {
        *acc = *acc * i % MOD;
        Some(*acc)
    }).collect();
    println!("{:?}", &fact[15..]);
}
