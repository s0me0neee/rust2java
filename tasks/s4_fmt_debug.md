# s4 — `{:?}` of Option / Vec / Map / tuples (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/fmt.rs`: `debug`, `debug_sequence`, `debug_lambda`, `debug_with_spec`,
`debug_operand`, `debug_is_to_string`. Test prefix: `short_fmtd_`.

```java
fmt_arg_12.map(fmt_some_13 -> "Some(" + ("(" + ("\"" + ...) + ", " + ...) + ")") + ")").orElse("None")
list.stream().map(fmt_item_3 -> String.valueOf(fmt_item_3)).collect(Collectors.joining(", ", "[", "]"))
```

1. Wherever Rust Debug text equals Java's `toString()`, use the value itself: `List<Integer>`,
   `List<Boolean>`, `List<List<Integer>>`, `Set`/`Map` of those (Java prints `{a=1}` for maps — not
   equal, keep maps), `Optional`? (Java prints `Optional[1]` — not equal). Extend
   `debug_is_to_string` accordingly and use it at every element level.
2. `.map(x -> String.valueOf(x))` → `.map(String::valueOf)`; `x -> x.debug()` → `T::debug` when the
   element type is nameable.
3. Redundant parentheses around concatenations inside concatenations: `"Some(" + ("(" + a + ")") + ")"`
   → `"Some((" + a + "))"` (merge adjacent string literals when building the Expr).
4. Option Debug: `o.map(v -> "Some(" + v + ")").orElse("None")` is fine; make sure the inner part
   uses the shortest form from 1–3.

Measure with `tasks/size.sh 'Collectors\.joining' 'fmt_some_' 'fmt_item_'`.
