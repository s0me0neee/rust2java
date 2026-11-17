# Worker 4 — std library: strings, chars, numbers, Option, collections, I/O

Read `tasks/COMMON.md` and `DESIGN.md` (§6, §6.1, §6.2, §9.2, §11.1, §12) first.

**You own:** `src/lower/std_map.rs`. Test prefix: `std_`.

Iterator-producing / iterator-consuming methods (`iter`, `chars`, `split`, `lines`, `map`, `collect`, …)
belong to worker 5 (`iter.rs` is consulted before `std_map`), so don't implement those.

## Scope

1. **Std function calls** (`lower_std_fn_call`, keys like `Vec::new`, `String::with_capacity`,
   `HashMap::new`, `i32::from_str_radix`, `char::from_u32`, `char::from_digit`, `cmp::max`, `cmp::min`,
   `mem::swap`/`replace`/`take`, `process::exit`, `env::args`, `io::stdin`, `fs::read_to_string`,
   `fs::write`, `Default::default`, `From::from`, `String::from_utf8`, `Box::new`, `Rc::new`,
   `RefCell::new`, …). Print the key when unsure: `std_fn_key` builds it.
2. **Std methods** (`lower_std_method`), per the DESIGN tables:
   - integers/floats (§6.1): `abs`, `pow`, `min`, `max`, `rem_euclid`, `checked_*`, `saturating_*`,
     `wrapping_*`, `count_ones`, `sqrt`, `floor`, `powi`, `powf`, `is_nan`, `to_string`, `cmp`, `partial_cmp`…
   - `String`/`&str` (§6.2): `len`, `is_empty`, `push_str`/`push`/`clear`/`insert`/`pop`/`truncate` (mutation
     = assignment back to the place via `expr::place_assign`), `contains`, `starts_with`, `find`, `replace`,
     `trim*`, `to_uppercase`, `parse::<T>()` (one-line try/catch when the Result is kept), `as_str`,
     `to_string`, `to_owned`, `repeat`, `char_indices` is worker 5's.
   - `char`: `is_*`, `to_digit`, `to_ascii_uppercase`, `as` conversions.
   - `Option` (§9.2), `Result` methods not covered by `Result.java`.
   - `Vec`/slices, `HashMap`/`BTreeMap`/`HashSet`/`BTreeSet`/`VecDeque`/`BinaryHeap` (§11.1), including the
     entry-API idioms and `Reverse` heaps.
   - `clone()` per type (§7.2), `cmp`/`Ordering` (`Ty::Ordering` is an `int`).
3. **Paths/consts** (`lower_std_path`): `None`, `i32::MAX`, `u64::MAX`, `f64::consts::PI`, `f64::INFINITY`,
   `usize::MAX`, `std::f64::EPSILON`, `Ordering::Less`…
4. **Constructors** (`lower_std_ctor`): `Some`, `Ok`, `Err`, `Reverse`.
5. **Arrays & slicing**: `lower_array_expr` (`[1, 2, 3]`, `[0; n]`, nested `[[0; 3]; 3]` with independent rows),
   `lower_slice_range` (`&v[a..b]` → `subList`, `&s[a..]` → `substring`).
6. **`&mut` to primitives/Strings** passed to std (`mem::swap(&mut a, &mut b)` on locals → temp swap on one line).
7. **I/O** (§12): stdin `read_line` (the `STDIN` field goes on the last line via `cx.file_trailer`),
   `lines()` is worker 5's (it returns an iterator) — coordinate by exposing the `STDIN` declaration helper.
   `fs::read_to_string` etc. with one-line try/catch. `env::args()`.

## Done when

`std_*` cases cover each bullet (use stdin files for I/O) and pass; full suite passes.
