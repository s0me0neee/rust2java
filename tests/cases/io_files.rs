use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};

fn main() {
    let mut first = File::create("/tmp/rust2java_w12_alpha.txt").unwrap();
    first.write_all(b"one\ntwo\nlast").unwrap();
    first.flush().unwrap();
    let mut second = File::create("/tmp/rust2java_w12_beta.txt").unwrap();
    second.write_all(b"other\n").unwrap();
    second.flush().unwrap();
    let file = File::open("/tmp/rust2java_w12_alpha.txt").unwrap();
    for line in BufReader::new(file).lines() {
        println!("line={}", line.unwrap());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir("/tmp").unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        if name == "rust2java_w12_alpha.txt" || name == "rust2java_w12_beta.txt" {
            names.push(name);
        }
    }
    names.sort();
    println!("files={:?}", names);
}
