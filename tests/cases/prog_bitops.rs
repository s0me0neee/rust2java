// Bit manipulation: unsigned widths, rotates, popcount, leading/trailing zeros, byte conversions,
// radix parsing/printing, a bitset struct, and gray codes.
struct BitSet {
    words: Vec<u64>,
}

impl BitSet {
    fn new(n: usize) -> Self {
        BitSet { words: vec![0; (n + 63) / 64] }
    }
    fn set(&mut self, i: usize) {
        self.words[i / 64] |= 1 << (i % 64);
    }
    fn clear(&mut self, i: usize) {
        self.words[i / 64] &= !(1u64 << (i % 64));
    }
    fn get(&self, i: usize) -> bool {
        self.words[i / 64] >> (i % 64) & 1 == 1
    }
    fn count(&self) -> u32 {
        self.words.iter().map(|w| w.count_ones()).sum()
    }
    fn iter(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.words.len() * 64).filter(move |&i| self.get(i))
    }
}

fn gray(n: u32) -> u32 {
    n ^ (n >> 1)
}

fn reverse_bits8(mut b: u8) -> u8 {
    let mut r = 0u8;
    for _ in 0..8 {
        r = (r << 1) | (b & 1);
        b >>= 1;
    }
    r
}

fn main() {
    let x: u32 = 0b1011_0000_0000_0000_0000_0000_0000_0110;
    println!("{} {} {} {}", x.count_ones(), x.leading_zeros(), x.trailing_zeros(), x.count_zeros());
    println!("{:#x} {:#b}", x.rotate_left(4), x.rotate_right(8) as u8);
    println!("{:08b} {:08b} {}", 0b1100_1010u8, reverse_bits8(0b1100_1010), 0b1100_1010u8.reverse_bits());
    let y: u16 = 0xFFF0;
    println!("{} {} {}", y.wrapping_add(0x20), y.checked_add(0x20).is_none(), y.saturating_add(0x20));
    println!("{} {}", (y as i16), (-1i8) as u8);
    let bytes = 0x12345678u32.to_be_bytes();
    let le = 0x12345678u32.to_le_bytes();
    println!("{:?} {:?} {:#x}", bytes, le, u32::from_be_bytes([0xde, 0xad, 0xbe, 0xef]));
    println!("{:?} {:?}", u8::from_str_radix("ff", 16), i32::from_str_radix("-101", 2));
    println!("{:?}", u8::from_str_radix("1ff", 16).is_err());
    println!("{}", (0..8).map(|i| format!("{:03b}", gray(i))).collect::<Vec<_>>().join(" "));
    println!("{} {}", u64::MAX.count_ones(), (1u64 << 40).trailing_zeros());
    println!("{} {}", i32::MIN.wrapping_abs(), (-17i32).rem_euclid(5));
    println!("{} {}", 37u32.is_power_of_two(), 37u32.next_power_of_two());
    let mut bs = BitSet::new(130);
    for i in [0, 5, 64, 65, 129, 5] {
        bs.set(i);
    }
    bs.clear(65);
    println!("{} {:?} {}", bs.count(), bs.iter().collect::<Vec<_>>(), bs.get(64));
    let mask = (1u32 << 5) - 1;
    let packed = (3u32 << 10) | (17 << 5) | 9;
    println!("{} {} {}", packed >> 10, (packed >> 5) & mask, packed & mask);
    let h = "hello".bytes().fold(0xcbf29ce484222325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
    println!("{:016x}", h);
    println!("{}", i64::MAX.to_string().len() + (u8::MAX as usize));
}
