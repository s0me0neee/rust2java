// Numeric edge cases: wrapping/checked/saturating/overflowing ops, bit tricks, radix parsing,
// float rounding, integer casts between widths, gcd/lcm/primes/modpow.
fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 { a } else { gcd(b, a % b) }
}

fn modpow(mut base: u64, mut exp: u64, m: u64) -> u64 {
    let mut result = 1u64;
    base %= m;
    while exp > 0 {
        if exp & 1 == 1 {
            result = result * base % m;
        }
        base = base * base % m;
        exp >>= 1;
    }
    result
}

fn sieve(n: usize) -> Vec<usize> {
    let mut is = vec![true; n + 1];
    is[0] = false;
    if n >= 1 {
        is[1] = false;
    }
    let mut i = 2;
    while i * i <= n {
        if is[i] {
            let mut j = i * i;
            while j <= n {
                is[j] = false;
                j += i;
            }
        }
        i += 1;
    }
    is.iter().enumerate().filter(|(_, p)| **p).map(|(i, _)| i).collect()
}

fn main() {
    let a: i32 = i32::MAX;
    println!("{} {:?} {:?} {}", a.wrapping_add(1), a.checked_add(1), a.overflowing_add(1), a.saturating_add(1));
    let b: u8 = 200;
    println!("{} {:?} {} {}", b.wrapping_mul(2), b.checked_mul(2), b.saturating_sub(250), (b as i8));
    let c: i64 = -9_000_000_000;
    println!("{} {} {} {}", c as i32, c as u32, (c as f64) / 1e9, c.unsigned_abs());
    let big: u64 = u64::MAX - 5;
    println!("{} {} {} {}", big, big / 7, big % 1000, big.count_ones());
    let n: u32 = 0b1011_0000;
    println!("{} {} {} {} {}", n.count_ones(), n.leading_zeros(), n.trailing_zeros(), n.rotate_left(28), n.swap_bytes());
    println!("{} {} {}", 1u64 << 40, -1i32 >> 1, (-1i32 as u32) >> 1);
    println!("{:?} {:?} {:?}", i64::from_str_radix("-ff", 16), u8::from_str_radix("101", 2), i32::from_str_radix("zz", 36));
    println!("{} {} {} {}", 7.5f64.round(), (-7.5f64).round(), 2.5f64.floor(), (-2.5f64).ceil());
    println!("{} {} {:.3} {:.0}", 10f64.powi(-2), 2f64.powf(0.5), std::f64::consts::E.ln(), 1234.5678);
    println!("{} {} {}", (0.1 + 0.2 == 0.3), ((0.1f64 + 0.2 - 0.3).abs() < f64::EPSILON), 1.0 / 3.0);
    println!("{} {} {}", 255u8 as char, 'A' as u32 + 2, ((b'a' + 25) as char).to_ascii_uppercase());
    println!("{} {} {}", gcd(1071, 462), 1071 / gcd(1071, 462) * 462, modpow(2, 100, 1_000_000_007));
    let primes = sieve(60);
    println!("{:?} sum={} count={}", &primes[primes.len() - 4..], primes.iter().sum::<usize>(), primes.len());
    let digits: u32 = 9875u32.to_string().chars().map(|d| d.to_digit(10).unwrap()).sum();
    println!("{} {} {}", digits, i32::pow(3, 4), u64::pow(10, 18));
    println!("{} {} {}", (17i32).div_euclid(-5), (17i32).rem_euclid(-5), (-17i32) % 5);
    println!("{} {} {}", i16::MAX as i32 + 1, (40000u32 as u16) as i16, (3.99f64 as i32) + (-3.99f64 as i32));
    println!("{} {} {}", (300i32 as u8), (-1i64 as u8), (1e10 as i32));
    println!("{:?} {:?}", 10u32.checked_div(0), 10i32.checked_rem(3));
    println!("{} {}", u32::MAX.wrapping_add(2), 5u32.abs_diff(9));
    println!("{} {} {}", isqrt(99), isqrt(100), (2.0f64).sqrt().mul_add(2.0, 1.0));
    println!("{} {}", f64::MAX > 1e308, i8::MIN.wrapping_abs());
}

fn isqrt(n: u64) -> u64 {
    (n as f64).sqrt() as u64
}
