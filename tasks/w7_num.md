# Worker 7 — numbers, conversions, Ordering, mem

Read `tasks/COMMON.md` and `DESIGN.md` (§6, §6.1) first.

**You own:** `src/lower/std_map/num.rs`, `src/lower/std_map/mem.rs`. Test prefix: `num_`.
Probe: `tasks/probes/num.rs`.

## Already done (w4, extend, don't redo)

abs/pow/min/max/rem_euclid, checked_/saturating_/wrapping_*, count_ones, float math, `as` casts,
`from_str_radix`, int consts, `mem::swap/replace/take`, `Ordering::is_*`/`reverse`.

## Gaps found by the probe

- `clamp` (int and float), `abs_diff`, `is_power_of_two`, `div_euclid`, `signum`, `hypot`,
  `leading_zeros`/`trailing_zeros` for every width, `to_le_bytes`-style only if cheap.
- Numeric `From`/`TryFrom` between std types: `i64::from(x)`, `f64::from(x)`, `u8::try_from(x)`
  (→ `Result`, error text `out of range integral type conversion attempted`), `x.into()` numeric widening,
  `usize::try_from(-1)`.
- `Ordering`: `then`, `then_with`, `cmp` on floats via `total_cmp`, `partial_cmp` → `Optional<Integer>`,
  **Debug of `Ordering`** (`Less`/`Equal`/`Greater`) and of `Option<Ordering>` — add a small `Ty::Ordering`
  arm in `fmt.rs::debug` (shared file, additive). Ordering values must be normalised to -1/0/1 wherever
  they're printed or compared with `==` (Java `compareTo` may return any sign magnitude).
- `mem::size_of::<T>()` (Rust sizes as constants for primitives; unsupported otherwise).
- Integer overflow semantics in release mode wrap (`-O`): check `+ - *` on `i32`/`i64` already wrap in Java;
  `u32`/`u64` represented signed — printing `u32` above `i32::MAX` must use `Integer.toUnsignedString`
  (check `fmt::display` for unsigned types; coordinate by adding the arm in `fmt.rs`). Same for `/`, `%`,
  `>>`, comparisons on `u32`/`u64` (`Integer.divideUnsigned`, `compareUnsigned`, `>>>`).
- Float Display corner cases: `1e-7` (Rust prints `0.0000001`), `1e21`, `-0.0`, `f64::MAX`, `NaN`, `inf`.

## Done when

`num_*` cases cover each bullet and the num probe passes; full suite passes.
