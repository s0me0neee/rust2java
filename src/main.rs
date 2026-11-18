mod emit;
mod frontend;
mod jir;
mod lower;
mod ty;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result};
use clap::Parser;
use ra_ap_hir::Semantics;

const RESULT_JAVA: &str = include_str!("support/Result.java");

#[derive(Parser)]
#[command(
    name = "rust2java",
    about = "Transpile Rust (1.95+) to readable Java 21"
)]
struct Args {
    // A cargo project directory, or one or more .rs files.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    #[arg(short, long, default_value = "out")]
    out: PathBuf,
    // Compile with javac and run each transpiled program.
    #[arg(long)]
    run: bool,
    // Also write `<name>.lines.json` (statement line-match report, used by the tests).
    #[arg(long, hide = true)]
    line_report: bool,
}

// Where a program's Java went: one file, or a package directory for multi-module crates.
enum Output {
    File(PathBuf),
    Package(String, PathBuf),
}

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> Result<ExitCode> {
    let args = Args::parse();
    let project = frontend::load(&args.inputs)?;
    ra_ap_hir::attach_db(&project.db, || transpile(&args, &project))
}

fn transpile(args: &Args, project: &frontend::Project) -> Result<ExitCode> {
    let sema = Semantics::new(&project.db);
    let mut outputs: BTreeMap<String, Output> = BTreeMap::new();
    let mut errors = 0;
    let units = project.units();
    for unit in &units {
        // A crate that is one source file becomes one Java file with everything in it.
        let single = units.iter().filter(|u| u.package == unit.package).count() == 1;
        let mut lowered = lower::lower_unit(&sema, unit);
        for d in &lowered.diags {
            eprintln!(
                "error: {}:{}:{}: {}",
                unit.path.display(),
                d.line,
                d.col,
                d.msg
            );
        }
        errors += lowered.diags.len();
        if single {
            lowered.file.package.clear();
            lowered.file.class.header = lowered
                .file
                .class
                .header
                .trim_start_matches("public ")
                .to_string();
        }
        let (mut java, report) = emit::print_file(
            &lowered.file,
            lowered.comments,
            &lowered.trailer,
            lowered.last_line,
        );
        let (dir, stem) = if single {
            (args.out.clone(), unit.package.clone())
        } else {
            (
                args.out.join("src").join(&unit.package),
                unit.class_name.clone(),
            )
        };
        std::fs::create_dir_all(&dir)?;
        if lowered.needs_result {
            if single {
                java.push_str(&embedded_result());
            } else {
                std::fs::write(
                    dir.join("Result.java"),
                    RESULT_JAVA.replace("PACKAGE", &unit.package),
                )?;
            }
        }
        let java_path = dir.join(format!("{stem}.java"));
        std::fs::write(&java_path, java)?;
        eprintln!("wrote {}", java_path.display());
        if args.line_report {
            let lines = serde_json::json!({ "rust": unit.path, "stmts": report.stmts, "drift": report.drift });
            std::fs::write(dir.join(format!("{stem}.lines.json")), lines.to_string())?;
        }
        let output = if single {
            Output::File(java_path)
        } else {
            Output::Package(unit.package.clone(), dir)
        };
        outputs.insert(unit.package.clone(), output);
    }
    if errors > 0 {
        eprintln!("{errors} error(s)");
        return Ok(ExitCode::from(1));
    }
    if args.run {
        for output in outputs.values() {
            let code = compile_and_run(&args.out, output)?;
            if code != 0 {
                return Ok(ExitCode::from(code as u8));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

// `Result`/`Ok`/`Err` as package-private types after the program's class.
fn embedded_result() -> String {
    let body: Vec<&str> = RESULT_JAVA
        .lines()
        .skip_while(|l| l.starts_with("package") || l.starts_with("import") || l.is_empty())
        .collect();
    format!(
        "\n{}\n",
        body.join("\n")
            .replacen("public sealed interface", "sealed interface", 1)
    )
}

fn compile_and_run(out: &Path, output: &Output) -> Result<i32> {
    let (sources, classes, main): (Vec<PathBuf>, PathBuf, String) = match output {
        Output::File(path) => {
            let stem = path.file_stem().unwrap_or_default().to_string_lossy();
            (
                vec![path.clone()],
                out.join("classes").join(&*stem),
                "Main".into(),
            )
        }
        Output::Package(package, dir) => {
            let sources = std::fs::read_dir(dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "java"))
                .collect();
            (sources, out.join("classes"), format!("{package}.Main"))
        }
    };
    let javac = which::which("javac").context("javac not found on PATH")?;
    let status = Command::new(javac)
        .arg("-d")
        .arg(&classes)
        .args(&sources)
        .status()?;
    if !status.success() {
        anyhow::bail!("javac failed for {}", sources[0].display());
    }
    let java = which::which("java").context("java not found on PATH")?;
    let status = Command::new(java)
        .arg("-cp")
        .arg(&classes)
        .arg(main)
        .status()?;
    Ok(status.code().unwrap_or(1))
}
