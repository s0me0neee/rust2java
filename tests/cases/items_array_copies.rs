// Copy arrays: copies (let, by-value args, rows of a grid) stay independent when changed in place.
fn grids() {
    let b = [1, 2, 3];
    let mut a = b;
    a[0] = 9;
    let grid = [[0; 2]; 2];
    let mut g2 = grid;
    g2[1][1] = 5;
    println!("{:?} {:?} {:?} {:?}", a, b, grid, g2);
}

fn bump(mut a: [i32; 3]) -> i32 { a[0] += 100; a[0] }
fn fill_row(r: &mut [u8; 4]) { r.fill(7); }
fn main() {
    grids();
    let b = [1, 2, 3];
    println!("{} {:?}", bump(b), b);
    let mut rows = [[0u8; 4]; 2];
    let saved = rows;
    fill_row(&mut rows[1]);
    let first = rows[1];
    rows[1][0] = 1;
    let mut sorted = b;
    sorted.sort_by(|x, y| y.cmp(x));
    println!("{:?} {:?} {:?} {:?} {:?}", rows, saved, first, sorted, b);
    let ro = [5, 6];
    let copy = ro;
    println!("{:?} {:?}", ro, copy);
}
