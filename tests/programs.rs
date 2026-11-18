// Differential tests for multi-file cargo projects: each tests/programs/NAME/ is a cargo
// project; its stdout/exit code must match after transpiling the whole project to Java.
//   PROG=substr      run only matching projects
//   UPDATE_EXPECTED=1 regenerate NAME.expected / NAME.exit from cargo run

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Program {
    name: String,
    dir: PathBuf,
    stdin: Option<PathBuf>,
}

fn programs() -> Vec<Program> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/programs");
    let filter = std::env::var("PROG").ok();
    let mut out: Vec<Program> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || !path.join("Cargo.toml").exists() {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if filter.as_ref().is_some_and(|f| !name.contains(f.as_str())) {
            continue;
        }
        let stdin = dir.join(format!("{name}.stdin"));
        out.push(Program {
            name,
            dir: path,
            stdin: stdin.exists().then_some(stdin),
        });
    }
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

fn expected(program: &Program, work: &Path) -> (String, i32) {
    let base = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/programs")
        .join(&program.name);
    let exp = base.with_extension("expected");
    let exit = base.with_extension("exit");
    if exp.exists() && std::env::var("UPDATE_EXPECTED").is_err() {
        let code = std::fs::read_to_string(&exit)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0);
        return (std::fs::read_to_string(&exp).unwrap(), code);
    }
    let target = work.join("cargo-target");
    let (stdout, code, stderr) = run_with_stdin(
        Command::new("cargo")
            .arg("run")
            .arg("--quiet")
            .arg("--manifest-path")
            .arg(program.dir.join("Cargo.toml"))
            .current_dir(&program.dir)
            .env("CARGO_TARGET_DIR", &target),
        &program.stdin,
    );
    assert_eq!(code, 0, "cargo run failed for {}:\n{stderr}", program.name);
    std::fs::write(&exp, &stdout).unwrap();
    if code != 0 {
        std::fs::write(&exit, code.to_string()).unwrap();
    }
    (stdout, code)
}

#[test]
fn differential() {
    let programs = programs();
    if programs.is_empty() {
        return;
    }
    let work = tempfile::tempdir().unwrap();
    let out_dir = work.path().join("out");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_rust2java"));
    cmd.args(programs.iter().map(|p| &p.dir))
        .arg("-o")
        .arg(&out_dir);
    let (_, _, diag) = run_with_stdin(&mut cmd, &None);

    let mut failures = Vec::new();
    for program in &programs {
        let (want, want_code) = expected(program, work.path());
        let case_diags: Vec<&str> = diag
            .lines()
            .filter(|l| l.contains(&format!("/{}/src/", program.name)))
            .collect();
        if !case_diags.is_empty() {
            failures.push(format!(
                "{}: transpile errors:\n  {}",
                program.name,
                case_diags.join("\n  ")
            ));
            continue;
        }
        let pkg_dir = out_dir.join("src").join(&program.name);
        let classes = work.path().join("classes").join(&program.name);
        let files: Vec<PathBuf> = std::fs::read_dir(&pkg_dir)
            .map(|rd| rd.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        let sources: Vec<&PathBuf> = files
            .iter()
            .filter(|p| p.extension().is_some_and(|e| e == "java"))
            .collect();
        if sources.is_empty() {
            failures.push(format!("{}: no Java output\n{diag}", program.name));
            continue;
        }
        let (_, code, err) = run_with_stdin(
            Command::new("javac").arg("-d").arg(&classes).args(&sources),
            &None,
        );
        if code != 0 {
            failures.push(format!("{}: javac failed:\n{err}", program.name));
            continue;
        }
        let (got, got_code, err) = run_with_stdin(
            Command::new("java")
                .arg("-cp")
                .arg(&classes)
                .arg(format!("{}.Main", program.name)),
            &program.stdin,
        );
        if got != want || got_code != want_code {
            failures.push(format!(
                "{}: output mismatch (exit {got_code}, want {want_code})\n--- want\n{want}--- got\n{got}--- stderr\n{err}",
                program.name
            ));
        }
    }
    eprintln!(
        "\n{}/{} projects passed",
        programs.len() - failures.len(),
        programs.len()
    );
    if !failures.is_empty() {
        panic!("\n{}\n", failures.join("\n\n"));
    }
}
