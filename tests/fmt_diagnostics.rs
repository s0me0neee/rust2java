use std::process::Command;

#[test]
fn unsupported_formatting_and_patterns_are_reported() {
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("unsupported.rs");
    std::fs::write(
        &source,
        r#"fn main() {
    println!("{:#?}", vec![1]);
    println!("{:e}", 1.0);
    println!("{}", matches!(2, 1..=3));
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
    assert!(stderr.contains("pretty Debug format"), "{stderr}");
    assert!(stderr.contains("format spec `{:e}`"), "{stderr}");
    assert!(stderr.contains("matches! pattern"), "{stderr}");
}
