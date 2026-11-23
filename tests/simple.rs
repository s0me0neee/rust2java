// Golden files for simple mode (DESIGN §22): tests/simple/NAME.rs must transpile with `-s` to
// exactly tests/simple/NAME.java. UPDATE_GOLDEN=1 rewrites the .java files.

use std::path::Path;
use std::process::Command;

#[test]
fn golden() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/simple");
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).unwrap() {
        let src = entry.unwrap().path();
        if src.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let out = Command::new(env!("CARGO_BIN_EXE_rust2java"))
            .args(["-s", "-q", "-o", "-"])
            .arg(&src)
            .output()
            .unwrap();
        let got = String::from_utf8_lossy(&out.stdout).into_owned();
        let golden = src.with_extension("java");
        if std::env::var("UPDATE_GOLDEN").is_ok() {
            std::fs::write(&golden, &got).unwrap();
            continue;
        }
        let want = std::fs::read_to_string(&golden).unwrap_or_default();
        if got != want {
            failures.push(format!(
                "{}:\n--- want\n{want}--- got\n{got}--- stderr\n{}",
                src.display(),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
