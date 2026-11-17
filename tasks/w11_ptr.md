# Worker 11 — Box/Rc/RefCell/Cell and closures as values

Read `tasks/COMMON.md` and `DESIGN.md` (§6 table rows for Box/Rc/RefCell/Cell, §8) first.

**You own:** `src/lower/std_map/ptr.rs`. Test prefix: `ptr_`. Probe: `tasks/probes/ptr.rs`.
Shared files you'll likely touch (small, additive): `frontend.rs::convert` (type mapping),
`expr.rs::lower_closure`, `lower/mod.rs` (cells).

## Representation (DESIGN §6, §8)

- `Box<T>`, `Rc<T>`, `Arc<T>` → `T`. `Rc::clone(&x)`/`x.clone()` on an `Rc` → the same reference
  (**not** a deep clone — check `clone_expr` isn't applied to `Rc`).
- `RefCell<T>`, `Cell<T>` → `T` when `T` is already a mutable Java object (collections, user structs);
  `borrow()`/`borrow_mut()` disappear, including through `Ref`/`RefMut` receivers
  (`Other("RefMut<..>")` in the probe — map those types to their target in `convert`).
- `Cell<i32>` / `RefCell<i32>` / `Rc<RefCell<i32>>` / `RefCell<String>` (immutable Java values) need shared
  mutation → the one-element array cell convention used for `&mut` (`int[] c = {0}`, `c[0]`), so
  `get()`/`set()`/`replace()`/`*x.borrow_mut() += 1` work and sharing via `Rc::clone` aliases the array.
- `Rc::strong_count`, `Weak`, `Rc::ptr_eq`: `ptr_eq` → `==`; the others are unsupported unless you find
  a faithful std-only lowering.
- `Rc<str>`, `Box<str>`, `Box<[T]>` → `String` / `List<T>`.

## Closures as values

- `Box<dyn Fn(i32) -> i32>` in collections, `impl Fn` return types, `F: Fn(..)` / `FnMut` / `FnOnce`
  params, `fn` pointers (`fn(i32) -> i32`), calling `f(x)` on any of those (`apply`/`test`/`accept`/`get`
  per `Ty::fn_call_method`).
- `FnMut` closures that mutate captured state (counters, accumulators) — captured locals become cells (§8).
- Closures stored in struct fields and called as `(self.f)(x)`.
- Recursive data with `Option<Box<Node>>` (linked lists, trees): `node.next.as_ref()`, `take()`,
  `as_mut()` walks.

## Done when

`ptr_*` cases cover each bullet and the ptr probe passes; full suite passes.
