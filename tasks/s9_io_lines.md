# s9 — line-oriented input loops (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** `src/lower/std_map/io.rs`. Test prefix: `short_io_`.

Typical competitive / CLI input loops and what they should become (stdin is a `BufferedReader` in line
mode — see `cx.stdin_lines`, `is_line_stdin`, `read_line_mode`):

```rust
for line in io::stdin().lock().lines() { let line = line.unwrap(); ... }      // → for (String line : (Iterable<String>) STDIN.lines()::iterator) {
let lines: Vec<String> = io::stdin().lines().map(|l| l.unwrap()).collect();  // → var lines = STDIN.lines().toList();
let mut input = String::new(); io::stdin().read_to_string(&mut input).unwrap(); // → input += STDIN.lines().map(l -> l + "\n").collect(Collectors.joining());
let n: usize = lines.next().unwrap().unwrap().trim().parse().unwrap();       // (lines = stdin.lines()) → STDIN.readLine() is checked; use the Iterator form
```

1. Write a case with each of these idioms (and `BufRead::lines` on `stdin.lock()`), record rustc
   output, and look at what is generated today.
2. In line mode, `lines()` items that are immediately unwrapped (`.unwrap()`, `.expect()`, `flatten`,
   `map_while(Result::ok)`, `?` in a fn returning io::Result) should be plain `String`s: no
   `Result<String, Object>` wrapping, no `Ok::new`, no Iterable cast of a Result stream. Coordinate
   with the for-loop code by keeping your change inside io.rs (`lines_stream` gets enough context if
   you pass the parent node).
3. `let mut it = stdin.lock().lines(); it.next().unwrap().unwrap()` (pulled iterator) → check it
   comes out as `it.next()` on an `Iterator<String>`.
4. Byte mode (`cx.stdin_lines == false`): leave behavior, but remove the `((InputStream) X)` cast when
   `X` is `STDIN` (already an InputStream).

Measure with `tasks/size.sh 'Ok::new' '\(InputStream\)' 'Result<String, Object>'`.
