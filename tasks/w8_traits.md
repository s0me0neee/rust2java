# Worker 8 — std traits on user types

Read `tasks/COMMON.md` and `DESIGN.md` (§7, §7.2, §7.3, §8) first.

**You own:** `src/lower/items.rs` (worker 1 is done) and any new `src/lower/traits.rs`. Test prefix: `traits_`.
Probe: `tasks/probes/traits.rs`.

## Already done (w1)

Structs/enums/impls/traits/generics, derives (Debug, Clone, Copy, PartialEq, Eq, Hash, Default,
PartialOrd/Ord on structs and plain enums), Display → `toString`, operator traits (`Add`, `Sub`, `Mul`,
`Neg`, `AddAssign` → methods `add`/`sub`/…).

## Scope (the lead turned the first two into explicit `unsupported` errors; they used to mistranslate)

1. **`From`/`Into`/`TryFrom`/`FromStr` for user types** (DESIGN §7.3): `impl From<A> for B` → `static B from(A value)`
   on B (overload by parameter type); `B::from(a)`, `a.into()` (target from the expected type), and
   `"..".parse::<B>()` via `FromStr` → `B.from_str(s)` returning `Result`. The `into()` guard is in
   `std_map/mod.rs` (generic fallback) — replace it with the real call. Worker 10 uses
   `B.from(e)` for `?` error conversion, so keep exactly this convention.
2. **`Index`/`IndexMut`** for user types: `g[i]` → `g.index(i)`, `g[i] = v` / `g[i] += v` → `g.index_mut_set(...)`
   or a clearer convention you document in DESIGN §7.3. The guard is in `expr.rs::lower_index`.
3. **Manual** `impl PartialEq` / `Eq` / `Hash` / `PartialOrd` / `Ord` / `Clone` / `Default` / `Debug`:
   `equals`/`hashCode`/`compareTo` (+ `implements Comparable<T>`) so `sort()`, `HashMap` keys, `max()`, `==`, `<` work.
4. **`Display` for data enums** (sealed interface): Java interfaces can't default `toString`, so emit a
   `display()` default method and `public String toString() { return display(); }` in each record.
   `derive(PartialOrd, Ord)` on data enums (variant order, then fields).
5. **`impl Iterator for T`** (DESIGN §7.3): `next()` returning `Optional<Item>` + a `stream()` so `for x in t`,
   `t.take(3).collect()`, `t.sum()` work — coordinate with `iter.rs` (add a source arm there).
6. Other ops: `Rem`, `Div`, `BitAnd/BitOr/BitXor`, `Shl/Shr`, `*Assign` variants, `Mul<f64> for V2` and
   `Mul<V2> for f64` (the latter is unsupported unless trivial), `Not`, `PartialEq` between different types.
7. `Deref` for newtypes (`impl Deref for Wrapper { type Target = Vec<T> }` → method calls forward) if feasible,
   otherwise a clear `unsupported`.

## Done when

`traits_*` cases cover each item and the traits probe passes; full suite passes.
