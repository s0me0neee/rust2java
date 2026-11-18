use std::io::{self, BufWriter, Write};

fn finish() {
    std::process::exit(7);
}

fn nested() {
    let mut output = BufWriter::with_capacity(1, io::stdout());
    write!(output, "before nested exit").unwrap();
    output.flush().unwrap();
    finish();
}

fn main() {
    nested();
    println!("unreachable");
}
