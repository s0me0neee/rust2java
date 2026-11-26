// f64 / f32 methods. Transcendental results print to 12 decimals: Java's Math and the platform
// libm may differ in the last bit.
use std::num::FpCategory;

fn main() {
    let (x, y) = (2.5f64, -0.75f64);
    let nan = f64::NAN;

    // trigonometry and hyperbolics
    println!("{:.12} {:.12} {:.12} {:.12}", 0.5f64.sin(), 0.5f64.cos(), 0.5f64.tan(), 0.5f64.asin());
    println!("{:.12} {:.12} {:.12}", 0.5f64.acos(), 0.5f64.atan(), 1.0f64.atan2(-2.0));
    println!("{:.12} {:.12} {:.12}", 1.5f64.sinh(), 1.5f64.cosh(), 1.5f64.tanh());
    println!("{:.12} {:.12} {:.12}", 1.5f64.asinh(), 1.5f64.acosh(), 0.5f64.atanh());
    let (s, c) = 1.0f64.sin_cos();
    println!("{:.12} {:.12} {:.6}", s, c, 0.5f32.sin());

    // exponentials, logs, roots
    println!("{:.12} {:.12} {:.12} {:.12}", x.exp(), x.exp2(), 1e-10f64.exp_m1(), 1e-10f64.ln_1p());
    println!("{} {} {:.12} {}", 8.0f64.log2(), 1000.0f64.log10(), 81.0f64.log(3.0), 1024.0f32.log2());
    println!("{:.12} {} {} {}", 10.0f64.log2(), 27.0f64.cbrt(), 4.0f64.recip(), (-8.0f64).cbrt());

    // sign, classification
    println!("{} {} {} {}", 0.0f64.signum(), (-0.0f64).signum(), y.signum(), nan.signum().is_nan());
    println!("{} {} {} {}", (-0.0f64).is_sign_negative(), 0.0f64.is_sign_positive(), y.is_sign_negative(), nan.is_sign_positive());
    println!("{} {} {} {}", x.is_normal(), 0.0f64.is_normal(), 1e-310f64.is_subnormal(), x.is_subnormal());
    println!("{} {} {}", f64::INFINITY.is_infinite(), x.is_infinite(), 1e-40f32.is_subnormal());
    println!("{:?} {:?} {:?} {:?} {:?}", x.classify(), 0.0f64.classify(), nan.classify(), f64::NEG_INFINITY.classify(), 1e-310f64.classify());
    println!("{}", 1e-310f64.classify() == FpCategory::Subnormal);

    // rounding, remainders, min / max with NaN, fused multiply-add
    println!("{} {} {} {}", 2.5f64.round_ties_even(), 3.5f64.round_ties_even(), (-2.5f64).round_ties_even(), 2.5f32.round_ties_even());
    println!("{} {} {} {}", 7.5f64.div_euclid(-2.0), (-7.5f64).div_euclid(2.0), (-7.5f64).rem_euclid(2.0), 7.5f64.rem_euclid(-2.0));
    println!("{} {} {} {}", nan.max(1.0), 1.0f64.min(nan), x.max(y), x.min(y));
    println!("{} {} {}", 0.1f64.mul_add(10.0, -1.0), x.midpoint(y), f64::MAX.midpoint(f64::MAX));
    println!("{} {} {}", 1.0f64.copysign(-0.0), 90.0f64.to_radians() / std::f64::consts::PI, std::f64::consts::PI.to_degrees());
    println!("{} {} {} {}", x.algebraic_add(y), x.algebraic_sub(y), x.algebraic_mul(y), x.algebraic_div(y));
    println!("{}", 7.5f64.algebraic_rem(2.0));

    // neighbours and bits
    // Java prints the smallest subnormal as 4.9E-324 (not shortest), so compare it instead.
    println!("{} {} {}", 1.0f64.next_up(), 1.0f64.next_down(), 0.0f64.next_up() == f64::from_bits(1));
    println!("{} {} {} {}", 1.0f64.to_bits(), (-2.0f32).to_bits(), f64::from_bits(0x4004000000000000), f32::from_bits(0x3fc00000));
    println!("{:?} {:?} {:?}", 1.5f64.to_be_bytes(), 1.5f32.to_le_bytes(), 1.5f32.to_ne_bytes());
    println!("{} {} {}", f64::from_be_bytes([63, 248, 0, 0, 0, 0, 0, 0]), f32::from_le_bytes([0, 0, 192, 63]), f32::from_ne_bytes([0, 0, 32, 64]));
}
