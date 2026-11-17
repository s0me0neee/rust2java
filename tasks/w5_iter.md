# Worker 5 — loops, iterators, closures → streams

Read `tasks/COMMON.md` and `DESIGN.md` (§9 loops, §11) first.

**You own:** `src/lower/iter.rs`. Test prefix: `iter_`.

## Scope

1. **`for` loops** (`lower_for`): ranges (`a..b`, `a..=b`, `.rev()`, `.step_by(k)`, non-pure bounds evaluated
   once), `for x in &v` / `v.iter()` / `v.iter_mut()` / `into_iter()`, `for (i, x) in v.iter().enumerate()`
   (index loop idiom), maps (`for (k, v) in &m` → entrySet with `getKey/getValue` binds on the same line),
   strings (`for c in s.chars()` → `for (char c : s.toCharArray())`), `s.bytes()`, `s.lines()`,
   `s.split(..)`, and any other iterator (stream `.iterator()` so `break`/`continue` keep working).
   Labeled loops already work via `cx.loops`.
   Mutating elements through `iter_mut()` for primitives needs index loops (`v.set(i, ...)`).
2. **Iterator chains** (`lower_iter_method` — called for every std method before `std_map`; return `None`
   for non-iterator methods): sources (`iter`, `into_iter`, `iter_mut`, `chars`, `bytes`, `char_indices`,
   `lines`, `split*`, `keys`, `values`, `drain(..)`, ranges), adaptors and consumers per the §11 table:
   `map filter filter_map flat_map flatten enumerate zip rev skip take step_by chain take_while skip_while
   peekable windows chunks inspect copied cloned` and `count sum product min max min_by_key max_by_key
   max_by min_by fold any all find position last nth for_each collect join`.
   Respect the documented semantic traps: `max*` returns the **last** max, float `sum` via `reduce(0.0, Double::sum)`,
   `collect` target from the inferred type (`Vec` → `toList()` only when never mutated, else
   `Collectors.toCollection(ArrayList::new)`; `String`; `HashMap`/`HashSet`/`BTreeMap`; `Result<Vec<_>,_>`
   via statements), `IntStream` boxing.
3. **Closures** are lowered by `expr::lower_closure` (shared, in expr.rs); fix it there if needed (report changes).
   Closure parameter patterns `|(a, b)|`, `|&x|`, `|&(a, b)|` must work in stream lambdas.
4. **Iterators held in variables**: `let mut it = v.iter(); it.next()` → `Iterator` + `Optional` (§11 table).
5. `impl Iterator for UserType` integration can wait for worker 1; skip if blocked.

## Done when

`iter_*` cases cover the table (including side-effect ordering inside `map`) and pass; full suite passes.
