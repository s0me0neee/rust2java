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
#[command(name = "rust2java", about = "Transpile Rust (1.95+) to readable Java 21")]
struct Args {
    // A cargo project directory, or one or more .rs files.
    #[arg(required = true)]
    inputs: Vec<PathBuf>,
    #[arg(short, long, default_value = "out")]
    out: PathBuf,
    // Compile with javac and run each transpiled program.
    #[arg(long)]
    run: bool,
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
    let mut packages: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut errors = 0;
    for unit in project.units() {
        let lowered = lower::lower_unit(&sema, &unit);
        for d in &lowered.diags {
            eprintln!("error: {}:{}:{}: {}", unit.path.display(), d.line, d.col, d.msg);
        }
        errors += lowered.diags.len();
        let (java, report) = emit::print_file(&lowered.file, lowered.comments, &lowered.trailer, lowered.last_line);
        let dir = args.out.join("src").join(&unit.package);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{}.java", unit.class_name)), java)?;
        let lines = serde_json::json!({ "rust": unit.path, "stmts": report.stmts, "drift": report.drift });
        std::fs::write(dir.join(format!("{}.lines.json", unit.class_name)), lines.to_string())?;
        if lowered.needs_result {
            std::fs::write(dir.join("Result.java"), RESULT_JAVA.replace("PACKAGE", &unit.package))?;
        }
        packages.insert(unit.package.clone(), dir);
    }
    if errors > 0 {
        eprintln!("{errors} error(s)");
        return Ok(ExitCode::from(1));
    }
    if args.run {
        for (package, dir) in &packages {
            let code = compile_and_run(&args.out, package, dir)?;
            if code != 0 {
                return Ok(ExitCode::from(code as u8));
            }
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn compile_and_run(out: &Path, package: &str, dir: &Path) -> Result<i32> {
    let classes = out.join("classes");
    let sources: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "java"))
        .collect();
    let javac = which::which("javac").context("javac not found on PATH")?;
    let status = Command::new(javac).arg("-d").arg(&classes).args(&sources).status()?;
    if !status.success() {
        anyhow::bail!("javac failed for package {package}");
    }
    let java = which::which("java").context("java not found on PATH")?;
    let status = Command::new(java).arg("-cp").arg(&classes).arg(format!("{package}.Main")).status()?;
    Ok(status.code().unwrap_or(1))
}
