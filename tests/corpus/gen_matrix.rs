use std::ops::{Add, Index, IndexMut, Mul};

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
        let mut m = Self::zeros(n, n);
        for i in 0..n {
            m[(i, i)] = 1.0;
        }
        m
    }
    fn from_rows(rows: &[&[f64]]) -> Self {
        let mut m = Self::zeros(rows.len(), rows[0].len());
        for (r, row) in rows.iter().enumerate() {
            for (c, &v) in row.iter().enumerate() {
                m[(r, c)] = v;
            }
        }
        m
    }
    fn transpose(&self) -> Self {
        let mut t = Self::zeros(self.cols, self.rows);
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

impl Mul for &Matrix {
    type Output = Matrix;
    fn mul(self, rhs: &Matrix) -> Matrix {
        assert_eq!(self.cols, rhs.rows);
        let mut out = Matrix::zeros(self.rows, rhs.cols);
        for i in 0..self.rows {
            for j in 0..rhs.cols {
                out[(i, j)] = (0..self.cols).map(|k| self[(i, k)] * rhs[(k, j)]).sum();
            }
        }
        out
    }
}

impl Add for Matrix {
    type Output = Matrix;
    fn add(mut self, rhs: Matrix) -> Matrix {
        for (a, b) in self.data.iter_mut().zip(rhs.data.iter()) {
            *a += b;
        }
        self
    }
}

fn main() {
    let a = Matrix::from_rows(&[&[1.0, 2.0, 3.0], &[4.0, 5.0, 6.0]]);
    let b = a.transpose();
    let p = &a * &b;
    println!("{:?}", p.data);
    println!("{}", &p * &Matrix::identity(2) == p);
    let s = p.clone() + Matrix::identity(2);
    for r in 0..s.rows {
        let row: Vec<String> = (0..s.cols).map(|c| format!("{:6.1}", s[(r, c)])).collect();
        println!("{}", row.join(""));
    }
}
