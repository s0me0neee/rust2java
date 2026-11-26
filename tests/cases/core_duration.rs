// std::time::Duration: constructors, accessors, checked / saturating arithmetic, float scaling,
// operators and Debug formatting.
use std::time::Duration;

fn main() {
    let a = Duration::new(2, 500_000_000);
    let b = Duration::from_millis(750);
    println!("{:?} {:?} {:?} {:?}", a, b, Duration::from_micros(1500), Duration::from_nanos(42));
    println!("{:?} {:?} {:?}", Duration::from_mins(2), Duration::from_hours(1), Duration::from_nanos_u128(3_000_000_001));
    println!("{:?} {:?} {:?}", Duration::from_secs(0), Duration::ZERO, Duration::from_secs(90));
    println!("{:?} {:?}", Duration::from_secs_f64(1.25), Duration::from_secs_f32(0.5));
    println!("{} {}", Duration::try_from_secs_f64(-1.0).is_err(), Duration::try_from_secs_f32(2.0).is_ok());

    println!("{} {} {} {}", a.as_secs(), a.as_millis(), a.as_micros(), a.as_nanos());
    println!("{} {} {}", a.as_secs_f64(), b.as_secs_f32(), a.is_zero());
    println!("{} {} {}", a.subsec_millis(), a.subsec_micros(), a.subsec_nanos());

    println!("{:?} {:?} {:?}", a.checked_add(b), b.checked_sub(a), a.checked_sub(b));
    println!("{:?} {:?} {:?}", a.checked_mul(3), a.checked_div(4), a.checked_div(0));
    println!("{:?} {:?} {:?}", a.saturating_add(b), b.saturating_sub(a), b.saturating_mul(2));
    println!("{:?} {:?}", a.abs_diff(b), b.abs_diff(a));
    println!("{:?} {:?} {:?} {:?}", a.mul_f64(1.5), b.mul_f32(2.0), a.div_f64(2.0), b.div_f32(3.0));
    println!("{} {}", a.div_duration_f64(b), b.div_duration_f32(a));

    let mut total = Duration::ZERO;
    for ms in [100, 250, 650] {
        total += Duration::from_millis(ms);
    }
    let doubled = total * 2;
    let half = doubled / 4;
    println!("{:?} {:?} {:?} {:?}", total, doubled, half, doubled - total);
    println!("{} {} {:?}", a > b, b == Duration::from_micros(750_000), [a, b].iter().max());
}
