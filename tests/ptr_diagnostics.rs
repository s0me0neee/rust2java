use std::process::Command;

#[test]
fn unsupported_pointer_operations_are_reported() {
    let work = tempfile::tempdir().unwrap();
    let source = work.path().join("unsupported.rs");
    std::fs::write(
        &source,
        r#"use std::cell::{Cell, RefCell};
use std::rc::Rc;
fn main() {
    let a = Rc::new(1);
    let b = Rc::new(1);
    println!("{}", Rc::ptr_eq(&a, &b));
    println!("{}", Rc::strong_count(&a));
    let c = Cell::new(1);
    println!("{c:?}");
    let shared = Rc::new(RefCell::new(vec![1]));
    shared.replace(vec![2]);
    *shared.borrow_mut() = vec![3];
    let f = |a: i32, b: i32, c: i32| a + b + c;
    println!("{}", f(1, 2, 3));
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
    for expected in [
        "`Rc::ptr_eq` on erased immutable values",
        "`Rc::strong_count`/`weak_count`",
        "Debug/Display of a `Cell`/`RefCell` value",
        "replacing a shared object-backed cell",
        "replacing a shared object through `borrow_mut`",
        "functional values with more than two arguments",
    ] {
        assert!(stderr.contains(expected), "missing {expected}: {stderr}");
    }
}
