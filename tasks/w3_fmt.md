# Worker 3 — formatting & std macros

Read `tasks/COMMON.md` and `DESIGN.md` (§10, §10.1, §15) first.

**You own:** `src/lower/fmt.rs`, `src/lower/macros.rs`. Test prefix: `fmt_`.

## Already done (by the lead; extend, don't redo)

- `vec![a, b]` / `vec![x; n]` (macros.rs, via `std_map::lower_array_expr` / `lower_repeat`).
- `{:?}` of `Option` and `Vec`/`VecDeque` (stream joining), floats, strings, chars, user types (`.debug()`).

## Scope

1. **Format spec** (§10): width, fill, alignment (`<`, `>`, `^`), sign `+`, `#`, `0`-padding, precision,
   `x X o b e` (`e` may stay unsupported), `{:>width$}`, `{:.prec$}`, `{:.*}`, positional `{0}`, named
   `name = expr`, inline captures `{name}`. Rust left-aligns strings and right-aligns numbers by default.
   Prefer `String.format` / `printf`-free concatenation; centering needs hoisted temps (same line).
   Known accepted gap: `%.Nf` rounding ties half-up.
2. **Display** (`fmt::display`): floats via `BigDecimal(...).stripTrailingZeros().toPlainString()` (already),
   `char` concatenation pitfalls (`c1 + c2` adds ints in Java — make sure concatenations start from a String),
   user types rely on `toString()` (worker 1 generates it from `impl Display`).
3. **Debug** (`fmt::debug`, §10.1): strings/chars quoted (escape `"` `\` `\n` like Rust's `escape_debug`),
   floats (`1.0`, `1e16` style thresholds per Rust: Debug uses exponent when exp < -4 or ≥ 16), `Vec`/slices,
   `HashMap`/`BTreeMap` (`{k: v}`), sets, `Optional` (`Some(..)`/`None`), pairs/tuples, `Result` (`Ok(..)`),
   nested combinations via stream `map(...).collect(Collectors.joining(", ", "[", "]"))`; user types → `x.debug()`.
   `{:#?}`: implement for std containers + primitives if feasible, else unsupported.
4. **Macros** (`macros.rs`): `print!`, `println!`, `eprint!`, `eprintln!`, `format!`, `panic!` (with/without
   args), `unreachable!`, `todo!`, `unimplemented!`, `assert!`, `assert_eq!`, `assert_ne!` (Rust's message
   text: ``assertion `left == right` failed`` + left/right lines), `debug_assert*!` (no-ops: release semantics),
   `vec![a, b]`, `vec![x; n]` (fresh copy per element when `x` is mutable), `write!`/`writeln!` into a
   `StringBuilder`/`String` target (`f.append(...)`) — used by worker 1's `impl Display` → `toString()`,
   `dbg!` (stderr only; value passthrough), `matches!` (call worker 2's `pat::pattern_test` if it exists by
   the time you merge; otherwise implement simple literal/enum cases and leave a note).
5. Macros in expression position vs statement position (`stmt::lower_to_dest` treats diverging macros as `throw`).

## Done when

`fmt_*` cases cover the spec matrix (ints/floats/strings/chars/bools/vectors/maps/options × specs) and pass;
full suite passes.
