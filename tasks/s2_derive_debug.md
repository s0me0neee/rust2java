# s2 — derived Debug and Ord bodies (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/items.rs`: `debug_method`, `debug_value`, `debug_seq`; and the derived
`compareTo` generation (search `compareTo` in items.rs, the derive path only). Test prefix: `short_dbg_`.

```java
public String debug() { return "Version { major: " + String.valueOf(this.major) + ", minor: " + String.valueOf(this.minor) + ", tags: " + this.tags.stream().map(x0 -> "\"" + x0 + "\"").collect(Collectors.joining(", ", "[", "]")) + " }"; }
```

1. Drop `String.valueOf(...)` around a non-first operand of `+` (string concatenation already does it).
2. Use `"Version { major: %s, minor: %s, tags: %s }".formatted(...)` when there are 3+ fields and it is
   shorter; otherwise plain concatenation.
3. `debug_seq` for elements whose Debug equals Java's `toString` (ints, bools, chars-in-lists are NOT
   equal — Rust quotes chars): use the list itself (`this.tags.toString()` for `Vec<i32>`), like
   `fmt::debug_is_to_string` does.
4. Derived `compareTo` for structs: `Comparator.comparing(...).thenComparing(...)` is longer in Java;
   prefer `int c = Integer.compare(a, other.a); if (c != 0) return c; ...` only if it isn't already;
   otherwise leave it. Report what you found.
5. Strings inside derived Debug are printed as `"\"" + x + "\""` — no escaping, unlike `{:?}` on a
   String (which uses `debug_str`). Make them consistent by calling `fmt::debug` for `Ty::Str`/`Ty::Char`.

Measure with `tasks/size.sh 'public String debug' 'String\.valueOf'`.
