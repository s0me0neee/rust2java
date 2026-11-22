mod emit;
mod frontend;
mod jir;
mod lower;
mod ty;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use clap::Parser;
use ra_ap_hir::Semantics;

const RESULT_JAVA: &str = include_str!("support/Result.java");

#[derive(Parser)]
#[command(
    name = "rust2java",
    version,
    about = "Transpile Rust (1.95+) to readable Java 21",
    after_help = "Examples:\n  rust2java main.rs                 writes ./main.java\n  rust2java main.rs -o Solution.java\n  rust2java main.rs -o - | less\n  rust2java main.rs --run < input.txt\n  rust2java main.rs --run -- arg1 arg2\n  rust2java path/to/crate -o out    one directory per package for multi-module crates"
)]
struct Args {
    #[arg(
        required = true,
        value_name = "INPUT",
        help = "A cargo project directory, or one or more .rs files"
    )]
    inputs: Vec<PathBuf>,
    #[arg(
        short,
        long,
        default_value = ".",
        value_name = "PATH",
        help = "Output directory, a .java file (one program only), or - for stdout"
    )]
    out: PathBuf,
    #[arg(
        long,
        help = "Compile with javac and run each program; exits with the program's exit code"
    )]
    run: bool,
    #[arg(short, long, help = "Don't print the paths of written files")]
    quiet: bool,
    #[arg(
        last = true,
        requires = "run",
        value_name = "ARGS",
        help = "Arguments for the program run by --run"
    )]
    program_args: Vec<String>,
    // Also write `<name>.lines.json` (statement line-match report, used by the tests).
    #[arg(long, hide = true)]
    line_report: bool,
}

// What `-o` names.
enum Target {
    Dir(PathBuf),
    File(PathBuf),
    Stdout,
}

impl Target {
    fn parse(out: &Path) -> Target {
        if out == Path::new("-") {
            Target::Stdout
        } else if out.extension().is_some_and(|e| e == "java") {
            Target::File(out.to_path_buf())
        } else {
            Target::Dir(out.to_path_buf())
        }
    }
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
    let target = Target::parse(&args.out);
    // Checked before loading, which takes seconds.
    if matches!(target, Target::File(_)) && args.inputs.len() > 1 {
        bail!(
            "-o {}: a .java file holds one program; give a directory for several inputs",
            args.out.display()
        );
    }
    if args.line_report && matches!(target, Target::Stdout) {
        bail!("--line-report needs an output directory or file");
    }
    let project = frontend::load(&args.inputs)?;
    ra_ap_hir::attach_db(&project.db, || transpile(&args, &target, &project))
}

fn transpile(args: &Args, target: &Target, project: &frontend::Project) -> Result<ExitCode> {
    let sema = Semantics::new(&project.db);
    let mut outputs: BTreeMap<String, Output> = BTreeMap::new();
    let mut errors = 0;
    let units = project.units();
    // A crate that is one source file becomes one Java file with everything in it.
    let single =
        |unit: &frontend::Unit| units.iter().filter(|u| u.package == unit.package).count() == 1;
    let out_dir = match target {
        Target::Dir(dir) => dir,
        _ if !units.iter().all(single) => bail!(
            "-o {}: a multi-module crate needs an output directory",
            args.out.display()
        ),
        Target::File(_) if units.len() > 1 => bail!(
            "-o {}: a .java file holds one program; give a directory for several binaries",
            args.out.display()
        ),
        _ => Path::new("."),
    };
    // `-o - --run` still needs files for javac.
    let scratch = match target {
        Target::Stdout if args.run => Some(tempfile::tempdir()?),
        _ => None,
    };
    for unit in &units {
        let single = single(unit);
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
        for d in &lowered.warnings {
            eprintln!(
                "warning: {}:{}:{}: {}",
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
        if lowered.needs_result && single {
            java.push_str(&embedded_result());
        }
        // Prefixed on line 1 so the incomplete file can't compile, and line numbers still match.
        if !lowered.diags.is_empty() {
            let n = lowered.diags.len();
            java.insert_str(
                0,
                &format!("#error rust2java: {n} error(s), this file is incomplete; "),
            );
        }
        let java_path = match target {
            Target::File(path) => path.clone(),
            _ if single => out_dir.join(format!("{}.java", unit.package)),
            _ => out_dir
                .join("src")
                .join(&unit.package)
                .join(format!("{}.java", unit.class_name)),
        };
        if matches!(target, Target::Stdout) {
            print!("{java}");
        }
        let java_path = match &scratch {
            Some(tmp) => tmp.path().join(java_path),
            None if matches!(target, Target::Stdout) => continue,
            None => java_path,
        };
        let dir = java_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_default();
        std::fs::create_dir_all(&dir)?;
        if lowered.needs_result && !single {
            std::fs::write(
                dir.join("Result.java"),
                RESULT_JAVA.replace("PACKAGE", &unit.package),
            )?;
        }
        std::fs::write(&java_path, java)?;
        if !args.quiet && scratch.is_none() {
            let shown = java_path.strip_prefix(".").unwrap_or(&java_path);
            eprintln!("wrote {}", shown.display());
        }
        if args.line_report {
            let stem = java_path.file_stem().unwrap_or_default().to_string_lossy();
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
            let code = compile_and_run(output, &args.program_args)?;
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

// Class files go to a temporary directory so `--run` leaves only the .java behind.
fn compile_and_run(output: &Output, program_args: &[String]) -> Result<i32> {
    let tmp = tempfile::tempdir()?;
    let classes = tmp.path().to_path_buf();
    let (sources, main): (Vec<PathBuf>, String) = match output {
        Output::File(path) => (vec![path.clone()], "Main".into()),
        Output::Package(package, dir) => {
            let sources = std::fs::read_dir(dir)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|e| e == "java"))
                .collect();
            (sources, format!("{package}.Main"))
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
        .args(program_args)
        .status()?;
    Ok(status.code().unwrap_or(1))
}
