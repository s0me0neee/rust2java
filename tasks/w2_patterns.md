# Worker 2 — patterns: match, if-let/while-let/let-chains, let-else, `?`, Result

Read `tasks/COMMON.md` and `DESIGN.md` (§9, §9.1–9.3) first.

**You own:** `src/lower/pat.rs`, `src/support/Result.java`. Test prefix: `pat_`.

## Scope

1. **`match` as statement** (`lower_match_stmt`) and **as value** (`try_match_expr` → Java switch expression
   when no arm contains `return`/`break`/`continue`/`?`, otherwise statement form assigning a destination).
   Increment `cx.switch_depth` while lowering a Java switch *statement* body (stmt.rs uses it to label
   `break`s that must escape the switch). Arms with block bodies use `Dest::Yield` in switch expressions.
2. **Patterns** (§9.1): literals (ints, chars, strings, bools), `A | B`, ranges `1..=5`, wildcards, bindings,
   `x @ pat`, guards (`when`), tuples, refs, nested patterns, slices `[a, b, ..]` (on `List`), struct/variant
   patterns of **local enums/structs** using the representation contract in `items.rs`
   (`enum_is_plain`, `variant_java_name`, `java_field_names` — use them, don't redefine the representation).
   Record patterns where Java allows (`case Shape.Circle(var r) ->`); fall back to an `if`/`else if` chain
   on a temp for anything Java's switch can't express. Java 21: no unnamed `_` patterns — use fresh names.
   Use `default -> throw new IllegalStateException("unreachable")` only where Java needs it for exhaustiveness.
3. **`Option` patterns**: `Optional` is not a record → `isPresent()/get()` (§9.1 table), including in `match`.
4. **`Result`**: `Ok(v)/Err(e)` record patterns. Set `cx.needs_result = true` whenever Result is used.
   Extend `Result.java` only with methods that would otherwise need multi-line inline code.
5. **Conditions** (`lower_condition`): `if let`, `while let`, edition-2024 **let chains**
   (`if let Some(x) = a && x > 3 && let Ok(y) = f(x)`), with bindings scoped to the body (`binds`).
   Idiom: `while let Some(x) = v.pop()` → `while (!v.isEmpty()) { var x = v.removeLast();`.
6. **`let ... else`** and refutable destructuring.
7. **`?` operator** (§9.3): hoist into a switch statement on the same line, `return new Err<>(e)` (with
   `From` conversion when the error types differ: `ErrType.from(e)` — coordinate with worker 1's naming),
   Option `?` via `isEmpty()` check. Must work inside closures and loop conditions.
8. **`main` returning `Result`** — the entry point already handles `Err` (items.rs `entry_point`); make sure
   `Ok(())` tails and `?` in main work.
9. `matches!(e, P)` — worker 3 owns macros.rs; expose a `pub fn pattern_test(cx, pat, scrut, out) -> Expr`
   (boolean test without bindings) so worker 3 can call it, and tell them in your report.

## Done when

`pat_*` cases cover each bullet (use Option/Result/ints/strings/tuples; for local-enum patterns write the enum
declarations too — they'll fail until worker 1 lands, so mark such cases with a `_needs_items` suffix in the
file name and still commit them). Full suite passes otherwise.
