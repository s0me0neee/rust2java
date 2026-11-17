# Worker 12 — I/O, env, process, fs, time

Read `tasks/COMMON.md` and `DESIGN.md` (§12, §13) first.

**You own:** `src/lower/std_map/io.rs`. Test prefix: `io_`. Probe: `tasks/probes/io.rs` (+ `io.stdin`).
Shared files you'll likely touch (small, additive): `macros.rs::lower_write` (write targets),
`iter.rs` (stdin `lines()` source).

## Already done (w4)

`io::stdin().read_line`, `stdin().lines()`, `read_to_string` (partially), `fs::read_to_string`/`fs::write`,
`env::args`, `env::var`, `process::exit`, `Instant`/`Duration`, stdout `flush`.

## Gaps found by the probe

- `stdin.lock()` / `StdinLock::read_line` / `.lines()` on a lock, `BufRead` trait methods on
  `BufReader::new(io::stdin())`, `read_to_string` into an existing `String`, `bytes()` on stdin,
  reading all input then tokenising (competitive-programming style `split_ascii_whitespace` scanners).
- Output: `io::stdout().lock()`, `BufWriter::new(io::stdout())`, `write!`/`writeln!` to those
  (`write! target type` errors come from `macros.rs::lower_write`), `.unwrap()`/`?` on their results,
  `flush`, `io::stderr()` and `writeln!(io::stderr(), ..)`. A `BufWriter` must flush on exit
  (Rust flushes on drop) — map it to `System.out` (PrintStream) or a `PrintWriter` flushed in the entry point.
- `print!` without newline followed by stdin reads (ordering of buffered output).
- `fs::File::open` + `BufReader::new(f).lines()`, `File::create` + `write_all`, `fs::read_dir` (sorted) —
  one-line `try` per DESIGN §12, errors as `Result` with Rust's `io::Error` Display text where feasible.
- `env::args().nth(1)`, `env::args().skip(1)`, `env::var("X").unwrap_or(..)`.
- `std::process::exit` inside nested calls with pending `BufWriter` output.

## Done when

`io_*` cases (with `.stdin` files) cover each bullet and the io probe passes; full suite passes.
