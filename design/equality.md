# Equality

How Rust's `==` and `!=` are translated. **Common Lisp: implemented** in
`src/translate/lisp/equality.rs`, as a three-way type-driven choice. **Zig:
scalars, plus `std.meta.eql` for pointer-free aggregates** (level 2 below); an
aggregate the walk cannot clear keeps `==` and stays a compile error rather
than a wrong answer. **OCaml: implemented**, and needed no design, because one
polymorphic `=` covers every case Rust can present.

Unlike most documents here this one is not driven by a single fixture. It is
retrospective: the rule was arrived at while `lisp/direction.lisp` and
`lisp/iter.lisp` were being made to pass, and `doc/lisp.md` argues the
`equal`/`equalp` half of it. What this adds is the cross-backend view and one
finding that argument does not reach -- [equalp's case folding leaks through
aggregates](#the-case-folding-leaks-through-aggregates).

Facts are verified against SBCL 2.6.7, ECL 26.5.5, Zig 0.16.0, and OCaml 5.5.0,
by running the snippets. SBCL and ECL agreed on every row.

## What Rust `==` means

Three properties matter, and the third does most of the work:

* **It is `PartialEq::eq`.** A derived impl compares field-wise and recurses;
  arrays, slices, `Vec`, `String`, `Option`, and `Result` all compare by
  contents.
* **It is not reflexive on floats.** `NaN == NaN` is false, which is why
  `PartialEq` is *partial*.
* **It is homogeneous.** Both operands have the same type. Rust has no
  cross-type `==`, so no backend ever has to answer for a mixed pair.

`#[derive(PartialEq)]` itself is dropped by every backend, and nothing is lost:
all three targets can compare the translated value whether or not the Rust type
declared it. The derive is a permission Rust needs and the targets do not.

The corollary is a gap. A **hand-written `impl PartialEq`** -- one that is not
structural -- is silently ignored. `analyze` attaches trait impls to their type
like inherent ones (`src/translate/zig/mod.rs:113` special-cases only `Drop`),
so the impl's `eq` is emitted as an ordinary method and `==` still lowers
structurally, never reaching it. No fixture has one, and none of the three
backends would notice if it did.

## The three targets at a glance

| | Rust `==` | Zig | Common Lisp | OCaml |
|---|---|---|---|---|
| integers | contents | `==` | `=` | `=` |
| `&str` / `String` | contents | **compile error** | `equal` | `=` |
| `char` | contents | **no Zig type yet** | `equal` | `=` |
| arrays | contents | `std.meta.eql` | `equalp` | `=` |
| slices, `Vec` | contents | **compile error** | `equalp` | `=` |
| structs | field-wise | `std.meta.eql` if pointer-free, else **compile error** | `equalp` | `=` |
| payload-free enum | by variant | `==` | `equal` | `=` |
| data-carrying enum | variant + payload | `std.meta.eql` if pointer-free, else **compile error** | `equalp` | `=` |
| `Option<T>` | contents | `==` if `T` is scalar, else **compile error** | `equal` | `=` |
| `!=` | `!=` | `!=`, or `!std.meta.eql(...)` | `/=` or `(not ...)` | `<>` |
| type-driven? | -- | **partly** | **yes** | no |

The three targets sit at three points on one spectrum. **Zig refuses** to
compare anything structured with `==`, so the translator was able to be untyped
and still never wrong -- it just could not express half the table, and
expressing it is what made this backend type-driven at all. **OCaml accepts
everything** through one polymorphic `=`, so the translator can be untyped and
also complete. **Common Lisp accepts everything too, under three different
predicates that disagree**, so it is the one backend that has to choose between
predicates, and the one where choosing wrong is a silent wrong answer.

That is why this document is mostly about Common Lisp. Zig's choice is a
narrower one -- not *which* equality, but whether the type is one
`std.meta.eql` answers correctly at all.

## Common Lisp: three predicates, chosen by type

### The rule

> `=` for numbers, `equal` for everything scalar-ish, `equalp` for aggregates.
> Either operand being an aggregate settles it.

`src/translate/lisp/equality.rs` sorts each operand into `Number`, `Aggregate`,
`Other`, or `Unknown`, and `equality` combines the two sorts. `Number` wins over
everything, then `Aggregate`, and the rest falls to `equal`.

| Sort | Rust types | Predicate |
|---|---|---|
| `Number` | `i8`..`i128`, `u8`..`u128`, `isize`, `usize`, `f32`, `f64` | `=` |
| `Aggregate` | array, slice, `Vec`, crate struct, data-carrying enum | `equalp` |
| `Other` | `bool`, `char`, `str`, `String`, `Option`, `Result`, tuple | `equal` |
| `Unknown` | a payload-free enum, an erased `T`, anything else untypable | `equal` |

`Number` and `Aggregate` are the only sorts that change the answer; `Other` and
`Unknown` both mean `equal`, and are distinct only because `Other` is a
decision and `Unknown` is a shrug. A payload-free enum lands in `Unknown`
rather than `Other` -- `type_sort` names its variants' *structs*, which it has
none of -- and gets the right predicate anyway, since its keywords are what
`equal` compares correctly.

Call sites: `src/translate/lisp/expr.rs:92` for `==` and `!=`, and
`src/translate/lisp/mac.rs:31` for `assert_eq!`, which is
`(assert (<pred> left right))`. `!=` negates whichever predicate the same
choice picked: `/=` when it is `=`, and `(not (equal ...))` or
`(not (equalp ...))` otherwise, since Common Lisp names a negated predicate
only for numbers.

`doc/lisp.md`'s "What this forces elsewhere: a third equality" argues why all three are needed rather than just `equalp`: the blocker is
`equalp`'s case folding on strings and characters, which Rust's `==` does not
do. `=` is kept over `equalp` for numbers on loudness grounds -- `(= 1 "x")`
signals a `TYPE-ERROR` where `(equalp 1 "x")` quietly answers NIL.

### Common Lisp facts

| # | Case | `=` | `equal` | `equalp` | Rust |
|---|---|---|---|---|---|
| 1 | `"abc"` vs `"ABC"` | -- | NIL | **T** | false |
| 2 | `#\a` vs `#\A` | -- | NIL | **T** | false |
| 3 | `#(1 2)` vs `#(1 2)` | -- | **NIL** | T | true |
| 4 | two adjustable vectors, same contents | -- | **NIL** | T | true |
| 5 | two `(make-point :x 1 :y 2)` | -- | **NIL** | T | true |
| 6 | a `point` nested in a `point` | -- | NIL | T | true |
| 7 | `:north` vs `:north` | -- | T | T | true |
| 8 | `nil` vs `nil` | -- | T | T | true |
| 9 | `1` vs `1.0` | T | NIL | T | does not typecheck |
| 10 | `1` vs `"x"` | **TYPE-ERROR** | NIL | NIL | does not typecheck |
| 11 | `0.0` vs `-0.0` | T | **NIL** | T | true |
| 12 | `NaN` vs `NaN` | **FLOATING-POINT-INVALID-OPERATION** | **T** | **T** | false |

Rows 1-8 are the rule's justification: 1 and 2 rule out `equalp` for strings and
characters, 3-6 rule out `equal` for aggregates, 7 and 8 are why a payload-free
enum and an erased `Option::None` stay on `equal`.

Rows 9-12 are what the rule does *not* have to answer for, except the last.
Rust's homogeneity retires 9 and 10. Row 11 is a case where `equal` is wrong and
the rule happens to be right anyway, since floats sort as `Number`. Row 12 is
wrong under every predicate: `=` signals, and both others answer T where Rust
answers false. Floats have no backend support yet (`research/float.md` is the
plan), so this is inert -- but it is the one row where arriving at floats
without revisiting equality would produce a silent wrong answer.

Row 12 was checked on SBCL only, since constructing a NaN without tripping a
trap is implementation-specific.

### The case folding leaks through aggregates

`doc/lisp.md` retires the case-folding objection to `equalp` for the pairs the
rule sends to it, on the grounds that Rust's `==` is homogeneous: if one operand
is an aggregate the other is too, so a string is never compared against a string
under `equalp`.

**Homogeneity does not say anything about what is inside the aggregate**, and
`equalp` recurses.

| # | Case | `equalp` | Rust |
|---|---|---|---|
| 1 | two structs whose string slot differs only in case | **T** | false |
| 2 | two vectors of strings differing only in case | **T** | false |
| 3 | two structs whose char slot differs only in case | **T** | false |
| 4 | two structs, one with slot `1` and one with `1.0` | T | does not typecheck |
| 5 | a struct vs a list with the same contents | NIL | -- |

So the rule is exactly right for a struct of numbers and quietly wrong for a
struct containing a `String`, a `&str`, or a `char`. Rows 1-3 are false
positives: an assertion passes that should fail.

Nothing hits it today, because no aggregate compared by any fixture has a string
or char field. `rust/regex` is where it lands: its `Literal` carries a `c: char`,
`Primitive::Literal` wraps a `Literal`, and its one test is an `assert_eq!`
comparing two `Primitive`s -- the assertion `doc/lisp-regex.md` is working
toward. Two literals differing only in case would compare equal. Whatever fixes this should land before that assertion is trusted, not
after.

Two candidate fixes, neither implemented:

* **A generated per-type predicate.** The translator knows each struct's field
  types, so it can emit `(defun point-equal (a b) (and (= (point-x a) (point-x b)) ...))`
  and dispatch each field by the same `Sort` rule, recursively. Precise, and
  costs nothing at a call site that compares scalars. It needs a name per type
  and an emission order, and it does nothing for the erased-`T` case below.
* **One `rust-equal` helper in the package prelude.** A single function that
  dispatches on the *runtime* value -- numbers to `=`, strings to `string=`,
  vectors and structs element-wise and slot-wise, recursing. This is the only
  option that also answers the erased-`T` case, since dispatching at runtime is
  exactly what erasure leaves available. The cost is a prelude function in every
  file and output that reads less like Common Lisp than `(equalp a b)` does.

The second is the better target for the same reason `equalp` was attractive: one
name, one meaning, correct everywhere. Its "less idiomatic" cost is also
smaller than it looks -- a reader who sees `(rust-equal a b)` learns the answer
by reading one definition at the top of the file, where `(equalp a b)` looks
right and is not.

### Known gaps

* **An erased `T` gets `equal`.** `lisp/iter.lisp`'s `*e == v` compares two
  values of a type parameter, and erasure destroyed what the choice needs:
  `equal` risks a false negative if `T` is a struct, `equalp` a false positive
  if `T` is a string. `equal` is emitted and the fixture pins it. This is the
  case only the `rust-equal` helper above answers.
* **`Some(x)` sorts as `Other`, whatever `x` is.** `expr_sort` returns `Other`
  for a `Some` call and a `None` path, and `type_sort` returns `Other` for
  `Option`, both deliberately: peeling `Option<i32>` to `Number` would send
  `assert_eq!(None, div(7, 3))` to `=`, which signals on the `nil` that `None`
  erases to. The cost is that `assert_eq!(Some(p), f())` on a struct `p` gets
  `equal` and answers NIL, since `Some` also erases to its payload. Latent: the
  only `Option` comparisons in the fixtures have integer payloads, where `equal`
  is `eql` and correct. The fix is to peel the payload and sort *it*, keeping
  `Other` only when the payload is unknown or when either operand is `None`.
* **`Result` has no encoding yet**, so `Ok(...)` sorts by whatever `expr_ty`
  says and lands on `equal`. `rust/regex`'s assertion is
  `assert_eq!(parse_primitive(), Ok(Primitive::Literal(...)))` -- an aggregate
  under an `Ok` -- so this and the `Some` gap are one problem, and both are
  reached the day `Result` is encoded.
* **Floats**, row 12 above.

## Zig: `==` for scalars, `std.meta.eql` for pointer-free aggregates

`translate_binary` asks `translate_aggregate_eq` first. When the operand type
resolves to an aggregate Zig's `==` rejects -- an array, a crate struct, a
data-carrying enum -- *and* that type is transitively free of slices and
pointers, `a == b` becomes `std.meta.eql(a, b)` and `a != b` becomes
`!std.meta.eql(a, b)`. Everything else falls through to `Node::EqualEqual` as
before. `assert_eq!` is untouched and still becomes
`try std.testing.expectEqual(left, right)` (`src/translate/zig/mac.rs:25`),
which already recursed through aggregates on its own (fact 10).

The type walk is `use_meta_eql` in `src/translate/zig/ty.rs`. It needs the
field types of every crate struct and enum, which `analyze` now records in
`aggregates`, keyed by **type name** rather than by SCIP symbol: the types it
is looked up with come from `expr_type`, which parses a rendered signature and
so produces synthetic spans that resolve to no symbol. That is the same
name-for-symbol substitution `doc/ml.md`'s `expr_module` makes, with the same
exposure -- two same-named types in one crate -- and `PLAN.md` lists it as
stopgap for the same reason.

**The pointer-free condition is the whole of the correctness argument.**
`std.meta.eql` compares a slice by pointer and length, so on a type with a
`&str` or `&[T]` field it answers false for equal contents -- fact 11's failure,
reached by a different route. Refusing those types keeps them on `==`, where
they remain a compile error. Verified: a struct with a `&'static str` field
still emits `x.* == y.*` and Zig rejects it with
`operator == not allowed for type 'Named'`.

One caveat on that loudness, which applies to this whole document's "compile
error, not a wrong answer" claim: **Zig analyses lazily**, so an uncalled
function's body is never checked. The refused comparison is a compile error
only where the code is actually reached, which for fixtures means reached by a
test. It is `test_test.sh` that turns the guarantee into an observed one.

### Zig facts

| # | Case | Result |
|---|---|---|
| 1 | `==` on two structs | **error: operator == not allowed for type 'P'** |
| 2 | `==` on two `[2]i32` arrays | **error: operator == not allowed** |
| 3 | `==` on two `[]const u8` slices | **error: operator == not allowed** |
| 4 | `==` on two `union(enum)` values | **error: operator == not allowed** |
| 5 | `==` on two payload-free enums, two optional ints, `x == null` | compiles |
| 6 | `==` on two `?P` where `P` is a struct | **error: operator == not allowed** |
| 7 | `==` on two error unions | **error: operator == not allowed** |
| 8 | `std.meta.eql` on structs, arrays, unions | compiles, compares field-wise |
| 9 | `std.mem.eql(T, a, b)` on slices | compiles, compares element-wise |
| 10 | `expectEqual` on structs, arrays, unions, optionals, error unions | compiles, **recurses** |
| 11 | `expectEqual` on two slices with equal contents, different pointers | **fails**: `expected slice ptr u8@…, found u8@…` |
| 12 | `expectEqualDeep` on the same pair | passes |
| 13 | `expectEqualSlices(u8, a, b)` on the same pair | passes |

Fact 1 is why this backend can be untyped and still safe: the failure mode is a
compile error at translation-test time, not a wrong answer at runtime. It is
also why the Common Lisp `Sort` machinery has no Zig counterpart -- there is
nothing to choose between.

Facts 5 and 7 are worth reading together with `design/result.md`: `==` on two
error unions is rejected, but `expectEqual(3, r)` compares *through* one (that
document's fact 3), which is why `zig/calc.zig` asserts on `Result` values
without unwrapping.

Fact 11 is the sharp edge in the otherwise-generous fact 10. `expectEqual`
recurses into structs and arrays but compares a slice by pointer and length, so
a Rust `assert_eq!` on a type with a `&str` or `&[T]` field -- again
`rust/regex` -- would fail on identical contents. `expectEqualDeep` is the
drop-in fix (fact 12).

### Levels

* **Level 1 (implemented).** `==` and `!=` verbatim; correct for integers,
  `bool`, payload-free enums, and optionals. `char` is still out -- it has no
  Zig type yet at all (`design/string.md`).
* **Level 2 (implemented).** `std.meta.eql` for structs, arrays, and tagged
  unions whose fields transitively contain no slice or pointer.
  `zig/geometry.zig` pins it. One gap remains inside the level: an operand
  whose type does not resolve keeps `==`. The reachable case is a **generic**
  operand -- `lisp/iter.lisp`'s `*e == v` compares two values of an erased `T`,
  and the Zig backend has the same expression with no type to walk, so it stays
  `==`. That is correct today only because `rust/iter` instantiates `T` at
  `i32`.
* **Level 3: a generated `eql` method.** For a type that does contain a slice,
  `std.meta.eql` is wrong the same way `expectEqual` is (fact 11), so the type
  gets `pub fn eql(self: Self, other: Self) bool` comparing field-wise with
  `std.mem.eql` at the slice fields, and `==` lowers to `a.eql(b)`. This is what
  hand-written Zig does. It collides with the union field/method namespace bug
  the README records, since `eql` is a name the container now owns.
* **`assert_eq!` -> `expectEqualDeep`** whenever either operand's type
  transitively contains a slice. Independent of the levels, and cheaper: it is a
  one-name change at the same call site, gated on the same type walk as level 2.

## OCaml: one polymorphic `=`

`==` is `=` and `!=` is `<>`, unconditionally and untyped -- visible in
`ml/gcd/lib/lib.ml`'s `while !b <> 0`, `ml/bitset/lib/lib.ml`'s
`… land 1 lsl bit <> 0`, and `ml/iter/lib/lib.ml`'s `if e = v`. `assert_eq!` is
`assert (left = right)` (`src/translate/ml/mac.rs:28`).

Structural equality is the standard library's `compare` specialized to
equality, so it recurses through records, variants, arrays, lists, strings,
and boxed integers with no help from the translator. There is nothing here to
design, and no fixture that stresses it -- which is the point worth recording.

### OCaml facts

| # | Case | Result | Rust |
|---|---|---|---|
| 1 | records, variants, arrays, lists, options, results, strings, chars, bytes | true when contents match | agrees |
| 2 | `"abc" = "ABC"` | false | agrees -- no case folding, unlike `equalp` |
| 3 | `ref 1 = ref 1` | true | agrees with `Cell`/`RefCell` `PartialEq` |
| 4 | `Int32.of_int 1 = Int32.of_int 1` | true | agrees; boxed ints compare by value |
| 5 | `Float.nan = Float.nan` | false | **agrees** |
| 6 | `0.0 = -0.0` | true | agrees |
| 7 | `==` (physical) on two equal records | false | -- not what `==` translates to |
| 8 | a closure, or a tuple containing one | **`Invalid_argument("compare: functional value")`** | -- |

Fact 5 is the one place a target's equality matches Rust's float behavior
exactly, and by construction rather than by luck: both are IEEE comparison.
Common Lisp is the odd one out here (row 12 above).

Fact 8 is the only hazard, and it is remote. Rust closures translate to OCaml
functions, so comparing a struct with a closure field raises at runtime instead
of failing to compile. Rust would not allow it either -- a closure is not
`PartialEq`, so the derive would not exist -- which means well-typed Rust input
cannot reach it. It is listed because the failure is an exception at runtime
rather than a type error, so if a future backend construct (a captured
environment stored in a record, say) ever puts a function where Rust had data,
the symptom will be an `Invalid_argument` from an `assert` rather than anything
that names equality.

Cyclic values -- where structural equality does not terminate -- are likewise
unreachable: Rust needs `Rc` or unsafe code to build one, and neither is
translated.

## Test

| Path | Role |
|------|------|
| `lisp/iter.lisp` | The erased `T` (`equal`) beside a length comparison (`=`), in one function -- the pair that shows the choice is per-operand-type and not per-file |
| `lisp/direction.lisp` | `equal` on keywords, the payload-free enum encoding; nothing else in the fixture |
| `lisp/div.lisp` | `equal` on an erased `Option` in `assert_eq!`, including `(equal nil (div 7 3))` for `None` |
| `lisp/geometry.lisp` | Structs, compared field-wise via `=` on the readers -- so it does *not* exercise `equalp`. **Stale**: see the note below |
| `zig/geometry.zig` | `test "translate"` compares two `Point`s both ways in one test -- `std.debug.assert(std.meta.eql(p, q))` from `==`, and `expectEqual(q, p)` from `assert_eq!` -- which is what pins level 2 and keeps the two paths visibly distinct |
| `zig/direction.zig`, `zig/div.zig`, `zig/calc.zig` | `expectEqual` on enums, optionals, and error unions |
| `ml/*/test/test.ml` | `assert (… = …)` on ints, options, and results |

**That missing struct-comparison fixture is now `rust/geometry`.** Its
`test_translate` builds a second `Point { x: 4, y: 6 }` and compares the whole
struct instead of asserting on `p.x` and `p.y` separately, which is what
exercises level 2.

**`lisp/geometry.lisp` was not regenerated with it**, because the Common Lisp
translator is not in this checkout. The golden still contains the old
field-wise `(assert (= 4 (point-x p)))` form, so it keeps loading and passing
under `test_test.sh` -- which runs the `.lisp` file directly -- while no longer
matching what the translator would now emit from its own source.
`test_lisp.sh` will flag it the day both are present in one tree, and the row
above describes a file that is a snapshot rather than current output.

Regenerating it is also the moment the Lisp side of this document gets its
first `equalp`: `Sort::Aggregate` is reached by exactly this comparison, and
**no fixture emits `equalp` today**. A fixture that exposes the case-folding
*leak* still needs a struct with a string field, which no current example has.

## Not implemented yet

1. Zig level 3, and `expectEqualDeep` for slice-bearing types. Both are the
   same missing piece seen twice: a slice-bearing aggregate has no working
   equality on the Zig side at all today, under `==` or `assert_eq!`.
2. Regenerating `lisp/geometry.lisp` against its changed source, which is also
   the tree's first `equalp`.
3. The Common Lisp aggregate leak fix -- generated predicates or a `rust-equal`
   helper.
4. Peeling `Option` and `Result` payloads when sorting, in Common Lisp.
5. Floats anywhere, including the NaN row.

## Not planned

* Honoring a hand-written `impl PartialEq`. Every backend lowers `==`
  structurally; routing it to a user-written `eq` means treating an operator as
  a method call and resolving the impl, which nothing else in the translator
  needs.
* `PartialOrd` and the ordering operators beyond the direct `<`, `<=`, `>`, `>=`
  mapping each backend already emits for numbers. Common Lisp would need the
  same three-way choice again (`<` vs `string<` vs a generated comparator), and
  no fixture orders anything but numbers.
* `Eq` and `Hash`. Nothing is hashed yet; `rust/hash` computes an FNV-1a value
  and never uses it as a map key.
