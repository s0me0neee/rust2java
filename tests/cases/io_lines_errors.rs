// Byte-level line splitting (lone \r kept, \r\n stripped): the byte read keeps stdin out of line mode.
use std::io::{self, BufRead, Read};

fn main() {
    for line in io::stdin().lock().lines().flatten() {
        println!("{:?}", line);
    }
    let _ = io::stdin().bytes().count();
}
