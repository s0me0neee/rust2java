use std::fs::{self, File};
use std::io::{self, BufWriter, Write};

fn main() {
    let bytes = b"AB";
    println!("{} {}", bytes.len(), bytes[0]);
    let mut out = io::stdout();
    out.write_all(b"\xc3\xa9\n").unwrap();
    let bytes = vec![240u8, 159, 166, 128, 10];
    let n = out.write(&bytes).unwrap();
    println!("wrote={}", n);
    println!("empty={}", out.write(b"").unwrap());
    let unit = out.write_all(b"unit\n").unwrap();
    let _ = unit;
    let path = "/tmp/rust2java_w12_format.txt";
    {
        let mut file = BufWriter::new(File::create(path).unwrap());
        writeln!(file, "file {}", 7).unwrap();
    }
    print!("{}", fs::read_to_string(path).unwrap());
    fs::write(path, "replacement\n").unwrap();
    print!("{}", fs::read_to_string(path).unwrap());
    fs::write(path, b"\xc3\xa9\n").unwrap();
    print!("{}", fs::read_to_string(path).unwrap());
    let _ = File::open("/rust2java-w12-no-such-file/input");
    let _ = fs::read_to_string("/rust2java-w12-no-such-file/input");
    let _ = fs::write("/rust2java-w12-no-such-file/output", "ignored");
    let missing = "/rust2java-w12-no-such-file/input";
    let missing_dir = "/rust2java-w12-no-such-file";
    println!("missing={}", File::open(missing).is_err());
    println!("missing-dir={}", fs::read_dir(missing_dir).is_err());
}
