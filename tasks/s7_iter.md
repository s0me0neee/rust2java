# s7 — iterator chains and for-loops (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** `src/lower/iter.rs` (collect/adaptor lowering and `for` loops). Test prefix: `short_iter_`.

```java
xs.stream().map(...).collect(Collectors.toCollection(ArrayList::new))
for (var line : (Iterable<Result<String, Object>>) reader.lines().<Result<String, Object>>map(Ok::new)::iterator) {
IntStream.range(0, v.size()).mapToObj(i_3 -> Map.entry(i_3, v.get(i_3)))
d.chars().mapToObj(c -> (char) c).map(String::valueOf).collect(Collectors.joining())
```

1. `collect::<Vec<_>>()` into a binding that is only read (`cx.binding_read_only(ip)`, already used
   for `List.of` literals) or into a temporary that is only iterated/printed → `.toList()` instead of
   `Collectors.toCollection(ArrayList::new)`. Keep the ArrayList form when the result is mutated.
2. `for x in v.iter().enumerate()` over a list → `for (int i = 0; i < v.size(); i++) { var x = v.get(i);`
   if not already; `for (a, b) in v.iter().zip(w)` → index loop over the shorter size.
3. `(Iterable<T>) stream::iterator` in for-loops: when the stream is `list.stream()` with no adaptors,
   iterate the list directly.
4. `.collect::<String>()` of chars → `.map(String::valueOf).collect(Collectors.joining())` is fine, but
   `s.chars().rev().collect::<String>()` → `new StringBuilder(s).reverse().toString()`.
(Unwrapped `stdin.lines()` items are s9's job; don't touch the Result/`Ok::new` wrapping.)

Measure with `tasks/size.sh 'Collectors\.toCollection' '\(Iterable<' 'IntStream\.range'`.
