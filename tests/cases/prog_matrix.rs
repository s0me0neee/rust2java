// A small dense matrix type with operator-trait impls and a Display table.
use std::fmt;
use std::ops::{Add, Index, IndexMut, Mul, Neg, Sub};

#[derive(Debug, Clone, PartialEq)]
struct Matrix {
    rows: usize,
    cols: usize,
    data: Vec<f64>,
}

impl Matrix {
    fn new(rows: usize, cols: usize, data: Vec<f64>) -> Matrix {
        assert_eq!(rows * cols, data.len(), "bad matrix shape");
        Matrix { rows, cols, data }
    }

    fn zeros(rows: usize, cols: usize) -> Matrix {
        Matrix {
            rows,
            cols,
            data: vec![0.0; rows * cols],
        }
    }

    fn identity(n: usize) -> Matrix {
        let mut m = Matrix::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }

    fn get(&self, r: usize, c: usize) -> f64 {
        self.data[r * self.cols + c]
    }

    fn set(&mut self, r: usize, c: usize, value: f64) {
        self.data[r * self.cols + c] = value;
    }

    fn transpose(&self) -> Matrix {
        let mut out = Matrix::zeros(self.cols, self.rows);
        for r in 0..self.rows {
            for c in 0..self.cols {
                out.set(c, r, self.get(r, c));
            }
        }
        out
    }

    fn trace(&self) -> f64 {
        (0..self.rows.min(self.cols)).map(|i| self.get(i, i)).sum()
    }

    fn max_abs(&self) -> f64 {
        self.data.iter().map(|x| x.abs()).fold(0.0, |a: f64, b: f64| a.max(b))
    }

    fn scale(&mut self, factor: f64) {
        for value in &mut self.data {
            *value *= factor;
        }
    }

    fn round_to(&self, places: i32) -> Matrix {
        let factor = 10f64.powi(places);
        Matrix {
            rows: self.rows,
            cols: self.cols,
            data: self.data.iter().map(|x| (x * factor).round() / factor).collect(),
        }
    }
}

impl Index<(usize, usize)> for Matrix {
    type Output = f64;

    fn index(&self, at: (usize, usize)) -> &f64 {
        &self.data[at.0 * self.cols + at.1]
    }
}

impl IndexMut<(usize, usize)> for Matrix {
    fn index_mut(&mut self, at: (usize, usize)) -> &mut f64 {
        &mut self.data[at.0 * self.cols + at.1]
    }
}

impl Add for Matrix {
    type Output = Matrix;

    fn add(self, other: Matrix) -> Matrix {
        assert_eq!(self.rows, other.rows);
        assert_eq!(self.cols, other.cols);
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a + b)
            .collect();
        Matrix::new(self.rows, self.cols, data)
    }
}

impl Sub for Matrix {
    type Output = Matrix;

    fn sub(self, other: Matrix) -> Matrix {
        let data = self
            .data
            .iter()
            .zip(other.data.iter())
            .map(|(a, b)| a - b)
            .collect();
        Matrix::new(self.rows, self.cols, data)
    }
}

impl Neg for Matrix {
    type Output = Matrix;

    fn neg(self) -> Matrix {
        Matrix::new(self.rows, self.cols, self.data.iter().map(|x| -x).collect())
    }
}

impl Mul<f64> for Matrix {
    type Output = Matrix;

    fn mul(self, factor: f64) -> Matrix {
        Matrix::new(self.rows, self.cols, self.data.iter().map(|x| x * factor).collect())
    }
}

impl Mul for Matrix {
    type Output = Matrix;

    fn mul(self, other: Matrix) -> Matrix {
        assert_eq!(self.cols, other.rows, "shape mismatch");
        let mut out = Matrix::zeros(self.rows, other.cols);
        for r in 0..self.rows {
            for c in 0..other.cols {
                let mut sum = 0.0;
                for k in 0..self.cols {
                    sum += self.get(r, k) * other.get(k, c);
                }
                out.set(r, c, sum);
            }
        }
        out
    }
}

impl fmt::Display for Matrix {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for r in 0..self.rows {
            let mut line = String::new();
            for c in 0..self.cols {
                line.push_str(&format!("{:>8.3}", self.get(r, c)));
            }
            writeln!(f, "{}", line)?;
        }
        Ok(())
    }
}

fn determinant(m: &Matrix) -> f64 {
    if m.rows == 1 {
        return m.get(0, 0);
    }
    if m.rows == 2 {
        return m.get(0, 0) * m.get(1, 1) - m.get(0, 1) * m.get(1, 0);
    }
    let mut det = 0.0;
    for c in 0..m.cols {
        let mut minor = Matrix::zeros(m.rows - 1, m.cols - 1);
        for r in 1..m.rows {
            let mut dst = 0;
            for k in 0..m.cols {
                if k != c {
                    minor.set(r - 1, dst, m.get(r, k));
                    dst += 1;
                }
            }
        }
        let sign = if c % 2 == 0 { 1.0 } else { -1.0 };
        det += sign * m.get(0, c) * determinant(&minor);
    }
    det
}

fn main() {
    let a = Matrix::new(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    let b = Matrix::new(2, 3, vec![6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
    println!("a + b =\n{}", a.clone() + b.clone());
    println!("a - b =\n{}", a.clone() - b.clone());
    println!("-a =\n{}", -a.clone());
    println!("2.5 * a =\n{}", a.clone() * 2.5);
    println!("a^T =\n{}", a.transpose());
    println!("trace = {}", Matrix::identity(3).trace());
    let p = Matrix::new(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
    let q = Matrix::new(2, 2, vec![0.0, 1.0, 1.0, 0.0]);
    println!("p * q =\n{}", p.clone() * q.clone());
    println!("det(p) = {}", determinant(&p));
    println!("det(p*q) = {}", determinant(&(p.clone() * q)));
    let mut m = Matrix::identity(3);
    m[(0, 1)] = 4.0;
    m[(2, 2)] += 2.0;
    println!("m =\n{}", m.round_to(2));
    println!("m[0,1] = {}, m[2,2] = {}", m[(0, 1)], m[(2, 2)]);
    let mut scaled = m.clone();
    scaled.scale(0.5);
    println!("scaled max abs = {:.2}", scaled.max_abs());
    let transposed = scaled.transpose();
    println!("rows/cols = {}x{} -> {}x{}", scaled.rows, scaled.cols, transposed.rows, transposed.cols);
    println!("equal to itself: {}", m == m.clone());
    let mut total = 0.0;
    for r in 0..a.rows {
        for c in 0..a.cols {
            total += a.get(r, c) * (c + 1) as f64;
        }
    }
    println!("weighted total = {}", total);
}
