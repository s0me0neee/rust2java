# s1 — records and derived equality (≈20 min)

Read `tasks/COMMON.md` (the "Shortening round" section first).

**You own** in `src/lower/items.rs`: `lower_variant_record`, `derive_methods`, `equals_method`,
`hash_method`, `clone_value`. Test prefix: `short_rec_`.

Data-enum variants are already Java `record`s, but every variant still writes its own `equals`,
`clone` and (sometimes) `hashCode`:

```java
record Num(long _0) implements Token { public String debug() {...} public Num clone() { return new Num(this._0); }
    public boolean equals(Object o) { if (!(o instanceof Num other)) return false; return Objects.equals(this._0, other._0); } ... }
```

1. A record already has component-wise `equals`/`hashCode`. Drop the generated ones for records when
   that gives the same result as Rust's derive (all component types except `f32`/`f64`, where records
   compare with `Double.compare`; keep explicit ones there).
2. `clone()` on a record whose components are all immutable (primitives, String, other records,
   enums) is `return this;` or can be dropped entirely if nothing calls `clone()` on it
   (check `lower_derive_method_call` / std `clone` lowering before dropping).
3. Struct classes: `hashCode` → `Objects.hash(a, b, c)`; `equals` → keep `instanceof X other` but
   join fields with `&&` using `==` for primitives and `Objects.equals` only for references.
4. Unit variants (`record Plus()`): `equals` via `o instanceof Plus` is what records already do — drop.

Measure with `tasks/size.sh 'public boolean equals' 'public int hashCode' 'clone\(\)'`.
