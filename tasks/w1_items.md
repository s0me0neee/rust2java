# Worker 1 — items: structs, enums, impls, traits, derives, generics, modules

Read `tasks/COMMON.md` and `DESIGN.md` (§4.3, §7, §8) first.

**You own:** `src/lower/items.rs`. Test prefix: `items_`.

## Scope

1. **Structs** → `static final class S { fields...; canonical ctor }` nested in the module class (DESIGN §7.1).
   Struct literals `S { b: 2, a: 1 }` (`items::lower_record_expr`) → `new S(1, 2)`, hoisting into temps
   when reordering would change side-effect order; `..base` update syntax. Tuple structs (`_0`, `_1`),
   unit structs. Field access already works via `expr.rs` (`p.x`, `t._0`).
2. **Enums** per the contract comment in `items.rs`: plain enums → Java `enum`; data enums → `sealed interface`
   + nested `record`s. Explicit discriminants + `as i32`. Constructors: `lower_ctor_call` (tuple variants,
   tuple structs) and `lower_def_path` (unit variants, unit structs, fn items as method refs).
   If a variant payload is mutated in place anywhere, use `final class` instead of `record` (§7.1).
3. **Impl blocks / class spans** (§4.3): a type's Java body spans from its `struct`/`enum` line to the closing
   brace of its last `impl` in the same file; impl headers and the struct's closing brace become blank lines.
   Methods: `&self`/`&mut self`/`self` → instance methods, associated fns → `static`. `Self` → class name.
   Unrelated items inside a span move after it (accept the drift). Inherent impls in other files: report unsupported for now.
4. **Traits** → `interface` (+ `default` methods, associated consts). `impl Trait for LocalType` adds
   `implements Trait` to the class header. `impl Trait for i32/String/Vec<T>` → static methods in a
   `<Trait>Impls` nested class, called statically (`lower_foreign_impl_call`). Trait static calls
   `T::new()` in generic code → unsupported. `dyn Trait`/`impl Trait` → the interface type.
5. **Display impl** → `public String toString()` with `f` as a `StringBuilder`; `write!(f, ...)` inside it
   becomes `f.append(...)` — coordinate: `macros.rs` is owned by worker 3, so add a small hook there only if
   needed and report it (worker 3 may also implement `write!` generically as `f.append(...)` which you can rely on).
6. **Derives** on the closing-brace line (§7.2): `Debug` → `public String debug()` in Rust's format
   (`Point { x: 1, y: 2 }`, `Circle(1.0)`, `Empty`; strings quoted, nested values via their debug form);
   `Clone` → `clone()`; `PartialEq/Eq/Hash` → `equals/hashCode` (records have them already);
   `PartialOrd/Ord` → `implements Comparable<S>` (fields in order; enums by variant index first);
   `Default` → `static S default_()`; `Copy` → copy-on-implicit-copy only for types mutated in place.
7. **Operator traits** (`impl Add for V`) → methods named `add`, `sub`, `neg`, `add_assign`… (expr.rs
   already calls `a.add(b)` for ADT operands).
8. **Generics** on structs/enums/fns/impls: `<T>` with bounds mapped per §7.4 (`Ord/PartialOrd` →
   `extends Comparable<T>`); `items::generic_params` is the starting point.
9. **Modules**: inline `mod x { ... }` → nested `static final class X`; `pub` → `public`. Multi-file crates
   already produce one Java file per module (check cross-module references work: `Geometry.area(...)`).
10. Nested `fn` items inside function bodies (`lower_local_item`) → private static methods of the class
    (place them on their own line if possible; otherwise report drift).

## Done when

`items_*` cases cover each bullet above and pass; the full suite still passes.
