# Worker 6 — strings, chars, bool

Read `tasks/COMMON.md` and `DESIGN.md` (§6.2, §10, §11) first.

**You own:** `src/lower/std_map/text.rs`. Test prefix: `text_`. Probe: `tasks/probes/str.rs`.

## Already done (w4/w5, extend, don't redo)

`lower_str_method` / `lower_small_method` cover len, push/push_str/insert/pop/truncate/clear, contains,
starts/ends_with, find, replace, trim*, to_upper/lowercase, parse, repeat, char `is_*`/to_digit/
to_ascii_*. Iterator-shaped methods (`chars`, `split`, `lines`, `split_whitespace`, `char_indices`,
`bytes`) are in `iter.rs` (worker 5 is done — you may extend `iter.rs` sources for strings).

## Gaps found by the probe

- `split_once`, `rsplit_once` (→ `Optional<Map.Entry<String,String>>`), `replacen`, `insert_str`,
  `strip_prefix`/`strip_suffix` (check), `rfind`, `as_bytes` (→ `List<Integer>`), `get(a..b)`,
  `eq_ignore_ascii_case`, `cmp` on strings (`compareTo`, sign-normalised if compared to `Less`…).
- `split` by closure / char set (`split(|c: char| …)`, `split(&['-', '_'][..])`, `split(char::is_whitespace)`).
- `String::from` / `str::to_string` / `ToString::to_string` used as **function values**
  (`.map(String::from)`, `.map(str::to_string)`) — `std path String::from`.
- `'x'.into()` → String, `String + &str`, `&s[a..b]` on non-ASCII text (byte vs char offsets: Rust
  slices by bytes; document the ASCII assumption or compute via `offsetByCodePoints`).
- `char::is_alphanumeric` etc. as function values; `c.is_ascii_punctuation()`, `to_digit(16)`,
  `char::from(u8)`, `(b'a' + 1) as char`, byte literals `b'x'`, byte strings `b"ab"`.
- `format!` results compared with `==` must use `equals` (check `lower_bin` handles `String == String`).

## Done when

`text_*` cases cover each bullet and the str probe's lines pass; full suite passes.
