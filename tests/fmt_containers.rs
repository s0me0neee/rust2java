use std::process::Command;

#[test]
fn nested_map_and_set_debug_matches_rust() {
    let work = tempfile::tempdir().unwrap();
    let functions = r#"use std::collections::{BTreeMap, BTreeSet, HashMap};
fn render(map: HashMap<&str, Vec<Option<i32>>>, ordered: BTreeMap<i32, &str>, set: BTreeSet<i32>) {
    println!("{:04?}|{:>5?}|{:?}", map, ordered, set);
}
"#;
    let source = work.path().join("fmt_java_probe.rs");
    std::fs::write(&source, format!("{functions}fn main() {{}}\n")).unwrap();
    let reference = work.path().join("reference.rs");
    std::fs::write(
        &reference,
        format!(
            r#"{functions}
fn main() {{
    let mut map = HashMap::new();
    map.insert("k", vec![Some(7), None]);
    let mut ordered = BTreeMap::new();
    ordered.insert(3, "v");
    let mut set = BTreeSet::new();
    set.insert(2);
    set.insert(3);
    render(map, ordered, set);
}}
"#
        ),
    )
    .unwrap();
    let binary = work.path().join("reference");
    let rustc = Command::new("rustc")
        .args(["--edition", "2024"])
        .arg(&reference)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        rustc.status.success(),
        "{}",
        String::from_utf8_lossy(&rustc.stderr)
    );
    let expected = Command::new(binary).output().unwrap();

    let output_dir = work.path().join("out");
    let transpile = Command::new(env!("CARGO_BIN_EXE_rust2java"))
        .arg(&source)
        .arg("-o")
        .arg(&output_dir)
        .output()
        .unwrap();
    assert!(
        transpile.status.success(),
        "{}",
        String::from_utf8_lossy(&transpile.stderr)
    );
    let probe = work.path().join("Probe.java");
    std::fs::write(
        &probe,
        r#"package fmt_java_probe;
import java.util.*;
public class Probe {
    public static void main(String[] args) {
        HashMap<String, List<Optional<Integer>>> map = new HashMap<>();
        map.put("k", new ArrayList<>(List.of(Optional.of(7), Optional.empty())));
        TreeMap<Integer, String> ordered = new TreeMap<>();
        ordered.put(3, "v");
        TreeSet<Integer> set = new TreeSet<>(List.of(2, 3));
        Main.render(map, ordered, set);
    }
}
"#,
    )
    .unwrap();
    let classes = work.path().join("classes");
    let java = output_dir.join("src/fmt_java_probe/Main.java");
    let javac = Command::new("javac")
        .arg("-d")
        .arg(&classes)
        .arg(java)
        .arg(probe)
        .output()
        .unwrap();
    assert!(
        javac.status.success(),
        "{}",
        String::from_utf8_lossy(&javac.stderr)
    );
    let result = Command::new("java")
        .arg("-cp")
        .arg(classes)
        .arg("fmt_java_probe.Probe")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(result.stdout, expected.stdout);
}
