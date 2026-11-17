# Worker 9 — collections: Vec/slices, maps, sets, VecDeque, BinaryHeap

Read `tasks/COMMON.md` and `DESIGN.md` (§6, §11, §11.1, §8) first.

**You own:** `src/lower/std_map/coll.rs`. Test prefix: `coll_`. Probes: `tasks/probes/vec.rs`, `tasks/probes/map.rs`.

## Already done (w4/w5, extend, don't redo)

Most Vec/HashMap/BTreeMap/HashSet/BTreeSet/VecDeque/BinaryHeap methods, the entry API, `Reverse`
heaps, array literals and `vec!`, `&v[a..b]`, iterator chains (`iter.rs`).

## Gaps found by the probes

- `&mut` element access: `get_mut`, `last_mut`, `first_mut`, `iter_mut` over `Vec<Vec<_>>` rows,
  `if let Some(x) = v.get_mut(0) { *x += 1 }` ("deref of a `&mut` primitive/String (cell)"): an element
  alias like `iter.rs` uses for `iter_mut` (`v.set(i, …)`) — see `Cx::elem_aliases`.
- `split_first`/`split_last`/`split_at` (→ tuple of sublists), `to_vec`, `concat` of `Vec<Vec<T>>`,
  `chunks`/`windows` results used as `Vec`, `binary_search` (→ `Result<usize, usize>`), `rotate_left`,
  `fill`, `swap` on nested indices, `sort_by` with `Ordering::then` chains (needs worker 7's `then`;
  otherwise mark the case `_needs_num_`), `sort_unstable_by_key`, `dedup_by_key`, `retain_mut`.
- Maps: `retain`, `first_key_value`/`last_key_value`, `pop_first`/`pop_last`, `range(a..b)` on BTreeMap,
  `keys().last()` / `iter().next_back()` on BTree collections, `get_or_insert_with`, `extend` from pairs,
  `values_mut`, `iter_mut` over map values, `HashMap<(i32, i32), _>` tuple keys (Map.entry keys hash fine).
- Sets: `first`/`last`/`pop_first` on BTreeSet, `difference`/`symmetric_difference`/`is_subset`,
  `retain`, `HashSet::from([..])`, set equality.
- `VecDeque` indexing/`get`/`iter().rev()`/`make_contiguous`, `BinaryHeap::into_sorted_vec`/`into_vec`.
- Deterministic Debug output: tests must not depend on HashMap/HashSet iteration order.

## Done when

`coll_*` cases cover each bullet and both probes pass; full suite passes.
