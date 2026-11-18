# s8 — strings, chars and numbers (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** `src/lower/std_map/text.rs` (except `direct_parse`/`lower_direct_parse`/`parse_expr`) and
`src/lower/std_map/num.rs`. Test prefix: `short_txt_`.

```java
int digit_4 = Character.digit(c, 10); sum += (digit_4 >= 0 ? Optional.of(digit_4) : Optional.<Integer>empty()).orElseThrow();
switch (Integer.signum(a.compareTo(b))) { case -1 -> "Less"; case 0 -> "Equal"; default -> "Greater"; }
('a' & 0xFF) + 1
```

1. `c.to_digit(r).unwrap()` / `.expect(..)` → hoist once and throw when negative:
   `int d = Character.digit(c, 10); if (d < 0) throw new NumberFormatException();` — or, when the
   char is known to be a digit (filtered by `is_ascii_digit`/`is_digit` just before), `c - '0'`.
   Keep the Optional form when the Option is used as an Option.
2. Masks that can't change the value: `('a' & 0xFF)` for a char literal / u8 literal ≤ 255,
   `(x & 0xFF)` right after a value that is already in range (e.g. `b as u8` from a `u8`). Find them
   in `num.rs` and skip the mask when the operand is a literal or already masked.
3. `Integer.signum(...)` around `compareTo` when the result is only compared with `< 0`, `== 0`,
   `> 0` or matched against -1/0/1 in a switch with `default` (sign-only use): drop where provably
   sign-only; keep in `switch` with `case -1`/`case 1` both present.
4. `s.chars().mapToObj(c -> (char) c).toList()` (collecting a `Vec<char>`) is fine; but
   `s.chars().count()` style → `s.length()` when the string is ASCII-only is NOT knowable — leave.
   Look for 2–3 other text.rs lowerings with an obviously shorter Java 21 form (e.g. `String.repeat`,
   `strip*`, `chars().allMatch`, `String.join`) and do those.

Measure with `tasks/size.sh 'Character\.digit' '& 0xFF' 'Integer\.signum'`.
