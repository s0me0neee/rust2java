use std::process::Command;

#[test]
fn unsupported_formatting_is_reported() {
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("unsupported.rs");
    std::fs::write(
        &source,
        r#"fn main() {
    let x = 1;
    println!("{:p}", &x);
    println!("{:e}", 1.0);
}"#,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_rust2java"))
        .arg(&source)
        .arg("-o")
        .arg(work.path().join("out"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("format spec `{:p}`"), "{stderr}");
    assert!(stderr.contains("format spec `{:e}`"), "{stderr}");
    // The incomplete file is still written for reference, but javac rejects it.
    let java = std::fs::read_to_string(work.path().join("out/unsupported.java")).unwrap();
    assert!(java.starts_with("#error rust2java: 2 error(s)"), "{java}");
    assert!(java.contains("UNSUPPORTED"), "{java}");
}
