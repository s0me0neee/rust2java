use std::ops::Add;

pub struct Triangle<T> {
    sides: [T; 3],
}

impl<T: Copy + PartialOrd + Add<Output = T> + Default> Triangle<T> {
    pub fn build(sides: [T; 3]) -> Option<Triangle<T>> {
        let [a, b, c] = sides;
        let zero = T::default();
        if a <= zero || b <= zero || c <= zero {
            return None;
        }
        if a + b < c || a + c < b || b + c < a {
            return None;
        }
        Some(Triangle { sides })
    }

    fn distinct(&self) -> usize {
        let [a, b, c] = self.sides;
        1 + usize::from(a != b) + usize::from(c != a && c != b)
    }

    pub fn is_equilateral(&self) -> bool {
        self.distinct() == 1
    }
    pub fn is_scalene(&self) -> bool {
        self.distinct() == 3
    }
    pub fn is_isosceles(&self) -> bool {
        self.distinct() <= 2
    }
}

fn describe<T: Copy + PartialOrd + Add<Output = T> + Default>(s: [T; 3]) -> String {
    match Triangle::build(s) {
        None => "invalid".to_string(),
        Some(t) => format!("eq={} iso={} sca={}", t.is_equilateral(), t.is_isosceles(), t.is_scalene()),
    }
}

fn main() {
    println!("{}", describe([2, 2, 2]));
    println!("{}", describe([3, 4, 4]));
    println!("{}", describe([5, 4, 6]));
    println!("{}", describe([1, 1, 3]));
    println!("{}", describe([0.5, 0.4, 0.6]));
    println!("{}", describe([0, 0, 0]));
}
