// Operator traits: Add/Sub/Mul<f64>/Div/Neg, *Assign, BitAnd/Or/Xor, Shl/Shr, Not, Rem.
use std::ops::{Add, AddAssign, BitAnd, BitOr, BitXor, Div, Mul, Neg, Not, Rem, Shl, Shr, Sub, SubAssign};

#[derive(Debug, Clone, Copy, PartialEq)]
struct V2 {
    x: f64,
    y: f64,
}
impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 { V2 { x: self.x + o.x, y: self.y + o.y } }
}
impl Sub for V2 {
    type Output = V2;
    fn sub(self, o: V2) -> V2 { V2 { x: self.x - o.x, y: self.y - o.y } }
}
impl Mul<f64> for V2 {
    type Output = V2;
    fn mul(self, k: f64) -> V2 { V2 { x: self.x * k, y: self.y * k } }
}
impl Div<f64> for V2 {
    type Output = V2;
    fn div(self, k: f64) -> V2 { V2 { x: self.x / k, y: self.y / k } }
}
impl Neg for V2 {
    type Output = V2;
    fn neg(self) -> V2 { V2 { x: -self.x, y: -self.y } }
}
impl AddAssign for V2 {
    fn add_assign(&mut self, o: V2) { self.x += o.x; self.y += o.y; }
}
impl SubAssign for V2 {
    fn sub_assign(&mut self, o: V2) { self.x -= o.x; self.y -= o.y; }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Bits(u32);
impl BitAnd for Bits {
    type Output = Bits;
    fn bitand(self, o: Bits) -> Bits { Bits(self.0 & o.0) }
}
impl BitOr for Bits {
    type Output = Bits;
    fn bitor(self, o: Bits) -> Bits { Bits(self.0 | o.0) }
}
impl BitXor for Bits {
    type Output = Bits;
    fn bitxor(self, o: Bits) -> Bits { Bits(self.0 ^ o.0) }
}
impl Not for Bits {
    type Output = Bits;
    fn not(self) -> Bits { Bits(!self.0 & 0xff) }
}
impl Shl<u32> for Bits {
    type Output = Bits;
    fn shl(self, n: u32) -> Bits { Bits((self.0 << n) & 0xff) }
}
impl Shr<u32> for Bits {
    type Output = Bits;
    fn shr(self, n: u32) -> Bits { Bits(self.0 >> n) }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Q(i32);
impl Rem for Q {
    type Output = Q;
    fn rem(self, o: Q) -> Q { Q(self.0 % o.0) }
}
impl Div for Q {
    type Output = Q;
    fn div(self, o: Q) -> Q { Q(self.0 / o.0) }
}

fn main() {
    let a = V2 { x: 1.0, y: 2.0 };
    let b = V2 { x: 0.5, y: 0.25 };
    println!("{:?}", a + b);
    println!("{:?}", a - b);
    println!("{:?}", a * 2.0);
    println!("{:?}", (a + b) / 2.0);
    println!("{:?}", -a);
    let mut c = a;
    c += b;
    println!("{:?}", c);
    c -= b;
    println!("{:?}", c);

    let x = Bits(0b1100);
    let y = Bits(0b1010);
    println!("{:?} {:?} {:?}", x & y, x | y, x ^ y);
    println!("{:?} {:?}", !x, x << 2);
    println!("{:?} {:?}", x >> 2, x == y);

    println!("{:?} {:?}", Q(7) % Q(3), Q(7) / Q(2));
}
