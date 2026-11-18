# s6 — Optional plumbing and Java 21 SequencedCollection (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/std_map/option.rs`: `lower_option_method` (not the Result or parse parts);
in `src/lower/std_map/coll.rs`: the Vec/VecDeque `first`/`last`/`get`/`pop`/`first_mut`/`last_mut`
arms. Test prefix: `short_opt_`.

```java
(a.isPresent() && b.isPresent() ? Optional.of(Map.entry(a.get(), b.get())) : Optional.<Map.Entry<Integer, Character>>empty())
i < v.size() ? Optional.of(v.get(i)) : Optional.<Integer>empty()
```

1. `v.first()` → `v.stream().findFirst()`; `v.last()` → `v.reversed().stream().findFirst()`
   (Java 21 `reversed()` is a view, no copy). `v.pop()` → `v.isEmpty() ? Optional.empty() :
   Optional.of(v.removeLast())` stays (needs the side effect), but use `removeLast()`/`getLast()`
   (Java 21) everywhere instead of `remove(v.size() - 1)` / `get(v.size() - 1)`.
2. `a.zip(b)` → `a.flatMap(x -> b.map(y -> Map.entry(x, y)))`; `a.and(b)` → `a.flatMap(x -> b)`;
   `a.xor(b)` keep; `a.or(b)` → `a.or(() -> b)`; `filter`, `take`, `replace`: check each for a
   shorter std form.
3. Explicit `Optional.<T>empty()` type witnesses are only needed where javac can't infer; drop them
   in assignment/return/argument position (keep when it's the receiver of a call or in a ternary
   whose other branch is `Optional.of(..)` — test with javac).

Measure with `tasks/size.sh 'Optional\.<[^>]*>empty\(\)' 'size\(\) - 1' 'isPresent\(\) &&'`.
