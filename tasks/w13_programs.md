# Worker 13 — whole programs (phase 2: launch after w6–w12 are merged)

Read `tasks/COMMON.md` and `DESIGN.md` (§1, §4, §16, §17) first.

**You own:** `tests/cases/prog_*` and `tests/programs/` (multi-file cargo projects). Test prefix: `prog_`.
Any `src/` change is shared: keep it minimal and list it.

## Scope

Write 12–15 realistic "light Rust project" programs (60–250 lines each, typical idioms, no external
crates) and make them transpile, compile and match rustc. Suggested set:

word frequency counter (stdin) · todo list with an enum command parser · matrix ops with operator traits ·
bank account simulation with `Result` errors · tokenizer + recursive-descent calculator (`Box<Expr>`) ·
BFS/DFS on a grid (`VecDeque`, `HashSet<(i32,i32)>`) · Dijkstra (`BinaryHeap<Reverse<..>>`) ·
inventory with `BTreeMap` + `Display` tables (`{:<10}{:>8.2}`) · text adventure state machine ·
linked list / binary tree with `Option<Box<Node>>` · shape trait objects (`Vec<Box<dyn Shape>>`) ·
generic `Stack<T>` / `Matrix<T>` with trait bounds · CSV-ish parsing with `split`/`parse`/`?` ·
game of life · a 2–3 module cargo project (`mod`, `pub fn`, `use crate::`).

For each failure: fix it if it's a small gap in shared code, otherwise reduce it to a minimal case named
`prog_needs_<area>_<topic>.rs` and list it in your report (area = text/num/traits/coll/err/ptr/io/pat/fmt/iter/items).
Also report readability problems in the generated Java (giant expressions, needless temps, drift).

## Done when

All `prog_*` cases pass (minus documented `_needs_` ones), line match stays ≥ 99%, full suite passes.
