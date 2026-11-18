# rust2java — Design

A source-to-source transpiler that turns Rust (1.95+, edition 2021/2024) into readable Java 21. Each Rust construct becomes one Java construct, and each Rust statement lands on the same line number in the Java file.

---

## 1. Goals and non-goals

### Goals (in priority order)

1. **Same program output.** A transpiled program prints the same **stdout** as the Rust program on the same input and exits with the same code. A panic exits with 101.
2. **Line correspondence.** A Rust statement on line *L* of `foo.rs` is emitted on line *L* of `Foo.java`. Where that can't be done, the drift stays local and is reported (§4).
3. **Readable, plain Java.** The output uses the **Java standard library** (`java.util`, `java.util.stream`, `java.nio.file`). Names are kept exactly as written in Rust. When Java has no direct equivalent, the fallback is a stream pipeline. There is **no helper library**. The only non-std code emitted is a `Result` type and, rarely, records for tuples with 3+ elements (§13).
4. **std first.** Cover the parts of Rust's `std` that light programs use (§14). External crates are out of scope for now.

### Assumptions

- The input already compiles and runs under `rustc` ≥ 1.95. The transpiler does no borrow checking or validation of its own and never needs to reason about invalid code.
- There is **one mode**: no semantic flags.

### Non-goals

- Exact integer type fidelity. `u8` is `int`, and the other unsigned types map to the same-width signed Java type (§6.1). Values outside the Java type's range, or `u8` arithmetic past 255, are allowed to behave differently.
- Memory layout, performance, allocation, and `Drop` timing.
- `unsafe`, raw pointers, FFI, `async`, `static mut`, `catch_unwind`, proc macros other than std derives, and external crates.
- `HashMap`/`HashSet` iteration order. Rust randomises it per run, so programs whose output depends on it are already nondeterministic.
- Byte-exact stderr. Only the panic exit code (101) is guaranteed. The panic message text is best-effort; Java exceptions keep their Java messages.

---

## 2. Usage and output

```
rust2java <cargo project dir | file.rs ...> [-o dir (default: current directory)] [--run]
```

A bare `file.rs` is wrapped in a temporary cargo project so the frontend can load it.

A crate that is one source file (every `.rs` input) becomes one Java file: `<dir>/<name>.java`, holding a
package-private `final class Main` with no package line, plus `Result`/`Ok`/`Err` appended after the
class when the program uses Result (§13). Run it with `java <name>.java`, or `--run` (classes go to a temp dir). The hidden
`--line-report` flag also writes `<dir>/<name>.lines.json` (statement line-match report, for the tests).

A cargo project with several module files keeps one class per module in a package:

```
out/src/<crate>/Main.java       from src/main.rs
out/src/<crate>/Geometry.java   from src/geometry.rs (mod geometry)
out/src/<crate>/Result.java     only if the program uses Result (§13)
```

To build and run that: `javac -d out/classes $(fd -e java . out/src) && java -cp out/classes <crate>.Main`. `--run` does both.

---

## 3. Architecture

```
Rust source (cargo project)
   │  ra_ap_load-cargo  (std types come from the rust-src component)
   ▼
Frontend ─ lossless syntax tree: comments, exact line/col
         ─ Semantics: type of every expr/pattern, resolved paths & method calls,
                      macro expansions, trait impls
   │  lower/*  — one lowering fn per Rust construct + std mapping tables
   ▼
JIR  (Java-shaped AST; every stmt and every breakable sub-expr carries `line`)
   │  passes: shadow-rename → cell-boxing → expr→stmt hoisting → idioms → imports
   ▼
Line-aware printer → .java files + lines.json
```

### 3.1 Frontend: rust-analyzer as a library (`ra_ap_*`)

`syn` has no type information, and Rust method calls can't be translated without types: `x.len()` means something different on a `Vec`, a `String` and a `HashMap`. `rustc_private` has exact types, but it requires a pinned nightly and only exposes code after `println!`, `for` and `?` have been desugared. rust-analyzer combines a lossless syntax tree that keeps macro calls intact with a semantic layer (`Semantics::type_of_expr`, `resolve_method_call`, `resolve_path`, `expand`), and it runs on stable.

- **Version.** Pin every `ra_ap_*` crate to the same exact version (`=0.0.N`), choosing a release from after Rust 1.95 so the parser and type checker understand 1.95+ syntax (let chains and edition 2024 rules). Bump the pin deliberately whenever newer Rust syntax needs to be supported.
- **Isolation.** Only `src/frontend/` imports `ra_ap_*`. It exposes a façade (`ty_of`, `method_target`, `path_target`, `line_of`, `expand`, …), so an upgrade touches one module.
- **Unknown types.** If the frontend reports `{unknown}`, emit a diagnostic and stop. Never guess.

### 3.2 JIR (Java IR)

A small Java AST. It exists only for emitting code, not for analysing it.

```rust
struct Line(u32);
struct At<T> { line: Line, node: T }   // items, stmts, call args, chained receivers
enum JItem { Class, Interface, Record, Enum, Method, Field, Comment }
enum JStmt { Local, Expr, If, While, For, ForEach, Switch, Return, Break, Continue, Block, Labeled, Throw, Try, Comment }
enum JExpr { Lit, Name, Field, Call, New, Lambda, MethodRef, Binary, Unary, Assign, Ternary, Switch, Cast, InstanceOf, ArrayInit }
```

### 3.3 Passes

1. **Shadow rename.** `let x = …; let x = …;` in the same Java scope becomes `x`, then `x_1`. This is the only renaming besides keyword escapes.
2. **Cell-boxing.** Wraps locals that need a mutable cell (§8).
3. **Hoisting.** A block, `if`, `match` or `loop` that can't be a Java expression is lowered into statements that assign to a destination variable (§9).
4. **Idioms.** Rewrites common Rust patterns into idiomatic Java (§9, §11.1). The generic lowering is always correct on its own, so this pass is optional.
5. **Imports.** Collects the imports the file needs.

---

## 4. Line preservation

### 4.1 Printer

The printer tracks `cur_line`. For each node tagged `line`:

- `line > cur_line`: it emits newlines until `cur_line == line`.
- `line <= cur_line`: it emits the node on the current line. Java allows several statements on one line, so when Java needs more code than Rust, the extra goes sideways and lines further down are not shifted.

Closing braces go on the Rust closing brace's line. Chained method calls and argument lists that span several lines in Rust keep their line breaks. Long Java lines are accepted.

### 4.2 Fixed placements

| Rust | Java |
|---|---|
| line 1 | `import java.util.*; import java.util.stream.*; final class Main {` (followed by whatever Rust line 1 contains; multi-module crates add `package <crate>;` and `public`) |
| `use …;` | blank line. Imports are emitted on line 1. |
| `#[derive(…)]` and other attributes | blank line. Derived methods are emitted on the type's closing-brace line (§7.2). |
| comments (`//`, `/* */`, `///`, `//!`) | copied verbatim to the same line |
| `fn main()` | `static void main() {…}`. The Java entry point (§9.3), any file-level fields it needs (`ARGS`, `STDIN`), and the class's closing `}` are appended to the last line of the file. |

### 4.3 Item placement

Java requires methods inside their class body, while Rust lets `impl` blocks appear anywhere. A type's Java body therefore spans from its `struct`/`enum` line to the closing brace of its last `impl` in the same file. Inside that span, the struct's own closing brace and the `impl X {` header lines become blank lines. The usual layout (`struct S`, `impl S`, `impl Trait for S`, then the next type) has **zero drift**. An unrelated item inside a span moves to just after it, and trait impls in other files move into the type's file. Both moves are recorded in `lines.json`.

### 4.4 Measuring

The test harness reports, for every Rust statement, whether it landed on its exact line, plus the maximum drift. **Target: ≥ 95% of statements on their exact line.**

---

## 5. Mapping principles

1. **Java std directly** when the semantics already match: `ArrayList.add`, `HashMap.put`, `String.startsWith`, `Math.max`.
2. **Java std with an adapter** when only the shape differs: `Optional.ofNullable(m.get(k))`, `s.split(Pattern.quote(sep), -1)`.
3. **Stream pipeline** when Java has no single method for it: iterator adaptors, `windows`, `chunks`, `filter_map`, `position`.
4. **Inline statements** when steps 1–3 would evaluate an operand twice or let a checked exception escape. The operand goes into a hoisted temporary, or the call into a one-line `try`/`catch`, on the same Rust line.
5. **Generated types** only where Java std has no equivalent *type*: `Result`, tuples with 3+ elements, and functional interfaces with 3+ parameters (§13). There are **no helper functions**.

**Names.** Every identifier is kept exactly as written (`snake_case` stays `snake_case`). Exceptions:
- Java keywords get a `_` suffix (`new` → `new_`, `default` → `default_`, `final` → `final_`).
- Shadowed bindings get `_1`, `_2`, ….
- Module files become PascalCase class names, because Java requires a class per file (`geometry.rs` → `Geometry`).
- `Display::fmt` becomes `toString()` so it plugs into Java's string conversion (§7.3).

---

## 6. Type mapping

| Rust | Java |
|---|---|
| `i8` | `byte` |
| `u8` | `int` |
| `i16`/`u16` | `short` |
| `i32`/`u32`, `isize`/`usize` | `int` |
| `i64`/`u64` | `long` |
| `i128`/`u128` | `long` (values outside its range are unsupported) |
| `f32`/`f64` | `float`/`double` |
| `bool` | `boolean` |
| `char` | `char` (characters outside the BMP are unsupported) |
| `&str`, `String`, `Box<str>` | `String` |
| `()` | `void` (`Void` in generic positions) |
| `Vec<T>`, `[T; N]`, `&[T]`, `&mut [T]` | `List<T>`, constructed as `ArrayList`. Sub-slices are `subList` views. |
| `HashMap`/`HashSet` | `HashMap`/`HashSet` |
| `BTreeMap`/`BTreeSet` | `TreeMap`/`TreeSet` |
| `VecDeque<T>` | `ArrayDeque<T>` |
| `BinaryHeap<T>` | `PriorityQueue<T>(Comparator.reverseOrder())`; for `BinaryHeap<Reverse<T>>`, natural order with the `Reverse` wrapper dropped |
| `Option<T>` | `Optional<T>` |
| `Result<T, E>` | generated `Result<T, E>`: a sealed interface with records `Ok(T value)` and `Err(E error)` (§13). Java has no equivalent. |
| pairs `(A, B)` | `Map.Entry<A, B>`, built with `Map.entry(a, b)`. `t.0` and `t.1` become `getKey()` and `getValue()`. This also lines up with map iteration, which already yields entries. |
| tuples `(A, B, C, …)` | a generated `record Tuple3<A, B, C>(A _0, B _1, C _2)`, emitted only for the arities the program uses |
| `Box<T>`, `Rc<T>`, `Arc<T>` | `T` |
| `RefCell<T>`, `Cell<T>` | `T`, or a one-element array cell (§8). `borrow()` and `borrow_mut()` disappear. |
| `&T`, `&mut T` | `T`, or a cell when `&mut` points at a primitive or `String` (§8) |
| `std::cmp::Ordering` | `int` from `compare`/`compareTo`. `Less`/`Equal`/`Greater` become `< 0`, `== 0`, `> 0`, and `match` on it becomes an `if` chain. |
| closures, `fn` pointers, `impl Fn`, `Box<dyn Fn>` | `java.util.function` interfaces (`Function`, `BiFunction`, `Supplier`, `Consumer`, `BiConsumer`, `Predicate`, `IntBinaryOperator`, `Runnable`, …). Higher arities use a generated `@FunctionalInterface`. |
| `dyn Trait`, `impl Trait` | the trait's interface, or a type parameter |

### 6.1 Integers

- Arithmetic uses Java's natural wrapping. Division and remainder truncate the same way in both languages. Dividing by zero throws `ArithmeticException`, which becomes a panic with exit code 101.
- `byte` and `short` arithmetic promotes to `int` in Java, so an assignment back to an `i8`/`i16`/`u16` place gets the cast Java requires: `(byte) (a + b)`. `u8` is a plain `int`, so it needs no casts, and byte literals `b'a'` become `'a'`.
- `as` casts become Java casts. Both languages wrap when narrowing an integer, and both saturate (with NaN → 0) when casting a float to an integer.
- Methods map to `Math`, `Integer` and `Long`:

  | Rust | Java |
  |---|---|
  | `abs`, `pow`, `min`, `max` | `Math.abs`, `(int) Math.pow(a, b)` (exact while results fit in 53 bits), `Math.min`, `Math.max` |
  | `rem_euclid` | `Math.floorMod` |
  | `count_ones`, `leading_zeros`, `trailing_zeros` | `Integer.bitCount`, `Integer.numberOfLeadingZeros`, `Integer.numberOfTrailingZeros` |
  | `checked_sub` on unsigned, `checked_div` | `a >= b ? Optional.of(a - b) : Optional.empty()`, `b != 0 ? Optional.of(a / b) : Optional.empty()` |
  | `checked_add`, `checked_mul` | `Math.addExact`/`multiplyExact` in a one-line `try`/`catch (ArithmeticException)` |
  | `wrapping_*` | plain operators |
  | `saturating_sub` on unsigned, `saturating_add` | `Math.max(a - b, 0)`, `(int) Math.min((long) a + b, Integer.MAX_VALUE)` |
  | `i32::MAX` | `Integer.MAX_VALUE` |

- Numeric literal suffixes are dropped. `1_000` stays `1_000`, because Java also allows underscores in number literals.

### 6.2 Strings and chars

- Strings are Java `String`, and lengths and indices are Java `char` positions. That's identical to Rust's byte offsets for ASCII; other text is best-effort.
- **Mutation.** A Java `String` is immutable, so mutating methods become assignments to the place: `s.push_str(t)` → `s += t;`, `s.push(c)` → `s += c;`, `s.clear()` → `s = "";`, `s.insert(i, c)` → `s = s.substring(0, i) + c + s.substring(i);`. A `&mut String` parameter is a cell (§8).

| Rust | Java |
|---|---|
| `len`, `is_empty` | `length()`, `isEmpty()` |
| `&s[a..b]`, `&s[a..]` | `substring(a, b)`, `substring(a)` |
| `contains`, `starts_with`, `ends_with`, `find(str)`, `replace`, `repeat` | `contains`, `startsWith`, `endsWith`, `indexOf` wrapped as `Optional` (−1 → empty), `replace`, `repeat` |
| `trim`, `trim_start`, `trim_end` | `strip`, `stripLeading`, `stripTrailing` |
| `to_uppercase`, `to_lowercase` | `toUpperCase(Locale.ROOT)`, `toLowerCase(Locale.ROOT)` |
| `split(sep)` | `Arrays.stream(s.split(Pattern.quote(sep), -1))`. The `-1` keeps trailing empty strings, like Rust. |
| `split_whitespace` | `Arrays.stream(s.strip().split("\\s+")).filter(w -> !w.isEmpty())` |
| `lines` | `s.lines()` (a Java 11 `Stream<String>`) |
| `chars` | `s.chars().mapToObj(c -> (char) c)`. `for c in s.chars()` → `for (char c : s.toCharArray())` |
| `bytes`, `as_bytes()[i]` | `s.chars()`, `s.charAt(i)` (the same values for ASCII, since `u8` is `int`) |
| `char_indices` | `IntStream.range(0, s.length()).mapToObj(i -> Map.entry(i, s.charAt(i)))` |
| `parse::<T>()` | `.unwrap()`, `.expect()` and `?` become a direct call: `Integer.parseInt(s)`, `Double.parseDouble(s)`. When the `Result` is actually used, the call is wrapped in a one-line `try { r = new Ok<>(Integer.parseInt(s)); } catch (NumberFormatException e) { r = new Err<>(e); }` |
| `to_string`, `to_owned`, `String::from`, `into` | `s` for strings; `String.valueOf(x)` for numbers and `char`; `x.toString()` for `Display` types; the float form from §10 for floats |
| `==`, `<` | `.equals`, `compareTo` (chosen from the static type) |
| `format!` | §10 |
| `char::is_alphabetic`, `is_numeric`, `is_alphanumeric`, `is_whitespace`, `is_ascii_digit`, `to_ascii_uppercase`, … | `Character.isLetter`, `Character.isDigit`, `Character.isLetterOrDigit`, `Character.isWhitespace`, `(c >= '0' && c <= '9')`, `Character.toUpperCase` |
| `c.to_digit(r)` | `Character.digit(c, r)` wrapped as `Optional` |
| `char::from_digit`, `b'a'` | `Character.forDigit`, `'a'` |

---

## 7. Structs, enums, traits, generics

### 7.1 Structs and enums

- **Structs** become `static final class` nested inside the module class. Fields stay where they are, and `pub` → `public`. Each struct gets a canonical constructor, emitted on the struct's closing-brace line.
- **Struct literals** `S { b: 2, a: 1 }` become `new S(1, 2)`. If reordering the fields would change the order of side effects, the field expressions are hoisted into temporaries on the same line. `..base` copies the remaining fields from `base`, which is evaluated once.
- **Tuple structs** get fields `_0`, `_1`, …. **Unit structs** get a single static instance.
- **Fieldless enums** become Java `enum`. Explicit discriminants become a constructor field, and `as i32` reads that field.
- **Data enums** become a `sealed interface E` with one nested `record` per variant. `match` then becomes `switch` with record patterns.
- If a variant's payload is mutated in place anywhere in the crate, that enum's variants become `final class`es, and matching uses type patterns plus field access.
- **Copy aggregates.** A `Copy` struct or array that is mutated in place somewhere in the crate is copied wherever Rust would implicitly copy it: `new ArrayList<>(a)` for arrays and a generated copy constructor for structs. Copy types that are never mutated need no copies.

### 7.2 Derives

Derived methods are emitted on the type's closing-brace line:

| derive | Java |
|---|---|
| `Debug` | `public String debug()`, producing Rust's format (`Point { x: 1, y: 2 }`, `Circle(1.0)`) |
| `Clone` | a copy constructor plus `public S clone()`. `.clone()` calls stay `.clone()`. For std types, `.clone()` becomes `new ArrayList<>(v)`, `new HashMap<>(m)`, …, deep when the element type is a mutable struct. On `String`, primitives and `Optional` it's dropped. |
| `PartialEq`, `Eq`, `Hash` | `equals`, `hashCode` (records get them for free) |
| `PartialOrd`, `Ord` | `implements Comparable<S>`, comparing fields in declaration order; for enums, variant index first |
| `Default` | `public static S default_()` |
| `Copy` | no code; it drives the copy rule in §7.1 |

### 7.3 Traits and impls

- A **trait** becomes an `interface`, with default methods as `default` methods and associated consts as `static final` fields. `impl Trait for UserType` adds `implements Trait` to the class header.
- **Inherent impls** become instance methods for `&self`, `&mut self` and `self` receivers, and `static` methods for associated functions. `Self` becomes the class name.
- **`impl Display`** emits `public String toString()`. Inside it, `f` is a `StringBuilder`, `write!(f, …)` → `f.append(…)`, `?` and the final `Ok(())` disappear, and `return f.toString();` goes on the closing line. String concatenation, `%s`, and padding like `{:>10}` then work on these types through plain Java. A manual `impl Debug` likewise becomes `public String debug()`.
- **`impl Display` on a data enum**: a Java interface cannot default `toString`, so the interface gets `default String display() { … }` and every variant record gets `public String toString() { return display(); }`. `impl Debug` on a data enum becomes `default String debug()`.
- **Trait impls on std types and primitives** (`impl Shape for i32`) become static methods in a `TraitNameImpls` class, and calls are dispatched statically because the types are known. Using such an impl through a generic bound gets a diagnostic for now.
- **Operator traits** (`Add`, `Sub`, `Mul<f64>`, `Div`, `Rem`, `Neg`, `Not`, `BitAnd/BitOr/BitXor`, `Shl/Shr`, `AddAssign`, …) become methods named after the Rust method (`add`, `sub`, `mul`, `rem`, `neg`, `not`, …). `a + b` becomes `a.add(b)`; `a += b` becomes `a.add_assign(b)`. The operand type decides the direction, so `impl Mul<V> for f64` (a user type on the right of an operator) is a diagnostic.
- **`impl Index`/`IndexMut`**: `g[i]` becomes `g.index(i)`, which also covers tuple indices (`g[(r, c)]` → `g.index(Map.entry(r, c))`). Java cannot assign through a method call, so an `impl IndexMut` whose body is `&mut <place>` additionally emits `void index_mut_set(<params>, Output value) { <place> = value; }`; `g[i] = v` becomes `g.index_mut_set(i, v)` and `g[i] += v` becomes `g.index_mut_set(i, g.index(i) + v)`. The Rust `index_mut` method itself is not emitted.
- **Manual std-trait impls** map onto their Java counterparts: `PartialEq` → `equals` (the parameter is matched with `instanceof`, so `impl PartialEq<Other> for S` works too), `Eq` → nothing, `Hash` → `hashCode` (`Objects.hash(…)` over the `self.f.hash(state)` calls), `Ord`/`PartialOrd` → `implements Comparable<S>` plus `compareTo` delegating to the Rust-named `cmp`/`partial_cmp` (where `Ordering` is the Java int -1/0/1), `Clone` → `clone`, `Default` → `default_`, `Debug` → `debug`. `PartialOrd` is only used for `compareTo` when there is no `Ord` impl for the same type.
- **`derive(PartialOrd, Ord)` on a data enum** compares variant order first: each record gets `variantIndex()` and `compareTo`, and the interface extends `Comparable<E>`.
- **`impl Iterator for T`** emits `next()` returning `Optional<Item>` plus `Stream<Item> stream() { return Stream.generate(() -> next().orElse(null)).takeWhile(Objects::nonNull); }`. `for x in t`, `t.take(…).collect()`, `t.sum()` and the other adaptors go through `t.stream()`. Calls to derived methods (`cmp`, `partial_cmp`, `eq`, `ne`) are redirected to the generated Java method names.
- **`From`/`Into`/`TryFrom`/`FromStr`**: `impl From<A> for B` emits `static B from(A)` on B, overloaded by parameter type; `B::from(a)` and `a.into()` (target taken from the expected type) both become `B.from(a)`. `TryFrom` is the same with `try_from` returning `Result`, and `x.try_into()` becomes `B.try_from(x)`. `impl FromStr for B` emits `static Result<B, E> from_str(String)`, so `s.parse::<B>()` becomes `B.from_str(s)`.
- **`impl Deref`/`DerefMut` for a newtype** emits the `deref`/`deref_mut` method; method calls, indexing and `for` loops on the wrapper forward through `w.deref()` (Java shares the reference, so `DerefMut` needs no separate path).

### 7.4 Generics

Rust generics become Java generics, with primitives boxed in type arguments. Std trait bounds:

| bound | Java |
|---|---|
| `Ord`/`PartialOrd` | `T extends Comparable<T>` |
| `Display`, `Debug`, `PartialEq`, `Hash`, `Clone` | no bound needed (methods dispatch through `Object`) |
| `Fn*` | functional interface |
| arithmetic (`T: Add`) on generic `T` | unsupported for now |

---

## 8. `&mut`, cells, closures

- `&x` and `&mut x` on objects (lists, maps, structs) are just `x`, since Java already shares the reference.
- `&mut x` where `x` is a primitive or `String` needs a mutable cell. `x` is declared for its whole lifetime as a **one-element array**, a common Java idiom:
  ```java
  int[] count = {0};          // let mut count = 0;   (taken as &mut later)
  bump(count);                // bump(&mut count);
  static void bump(int[] n) { n[0] += 1; }   // fn bump(n: &mut i32) { *n += 1; }
  ```
  Generic `T` uses `AtomicReference<T>`.
- `&mut v[i]` / `&mut s.f` on a primitive or `String` element: a temporary cell, the call, then a write-back, all on the same line.
- `*r = value` where `r: &mut Obj`: lists become `r.clear(); r.addAll(value);`, maps similar, and structs call a generated `assign(other)` that copies fields.
- `std::mem::swap`, `replace` and `take` are expanded into temp + assign sequences on the same line.
- **Closures** become lambdas, and `move` makes no difference. A local that a closure mutates becomes a one-element array, because Java requires captured locals to be effectively final.

---

## 9. Expressions, control flow, patterns

- **Values from blocks.** `let x = { stmts; e };` → `int x; { stmts; x = e; }`. The same destination-passing lowering applies to `if`, `match` and `loop` in value position.
- **`if`/`else` as a value** becomes `c ? a : b` when both branches are plain expressions.
- **`match` as a value** becomes a switch expression, unless an arm contains `return`, `break`, `continue` or `?`. Java forbids jumping out of a switch expression, so those cases become a switch statement that assigns to a destination.
- **Tail expressions** become `return`.
- **Loops.**
  - `loop` → `while (true)`.
  - Labels map directly (`'outer:` → `outer:`), and `break value` assigns the loop's destination first.
  - `for i in a..b` → `for (int i = a; i < b; i++)`, with `..=` → `<=`. `.rev()` and `.step_by(k)` become counted loops. Bounds that aren't pure are evaluated once.
  - `for x in &v` / `v.iter()` → `for (var x : v)`.
  - `for (i, x) in v.iter().enumerate()` → `for (int i = 0; i < v.size(); i++) { var x = v.get(i);`, on the same line.
  - Map iteration becomes `for (var e : m.entrySet()) { var k = e.getKey(); var v = e.getValue();`.
  - Any other iterator becomes `for (var it = <stream>.iterator(); it.hasNext();) { var x = it.next();`. That keeps `break`/`continue`/`return` working, which `forEach` would not.
- **Let chains (edition 2024).** `if let Some(x) = a && x > 3` → `if (a.isPresent() && a.get() > 3) { var x = a.get();`. `a` is hoisted first if it isn't a plain place. With record patterns it becomes `if (r instanceof Ok(var x) && x > 3)`, since Java scopes the binding the same way.
- **`let … else`** → `if (!cond) { diverge }`, followed by the bindings.
- **Shadowing** is handled by renaming (§3.3).
- **Evaluation order.** Rust and Java both evaluate left to right. One exception: for a primitive `a[i] op= f()`, Rust evaluates `f()` first, so `f()` is hoisted when both sides have side effects.

### 9.1 Patterns

| Rust | Java 21 |
|---|---|
| literals, `A \| B` | `case 1, 2 ->` / `case A, B ->` |
| enum variants with fields | record patterns `case Circle(var r) ->` (type arguments are inferred) |
| `_` inside a record pattern | `var _N`, a fresh name (unnamed patterns are preview-only in Java 21) |
| a whole variant ignored | `case Circle c ->` |
| guard `if cond` | `when cond` |
| `Some(x)`, `None` on `Optional` | `if (o.isPresent()) { var x = o.get(); … } else { … }` (`Optional` isn't a record, so it can't be matched with a record pattern) |
| `Ok(v)`, `Err(e)` | record patterns on the generated `Result` |
| ranges `1..=5`, slice patterns, `x @ p`, nested ORs that bind | an `if`/`else if` chain on a temporary |
| tuple destructuring `let (a, b) = t;` | `var a = t._0(); var b = t._1();` on the same line |
| `while let Some(x) = stack.pop()` | idiom `while (!stack.isEmpty()) { var x = stack.removeLast();`. Generic form: `for (Optional<T> o; (o = e).isPresent();) { var x = o.get();` |

Exhaustiveness was already checked by rustc. Switches over sealed types need no `default`. Other switches get `default -> throw new IllegalStateException("unreachable")`.

### 9.2 `Optional` (Rust `Option`)

| Rust | Java |
|---|---|
| `Some(x)`, `None` | `Optional.of(x)`, `Optional.empty()` |
| `is_some`, `is_none` | `isPresent`, `isEmpty` |
| `unwrap`, `expect(m)` | `orElseThrow()`, `orElseThrow(() -> new RuntimeException(m))` |
| `unwrap_or`, `unwrap_or_else`, `unwrap_or_default` | `orElse`, `orElseGet`, `orElse(<default>)` |
| `map`, `and_then`, `filter`, `or`, `or_else` | `map`, `flatMap`, `filter`, `or(() -> …)`, `or` |
| `ok_or(e)` | `o.<Result<T, E>>map(Ok::new).orElse(new Err<>(e))` |
| `iter`, `copied`, `cloned`, `as_ref`, `as_mut`, `as_deref` | `stream()`, or dropped |
| `take()` on a place | `var t = p; p = Optional.empty();` |

### 9.3 Panics, `Result`, `?`

- A panic is `throw new RuntimeException(msg)`. Because `throw` is a statement, a panic in expression position such as a `match` arm becomes `-> throw new RuntimeException(…)`, which Java allows inside switch arms.
- **The Java entry point** is written inline on the file's last line:
  ```java
  static String[] ARGS; public static void main(String[] args) { ARGS = args; try { main(); } catch (RuntimeException e) { System.out.flush(); System.err.println("thread 'main' panicked:\n" + e.getMessage()); System.exit(101); } } }
  ```
  Std exceptions (`NoSuchElementException` from `orElseThrow`, `IndexOutOfBoundsException`, `ArithmeticException`) keep their Java messages. Only the exit code, 101, is guaranteed (§1).
- **`main` returning `Result`.** The call inside the entry point becomes `if (main() instanceof Err<?, ?>(var e)) { System.err.println("Error: " + <debug of e>); System.exit(1); }`. `std::process::exit(n)` → `System.exit(n)`.
- **`?` operator.** Java can't return early from inside an expression, so each `?` operand is hoisted into a statement before its use, on the same line. A switch *statement* is allowed to `return`:
  ```java
  Config cfg; switch (read_config(path)) { case Ok(var v) -> cfg = v; case Err(var e) -> { return new Err<>(e); } }
  int n; switch (parse_num(s)) { case Ok(var v) -> n = v; case Err(var e) -> { return new Err<>(MyErr.from(e)); } }   // with From conversion
  if (opt.isEmpty()) return Optional.empty(); var v = opt.get();                                                       // Option ?
  ```
  - Several `?` in one expression are hoisted left to right, which keeps Rust's evaluation order.
  - A `?` in a loop condition turns the loop into `while (true) { …; if (!cond) break; … }`.
  - A closure containing `?` uses a block lambda with the same lowering.
  - There's no exception and no helper involved.

---

## 10. Formatting and printing

The format string is compiled at transpile time into plain Java:

| Rust | Java |
|---|---|
| `println!("x = {}, y = {}", x, y)` (Display of ints, strings, chars, bools) | `System.out.println("x = " + x + ", y = " + y);` |
| `{}` of `f32`/`f64` | `BigDecimal.valueOf(f).stripTrailingZeros().toPlainString()` (`new BigDecimal(String.valueOf(f))` for `f32`). Rust prints `1.0` as `1` and never uses exponent notation. `-0.0` prints `0` and NaN/inf throw (accepted). |
| `{}` of a `Display` user type | concatenated directly (its `toString()`) |
| `{:?}` | a type-directed inline expression (below) |
| `{:#?}` | Phase C (a diagnostic until then) |
| width, precision, alignment, `+`, `0`, `x`, `X`, `o`, `b` | `System.out.printf("%5d\n", n)` / `String.format`. Alignment is type-aware: Rust left-aligns strings by default and right-aligns numbers. `{:>8}` → `%8s`, `{:<8}` → `%-8s`, `{:08.3}` → `%08.3f`, `{:x}` → `%x`, `{:b}` → `Integer.toBinaryString`. |
| dynamic `{:w$}` / `{:.*}` | a format string built at runtime: `String.format("%" + w + "s", s)` |
| `^` centering | `" ".repeat(l) + t + " ".repeat(r)`, with `t`, `l` and `r` hoisted. Rust puts the odd space on the right. |
| `{:e}` | Phase C |
| `print!`, `eprintln!`, `eprint!` | `System.out.print`, `System.err.println`, `System.err.print` |
| `format!` | the same expression without printing |
| `write!(f, …)`, `writeln!` | `f.append(…)` |

- Inline arguments `{name}` read the variable directly. Positional `{0}` and named `{x}` arguments are resolved at transpile time.
- Known small gap: Java's `%.Nf` rounds exact binary ties half-up (`0.125` → `0.13`), while Rust rounds half-to-even (`0.12`). This is accepted.

### 10.1 `{:?}` without helpers

The transpiler knows the static type, so it builds the Debug string inline. Nested types compose recursively.

| Type | Java expression |
|---|---|
| integers, `bool`, `List` of those | `x`, or `v.toString()` (Java's `[1, 2, 3]` is the same as Rust's) |
| `String`/`&str`, `char` | `debug_str(s, '"')`, `debug_str(String.valueOf(c), '\'')`: one static helper per file, on the trailer line, with Rust's exact escaping |
| floats | `debug_float(f)`: per-file static helper; Display plus a forced `.0`, exponent form outside 1e-4..1e16 |
| `List<T>` | `v.stream().map(x -> <debug x>).collect(Collectors.joining(", ", "[", "]"))` |
| `HashSet`/`BTreeSet` | the same, with `"{", "}"` |
| `HashMap`/`BTreeMap` | `m.entrySet().stream().map(e -> <debug k> + ": " + <debug v>).collect(Collectors.joining(", ", "{", "}"))` |
| `Optional<T>` | `o.map(x -> "Some(" + <debug x> + ")").orElse("None")` |
| pair (`Map.Entry`) | `"(" + <debug k> + ", " + <debug v> + ")"` |
| user types, `Result`, `TupleN` | their generated `debug()` method (§7.2, §13) |

---

## 11. Iterators → streams

Iterator chains become `java.util.stream` pipelines. Both are lazy and process one element at a time, so side effects inside `map`/`filter` happen in the same order as in Rust.

| Rust | Java |
|---|---|
| `v.iter()`, `into_iter()`, `iter_mut()` | `v.stream()` |
| `a..b`, `a..=b` | `IntStream.range(a, b)`, `IntStream.rangeClosed(a, b)` (`.boxed()` when mapped to objects) |
| `.map`, `.filter`, `.flat_map`, `.skip`, `.take`, `.take_while`, `.skip_while`, `.inspect` | `.map`, `.filter`, `.flatMap`, `.skip`, `.limit`, `.takeWhile`, `.dropWhile`, `.peek` |
| `.filter_map(f)` | `.map(f).flatMap(Optional::stream)` |
| `.flatten()` on Options | `.flatMap(Optional::stream)` |
| `.flatten()` on collections | `.flatMap(List::stream)` |
| `.copied()`, `.cloned()` | dropped (or a clone for mutable structs) |
| `.enumerate()` | `IntStream.range(0, v.size()).mapToObj(i -> Map.entry(i, v.get(i)))` on lists; on other streams, a hoisted `var i = new AtomicInteger();` then `.map(x -> Map.entry(i.getAndIncrement(), x))` |
| `.zip(other)` | `IntStream.range(0, Math.min(a.size(), b.size())).mapToObj(i -> Map.entry(a.get(i), b.get(i)))`. Sources that aren't lists are collected with `.toList()` first. |
| `.rev()` | `v.reversed().stream()` (Java 21 `List.reversed`); on ranges, a descending `IntStream.iterate` |
| `.step_by(k)` on ranges | `IntStream.iterate(a, i -> i < b, i -> i + k)` |
| `.chain(o)` | `Stream.concat(a, o)` |
| `.windows(k)`, `.chunks(k)` | `IntStream.rangeClosed(0, n - k).mapToObj(i -> v.subList(i, i + k))`, and the analogous form for chunks |
| `.count()` | `(int) s.count()` |
| `.sum()` | `.mapToInt(x -> x).sum()` for integers. Floats use `.reduce(0.0, Double::sum)`: `DoubleStream.sum()` uses compensated summation and can differ from Rust's plain left fold. |
| `.product()` | `.reduce(1, (a, b) -> a * b)` |
| `.min()`, `.min_by_key(f)` | `.min(Comparator.naturalOrder())`, `.min(Comparator.comparing(f))`. Both return the first minimum, like Rust. |
| `.max()`, `.max_by_key(f)`, `.max_by(c)` | Rust returns the **last** maximum and Java returns the first, so these become `.reduce((a, b) -> cmp(a, b) > 0 ? a : b)`. Plain `.max()` on primitives stays `.max(...)` because equal values are indistinguishable. |
| `.fold(init, f)` | `.reduce(init, f, (a, b) -> a)` |
| `.any`, `.all`, `.find` | `.anyMatch`, `.allMatch`, `.filter(p).findFirst()` |
| `.position(p)` | `IntStream.range(0, v.size()).filter(i -> p(v.get(i))).boxed().findFirst()` |
| `.last()`, `.nth(n)` | `.reduce((a, b) -> b)`, `.skip(n).findFirst()` |
| `.for_each(f)` | `.forEach(f)` |
| `.collect::<Vec<_>>()` | `.toList()` if the result is never mutated (not bound `mut` and not moved into something mutable); otherwise `.collect(Collectors.toCollection(ArrayList::new))` |
| `.collect::<String>()` | `.map(String::valueOf).collect(Collectors.joining())` |
| `.collect::<HashSet<_>>()`, `BTreeSet` | `Collectors.toCollection(HashSet::new)` / `TreeSet::new` |
| `.collect::<HashMap<_, _>>()` | `Collectors.toMap(Map.Entry::getKey, Map.Entry::getValue, (a, b) -> b, HashMap::new)`. Later keys win, as in Rust. |
| `.collect::<Result<Vec<_>, _>>()` | lowered to statements on the same line: `List<T> out = new ArrayList<>(); Result<List<T>, E> r = new Ok<>(out); for (var x : (Iterable<Result<T, E>>) s::iterator) { if (x instanceof Err<T, E>(var e)) { r = new Err<>(e); break; } out.add(((Ok<T, E>) x).value()); }` |
| `v.join(sep)`, `concat()` | `String.join(sep, v)` |
| an iterator held in a variable (`let mut it = …; it.next()`) | `var it = <stream>.iterator();`, with `it.next()` → `(it.hasNext() ? Optional.of(it.next()) : Optional.empty())` |
| `.peekable()`, `peek()`, `next_if` | collect the source once into a hoisted list `xs`, then `var it = xs.listIterator();`. `peek()` → `(it.hasNext() ? Optional.of(xs.get(it.nextIndex())) : Optional.empty())`. This covers the common case of a tokenizer over `s.chars()`. |

### 11.1 Collections

| Rust | Java |
|---|---|
| `Vec::new()`, `vec![a, b]`, `vec![x; n]` | `new ArrayList<>()`, `new ArrayList<>(List.of(a, b))`, `new ArrayList<>(Collections.nCopies(n, x))` (fresh copies for mutable `x`) |
| `[0; 3]`, `[1, 2, 3]`, `[[0; 3]; 3]` | the same `ArrayList` forms. 2-D arrays become nested lists with an independent copy per row. |
| `v.push(x)`, `v.pop()`, `v.len()`, `v[i]`, `v[i] = x` | `add`, `v.isEmpty() ? Optional.empty() : Optional.of(v.removeLast())`, `size()`, `get(i)`, `set(i, x)` |
| `v.get(i)`, `first`, `last` | `i < v.size() ? Optional.of(v.get(i)) : Optional.empty()`, `v.stream().findFirst()`, `v.isEmpty() ? Optional.empty() : Optional.of(v.getLast())` |
| `insert`, `remove`, `swap`, `clear`, `truncate`, `extend`, `retain`, `dedup` | `add(i, x)`, `remove(i)`, `Collections.swap`, `clear`, `subList(n, size).clear()`, `addAll`, `removeIf(x -> !p)`, `for (int i = v.size() - 1; i > 0; i--) if (v.get(i).equals(v.get(i - 1))) v.remove(i);` |
| `contains`, `sort`, `sort_by`, `sort_by_key`, `sort_unstable`, `reverse` | `contains`, `v.sort(null)`, `v.sort(c)`, `v.sort(Comparator.comparing(f))`, `v.sort(null)`, `Collections.reverse` |
| `binary_search` | `Collections.binarySearch` mapped to `Result` (a negative result `-(i+1)` → `Err(i)`) |
| `&v[a..b]`, `split_at` | `v.subList(a, b)`, two `subList` calls |
| `m.insert(k, v)`, `m.get(&k)`, `m.remove(&k)` | `put`, `Optional.ofNullable(m.get(k))`, `Optional.ofNullable(m.remove(k))`. When the result is unused, the bare call is emitted. |
| `contains_key`, `keys`, `values`, `len`, `iter()` | `containsKey`, `keySet().stream()`, `values().stream()`, `size()`, `entrySet().stream()` (entries are already the pair type) |
| `*m.entry(k).or_insert(0) += 1` | `m.merge(k, 1, Integer::sum);` |
| `m.entry(k).or_insert_with(Vec::new).push(x)` | `m.computeIfAbsent(k, __ -> new ArrayList<>()).add(x);` |
| `m.entry(k).or_insert(v)` | `m.putIfAbsent(k, v); m.get(k)` |
| `HashSet` `insert`, `contains`, `remove`, `union`, `intersection` | `add` (returns boolean, same as Rust), `contains`, `remove`, stream `filter`/`concat` |
| `VecDeque` `push_back`, `push_front`, `pop_front`, `pop_back`, `front`, `back` | `addLast`, `addFirst`, `Optional.ofNullable(pollFirst())`, `Optional.ofNullable(pollLast())`, `Optional.ofNullable(peekFirst())`, `Optional.ofNullable(peekLast())`. `dq[i]` → `dq.stream().skip(i).findFirst().orElseThrow()` |
| `BinaryHeap` `push`, `pop`, `peek` | `add`, `Optional.ofNullable(poll())`, `Optional.ofNullable(peek())` |

---

## 12. I/O, process, environment

| Rust | Java |
|---|---|
When a program reads stdin, one shared reader is declared on the file's last line: `static final BufferedReader STDIN = new BufferedReader(new InputStreamReader(System.in));`.

| Rust | Java |
|---|---|
| `std::env::args()` | `Stream.concat(Stream.of("<crate>"), Arrays.stream(ARGS))`. Rust includes the program name as `args[0]`; Java doesn't. |
| `io::stdin().read_line(&mut s)` | `s += STDIN.lines().findFirst().map(l -> l + "\n").orElse("");`. `findFirst` pulls exactly one line, and `lines()` throws no checked exception. When the returned byte count is used, the line goes into a temporary first. `STDIN` is a `BufferedReader` unless the file also reads stdin bytes (`read`, `bytes`, `read_to_end`, …); then it is an `InputStream` and lines are split byte-exactly. |
| `io::stdin().lines()` | `STDIN.lines()`. The usual `.map(\|l\| l.unwrap())` / `.map_while(Result::ok)` is dropped. |
| `io::stdin().read_to_string(&mut s)` | `s += STDIN.lines().map(l -> l + "\n").collect(Collectors.joining());` |
| `fs::read_to_string(p)` with `.unwrap()`/`.expect()` | `String text; try { text = Files.readString(Path.of(p)); } catch (IOException e) { throw new UncheckedIOException(e); }` (one line) |
| the same, with `?` | `… catch (IOException e) { return new Err<>(e); }` |
| the same, with the `Result` kept | `… { r = new Ok<>(…); } catch (IOException e) { r = new Err<>(e); }` |
| `fs::write(p, s)` | `Files.writeString(Path.of(p), s)`, wrapped in the same one-line `try` |
| `File::open` + `BufReader::new(f).lines()` | `Files.lines(Path.of(p))`, wrapped in the same one-line `try` |
| `std::process::exit(n)` | `System.exit(n)` |
| `Instant::now()`, `.elapsed()`, `Duration` | `System.nanoTime()`, `java.time.Duration` |
| `io::stdout().flush()` | `System.out.flush()` |

Checked exceptions never leave the line that caused them, so generated method signatures never need `throws`.

---

## 13. Non-std code the transpiler emits

There is **no helper library**. Apart from translations of the user's own types, the only code emitted that doesn't come from Java std is the **types** that Java std lacks. Each is emitted only when the program uses it:

| Emitted | When | Contents |
|---|---|---|
| `Result` (after the class; `Result.java` for multi-module crates) | the program uses `Result` | `sealed interface Result<T, E>` with records `Ok(T value)` and `Err(E error)`, plus the Rust methods that would otherwise be multi-line switches: `unwrap`, `expect`, `is_ok`, `is_err`, `ok`, `err`, `map`, `map_err`, `and_then`, `unwrap_or`, `unwrap_or_else`, `debug`. About 40 lines. |
| `record Tuple3<A, B, C>(…)` etc. | a 3+-element tuple is used | a nested record with a `debug()` method, on the module class's last line. Pairs use `Map.Entry`. |
| `@FunctionalInterface interface Fn3<…>` etc. | a closure with 3+ parameters is used | one line, on the module class's last line |
| `static String[] ARGS`, `static final BufferedReader STDIN`, the Java `main` | always / when args or stdin are used | on the last line of the file that contains `fn main` (§9.3, §12) |

---

## 14. std coverage

**Phase A (core):**
- primitives, arithmetic, `String`/`&str`, `char`
- `Vec`, `Option`, `Result`, `?`
- structs, enums, `impl`, `match`, `if let`/`while let`/let chains, loops, closures, shadowing, block expressions
- `println!`/`format!` (`{}`, `{:?}`, width, precision)
- derives (`Debug`, `Clone`, `Copy`, `PartialEq`, `Eq`)
- panics, `process::exit`, `env::args`, stdin `read_line`/`lines`

**Phase B (common std):**
- traits, generics, trait objects, operator traits, `Display` impls
- `HashMap`/`HashSet`/`BTreeMap`/`BTreeSet`/`VecDeque`/`BinaryHeap`
- the full iterator table (§11), the full `str`/`char` table (§6.2), the full format spec
- `Ord`/`Hash`/`Default` derives, `From`/`Into`/`FromStr`/`parse`
- `Rc<RefCell<T>>`, `mem::{swap, replace, take}`, `cmp::{min, max, Ordering, Reverse}`
- `fs`, multi-file modules, `macro_rules!`

**Phase C (later):**
- generic arithmetic, trait impls on foreign types used through generics
- `Drop`
- `std::thread` + `Arc<Mutex>` + `mpsc` (`Thread`, `ReentrantLock`, `LinkedBlockingQueue`)
- `#[test]` → a runnable test main

---

## 15. Macros

- **std macros** are translated from their arguments, not their expansion: `println!`, `print!`, `eprintln!`, `eprint!`, `format!`, `write!`, `writeln!`, `panic!`, `assert!`, `assert_eq!`, `assert_ne!`, `debug_assert*!`, `vec!`, `matches!`, `unreachable!`, `todo!`, `unimplemented!`, `dbg!`.
  - `assert_eq!(a, b)` → `if (!Objects.equals(a, b)) throw new RuntimeException("assertion `left == right` failed\n  left: " + … );`
  - `matches!(e, P)` → `e instanceof P` or a switch expression returning a boolean.
- **User `macro_rules!`** are expanded by the frontend, and every emitted node is tagged with the **call-site line**.

---

## 16. Diagnostics

Every unsupported construct gets a specific error. The transpiler collects all errors in a run instead of stopping at the first:

```
error[R2J012]: arithmetic on generic type parameter `T` is not supported yet
  --> src/main.rs:42:17
```

`--report` lists each construct used in the crate and whether it is supported, so you can check whether a project is worth attempting.

---

## 17. Testing

1. **Differential corpus** in `tests/cases/<name>/` (`main.rs`, plus optional `stdin.txt` and `args`). The harness runs `rustc`/`cargo run`, then `javac` + `java`, and compares stdout and the exit code exactly, and records line drift.
   - Hand-written per-feature cases come first.
   - Then light real programs: Rust by Example, Exercism Rust solutions, Advent of Code solutions.
2. **Golden files.** The generated Java for each case is committed under `tests/cases/<name>/expected/`. Readability regressions show up as diffs in review.
3. **Drift report** printed on every run, with a CI threshold (§4.4).

---

## 18. Crate layout and dependencies

```
src/
  main.rs        CLI (clap)
  frontend/      only module importing ra_ap_*; semantic façade
  lower/         items.rs, exprs.rs, patterns.rs, types.rs, macros.rs, fmt.rs, iter.rs
  std_map/       data tables: Rust std path/method → Java emission template
  jir.rs         Java IR
  passes/        shadow.rs, cells.rs, hoist.rs, idioms.rs, imports.rs
  emit.rs        line-aware printer
  diag.rs        error codes + rendering
  support/Result.java  template for the generated Result type (include_str!)
tests/           differential harness + cases/
```

Dependencies:
- `ra_ap_hir`, `ra_ap_ide_db`, `ra_ap_syntax`, `ra_ap_load-cargo`, `ra_ap_project_model` (exact-pinned)
- `clap`, `anyhow`, `thiserror`, `serde`/`serde_json` (line map, report), `itertools`, `walkdir`
- tests: `duct` (runs cargo, javac, java), `tempfile`, `which`

`std_map` is the main lever for std coverage. Supporting another std API should usually mean adding one table entry, for example:

```rust
("alloc::vec::Vec::push",   Method("add")),
("core::str::trim",         Method("strip")),
("alloc::vec::Vec::get",    Template("{0} < {self}.size() ? Optional.of({self}.get({0})) : Optional.empty()")),
```

New lowering code should only be needed for structural constructs.

---

## 19. Milestones

| M | Scope | Exit criterion |
|---|---|---|
| M0 | Load a cargo project or single file through ra_ap; line-aware printer; inline entry point; harness | hello world passes, 100% line match |
| M1 | fns, `let`, primitives, arithmetic, `if`/`while`/`loop`/range `for`, `println!` `{}`, shadowing, block expressions | 20 feature cases pass |
| M2 | structs, impls, class spans, enums + `match`, `Optional`/`Result`/`?`, `List`, `String` mutation, closures, derives, panics | Phase A complete |
| M3 | traits, generics, streams (§11), collections (§11.1), full fmt, `str`/`char`, modules, `&mut` cells | Phase B; ≥ 70% of a 100-program Exercism/AoC sample pass |
| M4 | idioms, `--report`, diagnostics polish, drift reduction, `macro_rules!`, fs | ≥ 90% sample pass, ≥ 95% exact lines |

---

## 20. Risks

| Risk | Mitigation |
|---|---|
| ra_ap API churn, or lag behind new Rust syntax | exact pins, frontend façade, deliberate upgrades |
| Inference gaps in ra (rare, mostly inside macros) | hard error with location; the corpus tracks how often it happens |
| Copy-struct aliasing | crate-wide "mutated in place" analysis plus targeted differential cases |
| Boxed `==` in generics (`Integer == Integer`) | always emit `Objects.equals` / `.equals` for non-primitive static types |
| Unsigned values above the signed range (`u32`/`u64`), `u8` math past 255 | accepted by design (§1); shows up in the diff report |
| Streams vs Rust iterator edge semantics (`max` ties, float `sum`) | handled in the §11 table; differential cases for each |
| Inline-only output gets long on heavy lines (`{:?}` of nested types, `?` chains) | accepted; long lines are fine under the line rule. If the corpus shows a pattern repeated often, revisit adding a helper. |
