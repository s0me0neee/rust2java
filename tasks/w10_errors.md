# Worker 10 — Option/Result methods and error handling

Read `tasks/COMMON.md` and `DESIGN.md` (§9.2, §9.3, §13) first.

**You own:** `src/lower/std_map/option.rs`, `src/support/Result.java`, and `pat.rs::lower_try`
(worker 2 is done; keep other parts of `pat.rs` untouched). Test prefix: `err_`. Probe: `tasks/probes/opt.rs`.

## Already done (w2/w4)

Most Option/Result combinators, `?` on Option/Result, `main` returning `Result`, if-let/match on them.

## Gaps found by the probe

- Option: `zip`, `xor`, `get_or_insert`/`get_or_insert_with` (mutates the place: `o = o.or(Optional.of(..))`
  then get), `insert`, `take`/`replace` on fields, `as_mut`/`as_deref`, `None::<T>` turbofish path,
  `is_none_or`, `inspect`, `unwrap_unchecked` (= unwrap), `iter()` on Option.
- Result: `unwrap_or_default`, `is_ok_and`/`is_err_and`, `inspect_err`, `ok_or_else`, `expect_err`,
  `iter`, `transpose`, `copied`/`cloned`, `flatten`.
- **Error types**: std parse errors print Rust's text — `ParseIntError` Display (`invalid digit found in string`,
  `cannot parse integer from empty string`, `number too large to fit in target type`), `ParseFloatError`
  (`invalid float literal`), and their Debug (`ParseIntError { kind: InvalidDigit }`). Pick one std-only
  representation (e.g. the message `String`) and make Display/Debug/`unwrap` panics match rustc.
- `?` with error conversion: `Err(e)?` where `From<E1> for E2` exists → `E2.from(e)` (worker 8 emits the
  `static from`; until it's merged, name such cases `err_needs_traits_*`).
- `Box<dyn Error>`: `"msg".into()`, `?` from any error into `Box<dyn Error>`, `e.to_string()`, printing it;
  `main() -> Result<(), Box<dyn Error>>` failure output `Error: <Debug>` and exit code 1.
- User error enums `impl std::error::Error for E {}` (no-op), `#[derive(Debug)]` error printed via `{:?}`.
- `unwrap()`/`expect()` panic messages must match Rust's
  (`called \`Result::unwrap()\` on an \`Err\` value: ...`, `called \`Option::unwrap()\` on a \`None\` value`) and exit 101.

## Done when

`err_*` cases cover each bullet and the opt probe passes; full suite passes.
