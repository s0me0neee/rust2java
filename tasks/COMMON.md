# Shared brief for all rust2java workers

You are one of several agents working **in parallel**, each in its own git worktree and branch,
on `rust2java`: a transpiler from Rust (1.95+, edition 2024) to readable **Java 21**.
`DESIGN.md` is the spec — read it fully before coding. Where this brief and DESIGN.md disagree,
this brief wins.

## Hard rules (from the design)

- **Same stdout + exit code** as the Rust program. Panics exit 101 (handled by the generated entry point).
- **Line numbers**: every JIR `Stmt` must carry the Rust line it came from (`cx.line(node)`).
  Extra Java needed for one Rust line goes on that same line (push several `Stmt`s with the same line).
- **Java standard library only.** No helper classes/functions. The only generated non-std code is
  `Result.java` (src/support/Result.java), `TupleN` records (`items::need_tuple`), the per-file `debug_str`/`debug_float`
  helpers (`fmt::file_helper`, for `{:?}` on str/char and floats) and the user's own types.
  If std can't express something in one line, hoist temporaries into `out` (same line) or use a stream.
- **Keep Rust names** (`snake_case` stays). Java keywords get `_` suffix via `escape_keyword`.
- **Never silently mistranslate.** If something isn't supported, call `cx.unsupported(node, "what")`.
- Types: `u8` → `int`; other unsigned → same-width signed; `usize` → `int`; `char` → `char`;
  `String`/`&str` → `String`; `Vec`/arrays/slices → `List<T>` (constructed as `ArrayList`);
  `Option` → `Optional`; `Result` → generated `Result`/`Ok`/`Err`; pairs → `Map.Entry` (`Map.entry(a, b)`,
  `getKey()/getValue()`); 3+-tuples → `TupleN` records with `_0(), _1(), ...`.

## Code map (read these files first)

| File | Purpose |
|---|---|
| `src/frontend.rs` | loads the cargo project via rust-analyzer (`ra_ap_*` =0.0.358); `convert()` maps RA types to `Ty` |
| `src/ty.rs` | `Ty` model + `java()` / `boxed()` type rendering |
| `src/jir.rs` | Java IR (`Expr`, `Stmt`, `StmtKind`, `Block`, `Method`, `Member`, `Class`) |
| `src/emit.rs` | line-aware printer (you rarely need to touch it) |
| `src/lower/mod.rs` | `Cx` context: `ty(e)`, `pat_ty(p)`, `type_ref(t)`, `line(node)`, `declare(name)`, `lookup(name)`, `fresh(base)`, `unsupported(node, what)`, `import(..)`, `needs_result`, `file_trailer`, `loops`, `switch_depth` |
| `src/lower/expr.rs` | value expressions. `lower_expr(cx, e, out)` — `out` collects hoisted statements that run before the current one |
| `src/lower/stmt.rs` | statements/blocks; `lower_to_dest(cx, e, &Dest, out)` lowers an expression in statement context (`Dest::{Discard, Return, Assign(var), Yield}`) |
| `src/lower/calls.rs` | fn/method/ctor call routing (local vs std) |
| `src/lower/items.rs` | items + the enum/struct representation contract (see comment block there) |
| `src/lower/pat.rs` | patterns, match, if-let, `?` |
| `src/lower/fmt.rs`, `macros.rs` | format strings and std macros |
| `src/lower/std_map/mod.rs` | std dispatch: `lower_std_fn_call` / `lower_std_method` / `lower_std_path` / `lower_std_ctor`, shared helpers (`wrap_ok`, `consumed_by`, `clone_expr`, `default_value`, `optional`, …) |
| `src/lower/std_map/{text,num,option,coll,io,mem,ptr}.rs` | one std area each; every file has a `fn_call` and a `method` **hook** that the dispatcher tries first |
| `src/lower/iter.rs` | `for` loops, ranges, iterator chains → streams |

rust-analyzer source for API lookup: `~/.cargo/registry/src/*/ra_ap_hir-0.0.358/src/` (`semantics.rs`, `lib.rs`),
`ra_ap_syntax-0.0.358/src/ast/generated/nodes.rs`. `Semantics` is `cx.sema`, the db is `cx.db`.

## Std hooks (how to add std support without touching shared code)

`std_map/mod.rs` calls every area's `fn_call(cx, c, key, ret, out)` before its own match and every
area's `method(cx, m, recv_ty, key, out)` before lowering the receiver. Add new std functions/methods
in **your area's hook** (return `None` without lowering anything when the key isn't yours). Keys come
from `calls::std_fn_key` (e.g. `Rc::new`, `str::split_once`, `RefCell::borrow`); print it when unsure.
Existing per-type functions (`lower_str_method`, `lower_vec_method`, …) live in the area files too —
extend those directly. Shared helpers are in `mod.rs` (reachable through `use super::*`).

## Probes

`tasks/probes/*.rs` are realistic programs exercising each area, with rustc's output in `.expected`.
Run one with `./target/debug/rust2java tasks/probes/str.rs -o /tmp/p --run`. Turn the lines your area
owns into real test cases under your prefix; leave other areas' lines out (or put them in a
`<prefix>_needs_<area>_*.rs` case).

## Workflow

- Build: `cargo build`. CLI: `./target/debug/rust2java file.rs -o /tmp/x --run` (prints Java errors / output).
- Tests: put new cases in `tests/cases/<yourprefix>_<topic>.rs` (each a complete program with `fn main`,
  deterministic output, no dependence on HashMap iteration order). Optional stdin: `NAME.stdin`.
  Run `UPDATE_EXPECTED=1 CASE=<yourprefix> cargo test --test cases` once to record rustc's output
  (`NAME.expected`, `NAME.exit`), then `CASE=<yourprefix> cargo test --test cases` while iterating.
  Commit the `.expected`/`.exit` files. Before finishing, run the **whole** suite (`cargo test --test cases`).
- Cases that depend on another workstream: name them `<prefix>_needs_<area>_<topic>.rs`; the harness
  skips them unless `NEEDS=1` is set.
- Inspect generated Java and check that statements stay on their Rust line numbers, and that it reads
  like hand-written Java (no needless temps, `String.valueOf` wrappers or casts).
- Run `cargo fmt` before every commit (the project is rustfmt-clean).

## Ownership (to keep merges clean)

You own the files listed in your task. Other `src/` files are shared: change them only when you must,
keep changes small and additive (new function, new match arm), never reformat or restructure them,
and list every shared-file change in your final report.

## Style (from the user's global instructions)

- Comments: one line max, explain *why*, not *what*. No `///` doc comments. No essay comments.
- No `*Manager`/`*Service`/`*Factory` types, no inheritance-style layering.
- Don't add crates beyond: clap, serde/serde_json, anyhow, thiserror, regex, itertools, walkdir,
  tempfile, rayon, which, duct (already have the ra_ap ones). Ask before anything else.
- Edit files with your edit tool, not `sed -i`/heredocs (except bulk mechanical changes).

## Finish

Commit your work on your branch (small, logical commits are fine). End with a short report:
what works (with case names), what's left unsupported, and every shared-file change you made.
