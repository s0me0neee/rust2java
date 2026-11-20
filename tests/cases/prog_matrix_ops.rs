// Operator overloading on a Matrix: Add/Sub/Mul (matrix and scalar), Neg, Index<(usize, usize)>,
// IndexMut, AddAssign, PartialEq, Display with alignment, and a generic Vector2<T>.
use std::fmt;
use std::ops::{Add, AddAssign, Index, IndexMut, Mul, Neg, Sub};

#[derive(Debug, Clone, PartialEq)]
struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    fn zeros(rows: usize, cols: usize) -> Self {
        Matrix { rows, cols, data: vec![0.0; rows * cols] }
    }
    fn identity(n: usize) -> Self {
        let mut m = Matrix::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }
    fn from_rows(rows: &[&[f64]]) -> Self {
        let cols = rows[0].len();
        Matrix { rows: rows.len(), cols, data: rows.iter().flat_map(|r| r.iter().copied()).collect() }
    }
    fn transpose(&self) -> Matrix {
        let mut t = Matrix::zeros(self.cols, self.rows);
        for r in 0..self.rows {
            for c in 0..self.cols {
                t[(c, r)] = self[(r, c)];
            }
        }
        t
    }
}

impl Index<(usize, usize)> for Matrix {
    type Output = f64;
    fn index(&self, (r, c): (usize, usize)) -> &f64 {
        &self.data[r * self.cols + c]
    }
}

impl IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, (r, c): (usize, usize)) -> &mut f64 {
        &mut self.data[r * self.cols + c]
    }
}

impl Add for &Matrix {
    type Output = Matrix;
    fn add(self, o: &Matrix) -> Matrix {
        Matrix { rows: self.rows, cols: self.cols, data: self.data.iter().zip(&o.data).map(|(a, b)| a + b).collect() }
    }
}

impl Sub for Matrix {
    type Output = Matrix;
    fn sub(self, o: Matrix) -> Matrix {
        &self + &(-o)
    }
}

impl Neg for Matrix {
    type Output = Matrix;
    fn neg(self) -> Matrix {
        self * -1.0
    }
}

impl Mul<f64> for Matrix {
    type Output = Matrix;
    fn mul(mut self, k: f64) -> Matrix {
        for x in self.data.iter_mut() {
            *x *= k;
        }
        self
    }
}

impl Mul for &Matrix {
    type Output = Matrix;
    fn mul(self, o: &Matrix) -> Matrix {
        let mut m = Matrix::zeros(self.rows, o.cols);
        for i in 0..self.rows {
            for j in 0..o.cols {
                m[(i, j)] = (0..self.cols).map(|k| self[(i, k)] * o[(k, j)]).sum();
            }
        }
        m
    }
}

impl AddAssign<f64> for Matrix {
    fn add_assign(&mut self, k: f64) {
        self.data.iter_mut().for_each(|x| *x += k);
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for r in 0..self.rows {
            let row: Vec<String> = (0..self.cols).map(|c| format!("{:6.2}", self[(r, c)])).collect();
            writeln!(f, "[{}]", row.join(" "))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct V2 {
    x: i64,
    y: i64,
}

impl Add for V2 {
    type Output = V2;
    fn add(self, o: V2) -> V2 {
        V2 { x: self.x + o.x, y: self.y + o.y }
    }
}

fn main() {
    let a = Matrix::from_rows(&[&[1.0, 2.0], &[3.0, 4.0]]);
    let b = Matrix::identity(2);
    print!("{}", &a + &b);
    print!("{}", &a * &a);
    print!("{}", a.clone() - b.clone());
    let mut c = a.transpose() * 0.5;
    c += 1.0;
    c[(0, 1)] = -c[(1, 0)];
    print!("{}", c);
    println!("{} {}", &a * &b == a, a == b);
    let r = Matrix::from_rows(&[&[1.0, 2.0, 3.0]]);
    let prod = &r.transpose() * &r;
    println!("{}x{} trace {}", prod.rows, prod.cols, (0..3).map(|i| prod[(i, i)]).sum::<f64>());
    let v = V2 { x: 1, y: 2 } + V2 { x: 10, y: 20 };
    let w = V2 { x: -5, y: 15 } + v;
    println!("{:?} {:?} {}", v, w, v == V2 { x: 11, y: 22 });
}
