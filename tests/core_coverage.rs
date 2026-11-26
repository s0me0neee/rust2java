// Every item of the everyday core API (tests/core_api.txt, written by core_api.py) must be called
// by some test case, whose output tests/cases.rs checks against rustc. A dropped case or mapping
// shows up here as an uncovered item.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

// `i32::abs` and `u8::abs` are the same code path: integer widths fold into iN / uN, floats into fN.
fn normalize(key: &str) -> String {
    let (owner, name) = key.rsplit_once("::").unwrap_or(("", key));
    let owner = match owner {
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "iN",
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "uN",
        "f32" | "f64" => "fN",
        o => o,
    };
    format!("{owner}::{name}")
}

fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .collect();
    files.sort();
    files
}

#[test]
fn core_api_covered() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let work = tempfile::tempdir().unwrap();
    let calls = work.path().join("calls.txt");
    let mut inputs = rust_files(&root.join("tests/cases"));
    inputs.extend(rust_files(&root.join("tests/simple")));
    // One process for every file; the exit code is the cases' business, not this test's.
    Command::new(env!("CARGO_BIN_EXE_rust2java"))
        .env("R2J_STD_CALLS", &calls)
        .arg("-q")
        .arg("-o")
        .arg(work.path().join("out"))
        .args(&inputs)
        .output()
        .expect("spawn rust2java");
    let called: BTreeSet<String> = std::fs::read_to_string(&calls)
        .unwrap_or_default()
        .lines()
        .map(normalize)
        .collect();
    let api = std::fs::read_to_string(root.join("tests/core_api.txt")).unwrap();
    let missing: Vec<&str> = api.lines().filter(|k| !k.is_empty() && !called.contains(*k)).collect();
    let total = api.lines().filter(|k| !k.is_empty()).count();
    assert!(
        missing.is_empty(),
        "{} of {total} core API items are called by no test case:\n  {}",
        missing.len(),
        missing.join("\n  ")
    );
}
