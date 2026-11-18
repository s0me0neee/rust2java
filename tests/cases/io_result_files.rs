use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, Write};

fn open_file(path: &str) -> io::Result<File> {
    File::open(path)
}

fn main() -> io::Result<()> {
    let path = "/tmp/rust2java_w12_result.txt";
    let mut file = File::create(path)?;
    writeln!(file, "result {}", 4)?;
    file.write_all(b"tail\n")?;
    file.flush()?;
    let input = open_file(path)?;
    for line in BufReader::new(input).lines() {
        println!("{}", line?);
    }
    let entries = fs::read_dir("/tmp")?;
    let mut found = false;
    for entry in entries {
        if entry?.file_name().to_string_lossy() == "rust2java_w12_result.txt" {
            found = true;
        }
    }
    println!("found={}", found);
    Ok(())
}
