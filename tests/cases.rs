// Differential tests: every tests/cases/NAME.rs must print the same stdout and exit with the same
// code under rustc and after transpiling to Java.
//   CASE=a,b         run only cases whose name contains a or b
//   UPDATE_EXPECTED=1 regenerate NAME.expected / NAME.exit from rustc

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Case {
    name: String,
    src: PathBuf,
    stdin: Option<PathBuf>,
}

fn cases() -> Vec<Case> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/cases");
    let filter = std::env::var("CASE").ok();
    let mut out: Vec<Case> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "rs"))
        .map(|src| {
            let name = src.file_stem().unwrap().to_string_lossy().to_string();
            let stdin = src.with_extension("stdin");
            Case {
                name,
                stdin: stdin.exists().then_some(stdin),
                src,
            }
        })
        .filter(|c| {
            filter
                .as_ref()
                .is_none_or(|f| f.split(',').any(|p| c.name.contains(p)))
        })
        // `*_needs_<area>` cases wait on another workstream; NEEDS=1 runs them too.
        .filter(|c| !c.name.contains("_needs_") || std::env::var("NEEDS").is_ok())
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

fn run_with_stdin(cmd: &mut Command, stdin: &Option<PathBuf>) -> (String, i32, String) {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(match stdin {
        Some(p) => Stdio::from(std::fs::File::open(p).unwrap()),
        None => Stdio::null(),
    });
    let out = cmd.output().expect("spawn");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn expected(case: &Case, work: &Path) -> (String, i32) {
    let exp = case.src.with_extension("expected");
    let exit = case.src.with_extension("exit");
    if exp.exists() && std::env::var("UPDATE_EXPECTED").is_err() {
        let code = std::fs::read_to_string(&exit)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        return (std::fs::read_to_string(&exp).unwrap(), code);
    }
    let bin = work.join(format!("rust_{}", case.name));
    let status = Command::new("rustc")
        .args(["--edition", "2024", "-O", "-o"])
        .arg(&bin)
        .arg(&case.src)
        .status()
        .expect("rustc");
    assert!(status.success(), "rustc failed for {}", case.name);
    let (stdout, code, _) = run_with_stdin(&mut Command::new(&bin), &case.stdin);
    std::fs::write(&exp, &stdout).unwrap();
    if code != 0 {
        std::fs::write(&exit, code.to_string()).unwrap();
    }
    (stdout, code)
}

#[test]
fn differential() {
    let cases = cases();
    if cases.is_empty() {
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let out_dir = work.path().join("out");
    // SIMPLE=1: transpile with `-s`. Every case must compile and exit like Rust (zero or not)
    // unless listed in tests/cases/simple-skip, and print like Rust unless listed in
    // tests/cases/simple-diffs (`{:?}` is `toString`, unsigned values print signed).
    let simple = std::env::var("SIMPLE").is_ok();
    let list = |file: &str| -> Vec<String> {
        if !simple {
            return Vec::new();
        }
        std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/cases")
                .join(file),
        )
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split('#').next())
        .flat_map(|l| l.split_whitespace().map(str::to_string).collect::<Vec<_>>())
        .collect()
    };
    let skip = list("simple-skip");
    let allowed_diffs = list("simple-diffs");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rust2java"));
    cmd.args(cases.iter().map(|c| &c.src))
        .arg("-o")
        .arg(&out_dir)
        .arg(if simple { "-s" } else { "--line-report" });
    let (_, _, diag) = run_with_stdin(&mut cmd, &None);

    let mut failures = Vec::new();
    let (mut stdout_diffs, mut stale_diffs) = (Vec::new(), Vec::new());
    let (mut total_stmts, mut total_drift, mut drift_notes) = (0u64, 0u64, Vec::new());
    for case in &cases {
        let (want, want_code) = expected(case, work.path());
        let case_diags: Vec<&str> = diag
            .lines()
            .filter(|l| l.starts_with("error") && l.contains(&format!("/{}.rs:", case.name)))
            .collect();
        if !case_diags.is_empty() {
            failures.push(format!(
                "{}: transpile errors:\n  {}",
                case.name,
                case_diags.join("\n  ")
            ));
            continue;
        }
        let classes = work.path().join("classes").join(&case.name);
        let java_file = out_dir.join(format!("{}.java", case.name));
        let sources: Vec<&PathBuf> = std::iter::once(&java_file).filter(|p| p.exists()).collect();
        if let Ok(text) = std::fs::read_to_string(out_dir.join(format!("{}.lines.json", case.name)))
        {
            let v: serde_json::Value = serde_json::from_str(&text).unwrap();
            let drift = v["drift"].as_array().map(|a| a.len()).unwrap_or(0) as u64;
            total_stmts += v["stmts"].as_u64().unwrap_or(0);
            total_drift += drift;
            if drift > 0 {
                drift_notes.push(format!(
                    "  {}: {} drifted {:?}",
                    case.name, drift, v["drift"]
                ));
            }
        }
        if sources.is_empty() {
            failures.push(format!("{}: no Java output\n{diag}", case.name));
            continue;
        }
        let (_, code, err) = run_with_stdin(
            Command::new("javac").arg("-d").arg(&classes).args(&sources),
            &None,
        );
        if code != 0 {
            failures.push(format!("{}: javac failed:\n{err}", case.name));
            continue;
        }
        let (got, got_code, err) = run_with_stdin(
            Command::new("java").arg("-cp").arg(&classes).arg("Main"),
            &case.stdin,
        );
        // Simple mode keeps whether the program fails, not the exit code or the panic text.
        let code_ok = got_code == want_code || simple && (got_code != 0) == (want_code != 0);
        if simple && code_ok && got != want && allowed_diffs.contains(&case.name) {
            stdout_diffs.push(case.name.clone());
            continue;
        }
        if simple && got == want && allowed_diffs.contains(&case.name) {
            stale_diffs.push(case.name.clone());
        }
        if (got != want || !code_ok) && !skip.contains(&case.name) {
            failures.push(format!(
                "{}: output mismatch (exit {got_code}, want {want_code})\n--- want\n{want}--- got\n{got}--- stderr\n{err}",
                case.name
            ));
        }
    }
    let passed = cases.len() - failures.len();
    eprintln!("\n{passed}/{} cases passed", cases.len());
    if simple {
        eprintln!(
            "simple mode: {} listed case(s) print differently from Rust",
            stdout_diffs.len()
        );
        if !stale_diffs.is_empty() {
            eprintln!(
                "simple mode: now print like Rust, remove from simple-diffs: {}",
                stale_diffs.join(" ")
            );
        }
    }
    let exact = if total_stmts == 0 {
        100.0
    } else {
        100.0 * (total_stmts - total_drift) as f64 / total_stmts as f64
    };
    eprintln!("line match: {exact:.1}% of {total_stmts} statements on their Rust line");
    for n in &drift_notes {
        eprintln!("{n}");
    }
    if !failures.is_empty() {
        panic!("\n{}\n", failures.join("\n\n"));
    }
}
