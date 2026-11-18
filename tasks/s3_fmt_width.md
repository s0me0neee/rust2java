# s3 — width, alignment and precision in format strings (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/fmt.rs`: `format_one`, `lower_printf`, `printf_conv`, `has_flags`,
`hoist_string`. Test prefix: `short_fmtw_`.

Padding is built by hand with code-point counts:

```java
fmt_text_1 + " ".repeat(Math.max(0, 5 - fmt_text_1.codePointCount(0, fmt_text_1.length()))) + "|" + (" ".repeat(Math.max(0, 5 - fmt_text_2.codePointCount(...))) + fmt_text_2)
```

1. `{:<w}` / `{:>w}` with the default space fill on strings and numbers → `String.format("%-5s", x)` /
   `"%5s"` (or fold into one `String.format` for the whole line, which `lower_printf` already does for
   some specs). Java pads by UTF-16 units, Rust by chars: only differs for non-BMP text — accept.
2. `{:^w}` (center) has no printf form; keep it, but compute the pad once into a temp on the same line
   instead of repeating `codePointCount` three times.
3. Custom fill (`{:*>8}`): `String.format("%8s", x).replace(' ', '*')` is wrong if `x` has spaces —
   keep the current form there.
4. `{:#08x}`-style (alternate + zero pad): use `String.format("%#010x", ...)`-like conversions where
   Java's output is identical; check against rustc with a test.
5. Remove temps (`fmt_text_N`) that are used once after these changes.

Measure with `tasks/size.sh 'codePointCount' '\.repeat\(Math\.max' 'String\.format'`.
