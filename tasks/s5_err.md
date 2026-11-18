# s5 — parse errors and Result unwrap (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/std_map/option.rs`: `error_discarded`, `parse_result`, `parse_int`,
`parse_float`, the Result `unwrap`/`expect` arms of `lower_result_method`; and
`std_map/text.rs::direct_parse`/`lower_direct_parse`. Test prefix: `short_err_`.

`parse` builds Rust's exact error text (a ~700-char line) whenever the error *might* be shown:

```java
String text_2 = text; Result<Long, Object> res_3; try { long num_4 = Long.parseLong(text_2); if (num_4 >= Long.MIN_VALUE && ...) ... } catch (NumberFormatException ex_5) { res_3 = new Err<>(new NumberFormatException((text_2.isEmpty() ? "cannot parse integer from empty string" : ...
```

1. Extend `error_discarded` so the short form (`try { res = new Ok<>(Integer.parseInt(s)); } catch
   (NumberFormatException e) { res = new Err<>(e); }`) is used when the error value is never printed:
   `.map_err(|_| ...)`, `.unwrap_or_else(|_| ...)`, `if let Ok(x) = s.parse()`, `match` whose `Err`
   arm binds `_`, `.ok()?`, `.is_err()`.
2. `parse::<i64>()` / `u64` in the long path: the `num >= Long.MIN_VALUE && num <= Long.MAX_VALUE`
   check is always true — drop it.
3. `r.unwrap()` on a Result whose error is a parse error: the `unwrap(err_value -> switch (...) {...})`
   lambda only builds the stderr panic text — use plain `r.unwrap()` (stderr isn't compared).
4. `f64` parse: same short form as ints when the error is discarded (`Double.parseDouble`).

Measure with `tasks/size.sh 'cannot parse integer' 'unwrap\(err_value' 'NumberFormatException'`.
