// `Vec::resize` as grow/shrink loops.
fn move_zeros(arr: &[u8]) -> Vec<u8> {
    let mut out: Vec<u8> = arr.iter().copied().filter(|&e| e != 0).collect();
    out.resize(arr.len(), 0);
    out
}

fn main() {
    println!("{:?}", move_zeros(&[1, 2, 0, 1, 0, 1, 0, 3, 0, 1]));
}
