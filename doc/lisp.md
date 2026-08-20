# Common Lisp backend

Backend-specific behavior for the `lisp` target. See the top-level `README.md`
for the shared pipeline, desugaring, and SCIP integration.

Implemented: `cargo run -- lisp <source-dir> <target-dir>` writes
`<name>.lisp`, `test_lisp.sh` diffs it against `lisp/<name>.lisp`, and
`test_test.sh` runs each target file under `sbcl --script` and `ecl --shell`,
which is silent on success.

Claims were checked against the latest SBCL (2.6.7) and ECL (26.5.5).
Where they differ, the claim below says which implementation it is about, and
[Portability](#portability) collects the differences.

## No Lisp AST

The other backends have a hand-written AST (`src/ast/zig.rs`) that their
printer matches on. This one has none: the translator builds `lexpr::Value`
directly and `src/print/lisp.rs` recovers the layout from each form's head
symbol, which is how Lisp indentation is defined anyway.

The one place a form is reached other than through its head is a `let` binding.
`(name value)` has no head symbol to key on -- the head is a translated
identifier -- so the printer descends into the *value*, which may well have a
rule of its own. Without that, a `lambda` bound to a name would print its whole
body on one line, since the binding above it was inlined.

The `sexp!` macro builds the literal forms readably (`sexp!((setf ,place
,value))`). A kebab-case symbol needs lexpr's escape *with a space*:
`sexp!((# "unsigned-byte" 32))`, as in `src/translate/lisp/ty.rs`. The space is
what makes it legal. Written closed up, `#"unsigned-byte"` is a single token to
Rust 2024's lexer and a hard error (`unprefixed guarded string literals are
reserved`); written open, `#` and the string are two ordinary tokens that
`sexp!` reassembles into the symbol itself. Without the escape,
`sexp!((unsigned-byte 32))` would lex as three tokens.

What no macro literal can spell is a name computed at runtime, and every name
derived from a Rust identifier is one, so applications are built with a
`call(name, args)` helper instead.

There is no unquote-splicing: `sexp!((progn ,@body))` does not compile. Splicing
runs the other way instead -- write the fragment as a literal and flatten it
into the `Vec<Value>` being accumulated, `clauses.extend(sexp!((for ,var across
,seq)).to_vec()?)` as in `src/translate/lisp/flow.rs`. `to_vec` answers `None`
on a non-list, so the `?` also rules out building a clause out of something that
was never a form.

## What the target gives

Common Lisp is the easiest of the three targets for control flow and the
hardest for names.

Rust's `break`, `continue`, and `return` all have direct counterparts, which is
the sharpest contrast with the OCaml backend. `doc/ml.md` has to lower `break`
to `raise Exit` with a `try ... with Exit -> ()` wrapper, and `return` to a
function-local `Return` exception carrying the value. Common Lisp needs neither:
`loop` establishes a block named `nil` and `defun` establishes a block named
after the function, so `break` is `(return)` and `return e` is
`(return-from <function> e)`.

Mutation is equally free. CL bindings and parameters are ordinary mutable
places, so there is no `ref` cell split (`doc/ml.md`'s `collect_refs`) and no
`_a` / `var a = _a` parameter rebinding (`zig/gcd.zig`). `let mut` is `let`, and
`a = e` is `(setf a e)`.

Integers are unbounded, which matters more than it sounds; see
[Integer types](#integer-types-and-declarations).

What it costs is names. Common Lisp has a large standard package that Rust
programs collide with constantly, and a handful of names that cannot be bound at
all.

## Packages and names

### The package

Each crate becomes one package, named after the crate, using `#:common-lisp`,
followed by `(in-package ...)`. Every `pub` item is `:export`ed -- the analogue
of the OCaml backend's namespacing, though at crate rather than type
granularity, since Common Lisp has no per-type namespace.

```lisp
(defpackage #:iter
  (:use #:common-lisp)
  (:shadow #:position)
  (:export #:position #:position2))

(in-package #:iter)
```

### Shadowing the standard package

A crate function whose name is an external symbol of `COMMON-LISP` must be
declared in `(:shadow ...)`.

This is not a rare case. Of the 84 distinct function names across the current
Rust fixtures, five collide: `eval`, `gcd`, `max`, `min`, `position`. Both
`lisp/gcd.lisp` and `lisp/iter.lisp` need a shadow.

The rule is mechanical -- for each name the crate defines, `find-symbol` it in
`:common-lisp` and shadow it when the status is `:external`. It needs no type
information, so it is a pure emission-time decision.

`find-symbol` needs a running Lisp, so `src/translate/lisp/standard.rs` carries
the 978 external symbols as a sorted table, generated from SBCL with
`do-external-symbols`. The set is fixed by the standard rather than by the
implementation, so the table is data, not a heuristic.

### Reserved names

Some names cannot be bound as variables at all. `let t = b;` in `rust/gcd`
cannot become `(let ((t b)) ...)`, because `t` is the true constant;
SBCL rejects it at compile time. `lisp/gcd.lisp` renames the binding to `t2`,
following the `shadowing` desugar pass's existing `v` -> `v2` convention.

The unbindable names are the 62 external symbols of `COMMON-LISP` that name
defined constants -- `t`, `nil`, `pi`, `most-positive-fixnum`, the `boole-*`
family, the float limits -- generated into the same table. Names that only
denote *functions* need no rename: `(let ((list 3)) list)` is legal, and only
defining `list` would violate the package lock. Verified on SBCL.

This is a *different* pass from Zig's `shadowing`, and they want opposite
things. Zig renames because it forbids shadowing that Rust permits; Lisp renames
because a specific small set of names is unbindable. Lisp permits shadowing
exactly as Rust does, so like the OCaml backend it skips the `shadowing` pass
entirely.

### Block names are a separate namespace

`(block continue ...)` is legal inside a package that `:use`s CL even though
`CONTINUE` is an external `COMMON-LISP` symbol, and it needs no `:shadow`. Block
names are neither function nor variable bindings, so package locks do not apply.
Verified; see [Control flow](#control-flow).

### Case

`snake_case` becomes `kebab-case` -- `sum_odd` -> `sum-odd`, `test_gcd` ->
`test-gcd`. This is the Lisp counterpart of Zig's `snake_to_camel` and applies
to functions, parameters, and locals alike.

## Integer types and declarations

This is the backend's one substantive semantic decision, and it goes the
opposite way from `design/integer.md`'s OCaml analysis.

### The representation needs no analysis

OCaml's `int` is 63-bit, so `design/integer.md` has to pick a representation per
Rust type for the whole crate (`int` / `int32` / `int64`) and escalate when the
crate observes a width. Common Lisp integers are unbounded, so every Rust
integer type -- including `u64`, `i128` -- is faithfully represented by a plain
Common Lisp integer.

What remains is the same question that doc asks -- where does the crate observe
the width -- but the answer is *local*. A `wrapping_mul` on `u32` emits
`(ldb (byte 32 0) (* a b))`, which returns `1` for
`0xFFFFFFFF * 0xFFFFFFFF`, matching Rust. Nothing else in the crate changes.

### Declarations are the overflow check

Because Common Lisp promotes silently to bignums, an undeclared translation
loses Rust's debug-mode overflow panic. Declaring the Rust type restores it as a
`TYPE-ERROR`:

```lisp
(declaim (ftype (function ((unsigned-byte 32) (unsigned-byte 32)) (unsigned-byte 32)) gcd))
(defun gcd (a b)
  (declare (type (unsigned-byte 32) a b))
  ...)
```

`sum` over `[i32::MAX, i32::MAX]` and `gcd(2^32, 1)` both signal `TYPE-ERROR`
-- **on SBCL**. ECL ignores these declarations entirely, so the check is an
implementation property rather than a portable one; see
[Portability](#portability).
CL's `safety` setting is then the analogue of Rust's debug/release switch: the
checks are present at default safety and gone under `(optimize (safety 0))`,
which is the caller's choice. The measured cost of the declarations on `gcd` is
negative -- about 15% *faster* over 300k calls -- but speed is a side effect,
not the reason.

### The two forms are not interchangeable

Each covers a position the other misses:

| | argument on entry | assignment to a parameter | return value |
|---|---|---|---|
| `declaim ftype` alone | checked | not checked | checked |
| inline `declare` alone | checked | checked | not covered |

So a parameter needs an inline `declare` exactly when Rust marked it `mut`,
since that is the only way it can be assigned. `gcd`'s `a` and `b` appear in
both forms and the redundancy is load-bearing; `position`'s parameters are not
`mut` and get only the `declaim`.

Locals are declared uniformly, which is mechanical and needs no analysis. A
narrower rule -- declare only where a value is *computed* rather than copied
from a literal or variable -- would keep every check that can fire and cut the
noise in test functions, but it needs the kind of expression analysis `PLAN.md`
advises against growing.

### Which type to name

| Rust | Common Lisp |
|---|---|
| `i32` | `(signed-byte 32)` |
| `u32` | `(unsigned-byte 32)` |
| `isize` / `usize` | `fixnum` |
| `Option<usize>` | `(or null fixnum)` |
| `&[T]`, `Vec<T>`, `&str` | `vector` |
| erased type parameter `T` | `t` |

`fixnum` is right for `usize` and wrong for everything else. It is
implementation-defined -- 62 bits on this SBCL, 30 on a 32-bit build -- so `u32`
fits by accident and `u64` does not fit at all. For `usize` it is honest for
`design/integer.md`'s own reason: an index's width is the target's rather than
the program's, and Common Lisp array indices are fixnums by construction.

`Option<T>` maps onto a Common Lisp union type, so a return type stays fully
described rather than degrading to `t`.

### Slices are `vector`, never `simple-vector`

`simple-vector` means `(simple-array t (*))` -- element type exactly `t` -- and
it is true of `#(1 2 3 4 5)`, which is precisely why it is a trap:

| Rust | Common Lisp | `simple-vector` | `vector` |
|---|---|---|---|
| `&[i32]` as `#(1 2 3 4 5)` | simple vector | yes | yes |
| `&[u8]` | `(simple-array (unsigned-byte 8) (*))` | no | yes |
| `&str` | `"abc"` | no | yes |
| `Vec<T>` | adjustable + fill pointer | no | yes |
| `&v[1..3]` | displaced array | no | yes |

Declaring `simple-vector` and passing a `(unsigned-byte 8)` array signals
`TYPE-ERROR` -- a declaration *rejecting a legal program*. That is the same
invariant `design/bound.md` states for Zig's comptime checks, reached here by a
different mechanism: an emitted check may fail to catch an error, but must
never reject a valid program. It appears to be a property of emitted checks
generally rather than a Zig-specific rule.

The trap has a second floor: `(vector (signed-byte 32))` is *not* a more precise
spelling for `&[i32]`, because it means "element type upgraded from
`(signed-byte 32)`" and so rejects `#(1 2 3)`, whose element type is `t`. The
general rule is contravariance -- **declare narrowly what we construct, widely
what we accept** -- and a `pub` function is callable from hand-written Lisp we
do not control.

## Structs and enums

**Implemented for the data-carrying half**, driven by `rust/geometry`
(`Shape`, with a tuple variant and a struct variant) and `rust/bitset` (a
struct with methods and an associated function). The payload-free half below
is designed but unbuilt, no example having a C-like enum.

The encoding splits on whether any variant carries data -- the same predicate
the Zig backend's `analyze` already computes as `has_data` to choose between
`enum` and `union(enum)`. The two halves look nothing alike.

A struct is the same `defstruct` a variant is, with the fields it declares:

```lisp
(defstruct point
  (x 0 :type (signed-byte 32))
  (y 0 :type (signed-byte 32)))
```

A slot default is never the value the program uses, since every construction
site passes every field, but it still has to satisfy the slot's `:type` --
hence `0` for a numeric slot, where `nil` would not do.

### Payload-free: keywords

```lisp
(deftype direction () '(member :north :east :south :west))

(declaim (ftype (function (direction) direction) opposite))
(defun opposite (d)
  (ecase d
    (:north :south)
    (:east :west)
    (:south :north)
    (:west :east)))
```

`Direction::North` is `:north` and `match` is `ecase`, pinned by
`lisp/direction.lisp`. This is strong on every axis: `equal` compares keywords,
values print readably (`:SOUTH`), the declaration is enforced, and `ecase`'s
error on fall-through is the runtime echo of the exhaustiveness rustc already
checked.

It also sidesteps [Packages and names](#packages-and-names) entirely. Keywords
live in the `KEYWORD` package, so a `Red` in two different enums cannot
collide, and none of them need `:shadow` or `:export`. That is why a
payload-free enum defines no names at all beyond its type: `analyze` skips
`define_struct` for its variants, where a data-carrying enum has to register a
constructor, a predicate, a copier, and an accessor per slot.

Being values rather than types, the variants are not in `structs`;
`variant_keyword` resolves a path against `keywords` the way `struct_named`
resolves one against `structs`, and the same two lookups drive both a path
expression and a pattern.

The one construct this encoding cannot express is `_`. `ecase` reads its keys
literally, so `t` would be the symbol `T` rather than a catch-all -- `case` with
a final `(t ...)` clause is the shape that means it, at the cost of the
exhaustiveness echo that makes `ecase` worth choosing. A wildcard arm therefore
leaves a marker rather than a wrong answer; no fixture has one, since rustc
makes `_` pointless over an enum whose variants are all listed.

### Data-carrying: one `defstruct` per variant, unioned by `deftype`

```lisp
(defstruct shape-circle (center nil :type point) (radius 0 :type (signed-byte 32)))
(deftype shape () '(or shape-dot shape-line shape-circle))

(declaim (ftype (function (shape) list) bounding-box))
(defun bounding-box (s)
  (etypecase s
    (shape-circle (let ((center (shape-circle-center s))
                        (radius (shape-circle-radius s)))
                    ...))
    ...))
```

`match` becomes `etypecase` and each arm's pattern bindings become a `let` of
slot readers, declared like any other local. `lisp/geometry.lisp` is the
fixture, and the `deftype` union is really enforced -- `(bounding-box 42)`
warns at compile time and signals `TYPE-ERROR`.

`etypecase` evaluates its key form once, but the arms name it again in their
accessors, so a scrutinee that is not already a symbol is bound first, to
`%match`. `%` cannot appear in a translated Rust identifier, so the name
cannot capture one.

Three points are forced rather than chosen:

* **Variant names carry the enum as a prefix** (`shape-dot`, `option-some`).
  Rust gives each enum its own variant namespace and Common Lisp has one. This
  answers the namespace question the OCaml backend answers with modules: the
  crate-level package is enough, and types need no namespace of their own.
* **Generics cost nothing.** `Option<T>`'s slot takes no `:type`, so the erased
  parameter simply disappears. This is the first place the untyped target is
  *easier* than both typed backends -- no comptime, no functor.
* **A unit variant inside a data-carrying enum stays a struct.**
  `Option::None` is `(make-option-none)`, not `:none`, so an arm dispatches by
  type like every other arm instead of mixing an `(eql :none)` clause into a
  type dispatch.

### What this forces elsewhere: a third equality

`equal` compares structs by identity, so two separately constructed `Point`s
are `equal`-unequal and `equalp`-equal. `assert_eq!` on a struct or enum
therefore needs `equalp`, which makes three cases where [Notes](#notes) records
two. It stays type-driven, so it stays in the translator.

Which raises the question of why not use `equalp` for everything. It is much
closer to right than it looks: `equalp` is `equal` plus case-insensitive
strings and characters, element-wise arrays, and slot-wise structures, and two
of those three are what Rust's `==` means.

| | Rust `==` | `equal` | `equalp` |
|---|---|---|---|
| `"abc"` vs `"ABC"` | false | NIL | **T** |
| `#\a` vs `#\A` | false | NIL | **T** |
| `#(1 2)` vs `#(1 2)` | true | **NIL** | T |
| two `Point { x: 1, y: 2 }` | true | **NIL** | T |
| `1` vs `1.0` | does not typecheck | NIL | T |

The blocker is the case folding alone. `(equalp "abc" "ABC")` is true where
Rust is false, and `&str` comparison is not exotic -- `rust/regex` is built
from it. The number row is *not* a reason: `equalp` conflates `1` and `1.0`,
but Rust's `==` is homogeneous, so a well-typed program never presents that
pair.

Two things follow, and the first is a defect in the rule as it stands today:

* **`equal` is already wrong for slices.** `(equal #(1 2) #(1 2))` is NIL, so
  `a == b` on two `&[i32]` would answer false. No current example compares
  slices, so it is latent rather than broken, and `equalp` is the fix.
* **`=` survives for reasons other than correctness.** `equalp` agrees with it
  on every pair Rust can produce. It is kept because `(= 1 "x")` signals
  `TYPE-ERROR` where `(equalp 1 "x")` quietly returns NIL -- the same
  make-the-check-loud reasoning as declaring integer types -- and because it is
  what a human writes.

So the rule is `=` for numbers, `equal` for strings and characters, `equalp`
for structures and vectors. The remaining case is an erased `T`, as in
`lisp/iter.lisp`'s `*e == v`, where neither answer is safe: `equal` risks a
false negative if `T` is a struct, `equalp` a false positive if `T` is a
string. Erasure destroys what the choice needs, so this one is a known
limitation rather than a decision -- the translator emits `equal`, which is
what the fixture pins.

### Methods, and the road not taken

An `impl` block is a per-type namespace Common Lisp does not have, so a method
takes the same prefix as a variant: `p.translate(3, 4)` becomes
`(point-translate p 3 4)`, and an associated function is named the same way --
`BitSet::with_capacity(16)` is `(bit-set-with-capacity 16)`, there being no
receiver to tell the two apart once both are plain functions. `Self` resolves
to the enclosing `impl` block's type, so `-> Self` declaims as `bit-set`.

That resolution is not the translator's work, though. It is the `self_type`
desugar pass, which rewrites every `Self` to the enclosing block's type before
translation, so the name never reaches emission at all -- see
[Desugar passes](#desugar-passes).

Mutation needs nothing. A `&mut self` method mutates the caller's value because
a Common Lisp structure is a reference already, so `(incf (point-x self) dx)`
is the whole of it -- compare `zig/geometry.zig`, which threads a `*Self`
pointer, and `doc/ml.md`, which needs mutable record fields.

CLOS is the real alternative. `defmethod` can specialize on `defstruct`
classes, so two types could each keep a method named `is-some`, and a `match`
over variants could become one `defmethod` per variant instead of an
`etypecase`. It is rejected for the same reason the rest of the backend is
shaped the way it is: it turns one Rust function into N definitions, so
emission stops being structural. Worth revisiting only if name collisions
become common in practice.

### Not the same question as `core`'s `Option`

None of this touches `core::option::Option`, which stays the `nil` erasure in
[Notes](#notes) -- `rust/div` needs the `if let` lowering, not this encoding. A
crate can contain both, and the moniker check is what tells them apart, which is
what that check is for.

`rust/option` is the fixture that pins the crate-defined side, and it is worth
being clear about what it does not show: defining an enum named `Option` gets
no special treatment at all. It becomes `option-some` and `option-none`
`defstruct`s under a `deftype` union, its `unwrap` an `etypecase`, exactly as
`geometry`'s `shape` does. The name is a coincidence the moniker check sees
through.

## Control flow

* `break` is `(return)`, exiting `loop`'s implicit `nil` block.
* `return e` is `(return-from <function> e)`, using the block `defun`
  establishes.
* `continue` wraps the loop body in a named block:

  ```lisp
  (loop for x across xs
        do (block continue
             (when (= (rem x 2) 0)
               (return-from continue))
             (incf total x)))
  ```

  The block is emitted **only for a loop whose body contains a `continue`**;
  `sum` and `sum2` keep a bare `do`.

  The name matters. `(block nil ...)` would capture a `break` -- a bare
  `(return)` targets the innermost `nil` block, so a `break` inside the wrapper
  would silently become a `continue`.

* `if` / `else` is `(if c a b)`; a guard with no else is `(when c ...)`.
* `while` is `(loop while c do ...)`.

## `for` loops and ranges

`loop` covers every Rust form the fixtures use, with no shims:

| Rust | Common Lisp |
|---|---|
| `for x in xs` | `(loop for x across xs ...)` |
| `for i in 0..xs.len()` | `(loop for i of-type fixnum from 0 below (length xs) ...)` |
| `for x in 1..=5` | `(loop for x of-type (signed-byte 32) from 1 to 5 ...)` |
| `for (i, x) in xs.iter().enumerate()` | `(loop for i of-type fixnum from 0 for x across xs ...)` |
| `for (x, y) in std::iter::zip(a, b)` | `(loop for x across a for y across b ...)` |

`zip` is the one that gives the most away. `loop` iterates its `for` clauses in
parallel and ends as soon as any one of them is exhausted, which is exactly
where Rust's `zip` stops -- verified on SBCL in both truncation directions --
so there is no length shim and no `min`. `lisp/dot.lisp` is the fixture, and
the arity comes from the call rather than being fixed at two.

CL's `below` and `to` are exactly Rust's `..` and `..=`. Compare `zig/sum.zig`,
which has no inclusive range (`1..=5` is emitted as `1..6`) and needs an
`@intCast` shim per loop variable; and `doc/ml.md`, which maps a half-open range
to `for i = 0 to Array.length xs - 1`.

A loop variable's type declaration uses `loop`'s own `of-type` syntax, not
`declare`, and is emitted for every range variable SCIP can type -- the same
uniform rule as for locals. `lisp/sum.lisp` originally left `for x from 1 to 5`
undeclared, which would have made the rule "declare `fixnum` counters only";
it now reads `for x of-type (signed-byte 32) from 1 to 5`, and the declaration
carries its weight: the loop signals `TYPE-ERROR` on a bound past the width.
A variable iterated with `across` is not declared, since its type is the
element type rather than the loop's.

## Notes

* **Output layout.** One self-contained file per crate, `lisp/<name>.lisp`,
  following the Zig backend rather than OCaml's dune tree -- there is no ASDF
  machinery to justify yet. `#[test]` functions become ordinary `defun`s named
  `test-*`, called at the end of the file, so `sbcl --script` runs them. They
  are not exported and carry no `declaim`, being harness rather than API.
  Whether this should become an ASDF system with a separate test system is open.
* **References are erased**, as in the OCaml backend: `&xs` at a call site
  becomes `xs`, and the derefs the desugar passes insert (`*x`, `*self`)
  collapse to their operand. This is a translator lowering, not a desugar pass,
  for the reason `doc/desugar.md` gives -- the result is not valid Rust.
* **Generics are erased.** Common Lisp is dynamically typed, so a type parameter
  disappears and a bound with it; `position<T: PartialEq>` is `(defun position
  (l v) ...)`. Neither the `generic` desugar pass nor anything resembling
  `comptime` applies. `design/bound.md`'s comptime checks have no counterpart:
  the erased `T` is declared `t`.
* **Operators.** `%` is `rem`, **not** `mod` -- Rust's `%` truncates and CL's
  `mod` floors, so they diverge on negative operands. This is the same decision
  as Zig's signed `%` -> `@rem`. `!=` is `/=` for numbers. `==` is type-driven:
  `=` for numerics, `equal` otherwise, visible in `lisp/iter.lisp` where
  `i == l.len()` is `=` but `*e == v` on an erased `T` is `equal`. Being
  type-driven it belongs in the translator, not a desugar pass. An operand the
  translator cannot type picks `equal`.
  [Structs and enums](#structs-and-enums) adds a third case,
  `equalp`, and notes that `equal` is already the wrong answer for comparing
  two slices.
* **A bit test is `logbitp`.** Rust has no bit-test operator, so it spells one
  `x & (1 << b) != 0`; CL has one, and `logbitp` gives the boolean directly
  instead of through a comparison with zero. `translate_bit_test` recognizes the
  shape ahead of the generic binary translation, taking `&` and the comparison
  in either operand order, and `== 0` as the negation. `lisp/bitset.lisp` is
  where it shows, twice -- `bit-set-contains` and `bit-set-put`'s `prev`. The
  two forms agree for every `x`, negative included, since CL reads an integer as
  two's complement of unbounded width (`(logbitp 3 -1)` is `T` on both
  implementations); `b` comes from a `usize`, so the negative index `logbitp`
  rejects cannot arise. The result satisfies the emitted `boolean` declarations
  because SBCL and ECL both return exactly `T`/`NIL`, though the standard only
  promises a generalized boolean -- an implementation returning some other true
  value would violate `(declare (type boolean prev))`.
* **Compound assignment is half-native.** `total += x` is `(incf total x)` and
  `-=` is `decf`, so the Lisp backend does *not* run the `compound_assignment`
  desugar pass -- it would produce `(setf total (+ total x))`. But CL has no
  `*=` or `<<=`. Rather than teach the pass to run for some operators and not
  others, the translator handles both halves: `+=` and `-=` become `incf` and
  `decf`, and every other compound operator folds into `setf` at the same
  place. `i += 1` drops the step, since `incf` steps by one by default.
* **Tuples are multiple values.** `(p.x, p.y, p.x, p.y)` is
  `(values (point-x p) ...)`, the return type declaims as
  `(values (signed-byte 32) ...)`, and `let (x0, y0, x1, y1) = bounding_box(s)`
  is a `multiple-value-bind` wrapping the statements that follow, exactly as a
  `let` does. This is the one translation that works in a single position:
  `values` is a calling convention rather than a value, so a tuple stored in a
  variable or passed as an argument has no translation yet. A list would work
  everywhere and read worse everywhere; the choice can be revisited when an
  example needs it.
* **A unit function returns nil explicitly.** Rust's `()` is `nil`, but the
  last form of a translated body has no reason to produce it -- `point_translate`
  ends in an `incf`, whose value is the new coordinate. The emitted trailing
  `nil` is what makes the `null` in its `declaim` true; without it SBCL signals
  a `TYPE-ERROR` on return. Test functions have no `declaim`, so they get no
  trailing `nil`.
* **Wrapping arithmetic is `ldb`.** `a.wrapping_mul(b)` is
  `(ldb (byte 64 0) (* a b))` -- the operation on unbounded integers, cut back
  to the type's width. Checked against Rust rather than reasoned about:
  `0xFFFFFFFF * 0xFFFFFFFF` masked to 32 bits is `1`, `u64::MAX + 1` masked to
  64 is `0`, and the first PCG step of `rust/random` agrees to the digit.

  The width is the receiver's, and an unresolvable receiver leaves a marker
  rather than a guess -- a wrong width is a wrong number, not a compile error.
  Two things follow. Only *unsigned* receivers translate: `ldb` yields a value's
  unsigned reading, so a signed one would turn `(-1i32).wrapping_add(0)` into
  4294967295, which needs sign extension nothing asks for yet. And `usize` is
  excluded with them, since `int_bits` will not name a width the target decides.

  The receiver's type does not come from SCIP. These methods belong to `core`,
  and the index carries an occurrence naming `num/impl#[u64]wrapping_mul().`
  with no `symbol_information` behind it, so there is no signature to read a
  return type from. It does not need one: a wrapping operation has its
  receiver's type by definition, and saying so in `expr_ty` is what keeps a
  *chain* of them typed --
  `oldstate.wrapping_mul(M).wrapping_add(self.inc)` is one expression whose
  inner call has to be typed before the outer one can be.
* **A cast is a mask, or nothing.** Common Lisp integers are unbounded, so the
  two directions are not symmetric: `x as u32` narrowing a `u64` is
  `(ldb (byte 32 0) x)`, and widening a `u8` to `u32` is the operand untouched,
  the value already *being* that integer. Zig needs a builtin either way
  (`@truncate` or `@as`), which is what makes this the shorter rule. Both widths
  must be known, so an unresolvable operand leaves a marker, as does `usize` at
  either end -- `int_bits` will not name a width the target decides, `fixnum`
  being 62 bits on this SBCL and 61 on this ECL.
* **`rotate_right` is two shifts.** The one operation with no Common Lisp
  counterpart -- there is `ash`, and no rotate -- so it expands into what it is
  made of: `(logior (ash x (- n)) (ldb (byte 32 0) (ash x (- 32 n))))`. Only the
  left shift is masked, since `x >> n` cannot leave the width it started inside.
  At `n = 0` the left shift carries the value clear of the mask and contributes
  nothing, leaving `x`. Swept against Rust over eight cases including `0` and
  `31`, identical on SBCL and ECL.

  The expansion names each operand twice, so it is restricted to operands
  without effects -- a variable or a field of one, the same admission the Zig
  backend makes for `translate_wrapping_assign` -- and anything else leaves a
  marker. A helper `defun` would evaluate once and lift the restriction, at the
  price of emitting a function the Rust source has no counterpart for; between a
  duplicated variable and an invented definition, the duplication is the smaller
  departure.

  Rust reduces the amount modulo the width where this does not, so they part
  company for `n > width`. `rust/random`'s amount is `oldstate >> 59`, hence
  0-31 by construction, which is the same reasoning `README.md` records for
  `wrapping_shl` in the Zig backend.
* **`let _ = e;` is just `e`.** A wildcard binds nothing, so what is left is the
  expression, evaluated for its effect -- and it has to be kept: `rust/random`
  discards two `rand_u32()` calls this way, and each one advances the state the
  next result depends on. Like a tuple pattern it cannot join a `let` group,
  since there is no name to bind, but unlike one it opens no scope either, so
  the statements after it stay at the same level rather than nesting:

  ```lisp
  (let ((rng (make-rand32 :state 0 :inc ...)))
    (declare (type rand32 rng))
    (rand32-rand-u32 rng)
    (setf (rand32-state rng) ...)
    (rand32-rand-u32 rng)
    rng)
  ```

  One edge is unhandled and unexercised: a block whose *last* statement is
  `let _ = e;` has type `()` in Rust, where this emits `e` and yields its value.
  A function is safe -- the trailing `nil` above already covers it -- so this
  could only show up in a block used as a value, which no fixture does.
* **Constants are earmuffed.** A `const` becomes a `defconstant`, and an
  associated one takes its type as a prefix the way a method does:
  `Rand32::DEFAULT_INC` is `+rand32-default-inc+`, a top-level `FOOBAR` is
  `+foobar+`. The earmuffs are the part that matters. A `defconstant` name
  cannot be bound as a variable on either implementation -- `(let ((c 3)) ...)`
  signals `SIMPLE-PROGRAM-ERROR` where `c` is one -- so a plain `default-inc`
  would make that name unusable for every later local, which is
  [Reserved names](#reserved-names)' trap for `t`, except minted by the
  translator and once per constant. No translated Rust identifier can contain
  `+`, so an earmuffed name is disjoint from every name a local can take and the
  collision cannot arise. Same shape of argument as keywords for a payload-free
  enum: pick a spelling the rest of the namespace cannot reach.

  No declaration accompanies a constant. The value is a literal the compiler
  already sees, and a constant cannot be assigned, so there is nothing for one
  to catch -- unlike a local, where the declaration is the overflow check.

  One limit, not yet reached: `defconstant` requires its value to be `eql` on
  re-evaluation, so a *string* constant is not safe to load twice. SBCL signals
  `DEFCONSTANT-UNEQL`, ECL accepts it. Every constant today is an integer, and a
  translated file is loaded once, so this only matters when `rust/hash` lands --
  `defparameter` is the spelling for a value that is not `eql`-comparable.
* **Vectors.** `xs[i]` is `(aref xs i)`, `xs.len()` is `(length xs)`, and an
  array literal `[1, 2, 3]` is `#(1 2 3)`.
* **`Option` is `nil`.** `None` is `nil` and `Some(x)` is `x`, which is
  idiomatic -- CL's own `position` returns exactly that. It is only sound when
  the payload can never itself be `nil`/false, so it collapses `Some(false)`,
  `Some(nil)`, and `Option<Option<T>>`. This is the same shape as
  `design/result.md`'s payload-free test and needs the same explicit statement
  of when the erasure is legal. `rust/option` will decide it.

  `lisp/div.lisp` is where the encoding pays off. `if let Some(x) = e` is a test
  and a binding at once, which is exactly what `nil` already gives: bind `x` to
  the option itself and test that binding, so `div2` is
  `(let ((x (div a b))) (if x x 0))`. The binding gets no `declare`, and that is
  deliberate -- SCIP types `x` as the payload `u32`, but the binding holds the
  *option*, and is `nil` on precisely the branch where the declaration would be
  read. Any other pattern stays a `todo` marker: a user enum tests by type,
  which is `etypecase`, not `if`. The `else`-less form falls out for free, since
  `if let` and `if` share the same `when`/`if` shaping.
* **A closure is a `lambda`, not an `flet`.** `let f = |x| ...` becomes an
  ordinary value binding and the call site becomes `(funcall f x)`, which
  `lisp/closure.lisp` pins. The three candidates were `flet`, `labels`, and a
  `lambda` in a `let`, and two of them fall away on inspection. `labels` is for
  self-reference, which a Rust closure cannot have -- it is not in scope in its
  own initializer -- so recursion, the thing that looked like the hard part,
  never arises. `flet` then loses on namespace: a Rust closure lives in the
  value namespace, and only a value can be passed, stored, or returned, so
  `flet` would serve the direct call and nothing else, needing `#'f` the moment
  the closure is an argument. It would also split the calling convention, since
  a closure-typed *parameter* arrives as a value and must be `funcall`ed no
  matter what locals do. One convention holds because the choice is type-driven:
  `is_closure_type` asks whether SCIP typed the callee `impl Fn`/`FnMut`/
  `FnOnce`, and a parameter answers exactly as a local binding does. That test
  is pure syntax over a `syn::Type` and needs no `Scip`, so it moved to
  `src/translate/ty.rs` beside `moniker.rs`'s shared table -- the Zig backend
  asks the same question to append its `.call`.

  The parameters are declared *inside* the lambda, which is not what a `defun`
  does. A `defun`'s parameter types are stated by its `declaim ftype`, so its
  inline `declare` covers only what assignment can change; a lambda has no
  `declaim`, so the inline `declare` is the only place its types can be said.
  This keeps the checking story uniform -- and the split from
  [Portability](#portability) with it: the lambda's declaration is enforced on
  SBCL and ignored on ECL, exactly as every other declaration is. The binding
  itself declares as `function`; CL's `(function (args) result)` specifier
  exists, but nothing checks it on a variable.

  `let*` needs no new rule. `mentions` walks the whole initializer including a
  lambda body, so `test-capture`'s capture of the sibling binding `a` is seen
  and the group becomes `let*`. It is conservative in the safe direction:
  `test-closure` also gets `let*`, because the test is syntactic and the
  lambda's own parameter `x` shadows an outer `x` it never refers to.
* **Division leaks a second value.** `a / b` is `(truncate a b)`, which returns
  a quotient *and* a remainder, so `div` returns `2, 0` where its `declaim` says
  one value -- verified on both implementations. Nothing observes it today:
  every context the translator emits (an argument, a `let`, `equal`) takes the
  primary value only, and the one construct that would see the second is
  `multiple-value-bind`, which is emitted for tuple destructuring -- and rustc
  will not let a scalar-returning function be destructured as a tuple. Neither
  SBCL nor ECL objects. It is recorded rather than fixed because the fix,
  `(values (truncate a b))` on every division, is noise on every line to prevent
  something Rust's types already prevent. It becomes real the moment a value is
  produced by something other than a function return -- see
  [Tuples are multiple values](#notes).
* **Blocks group bindings, then nest.** A run of *adjacent* `let` statements
  becomes one `let` form; a `let` that follows any other statement opens a new
  nesting level, because a statement cannot sit inside a binding list. So
  `rust/iter`'s test -- `let l; let v = 3; assert; let v = 6; assert` -- is one
  `let` binding `l` and `v` together, then a nested `let` for the rebound `v`.
  Declarations for a group merge into a single `declare`.

  Grouping is what keeps output flat. Emitting one `let` per Rust `let` would
  indent once per binding, where OCaml's `let ... in` chain stays visually flat.

  A group of two or more needs `let*` whenever a later initializer references a
  binding earlier in the same group, since Rust's `let` scoping is sequential.
  Plain `let` is the readable default and is correct only when the bindings are
  independent -- getting this backwards is a wrong-answer bug, not an error,
  because the reference would silently resolve to an outer binding of the same
  name. The test is syntactic (does an initializer mention a symbol bound
  earlier in the group), so it needs no type information.

  A human would write `(setf v 6)` rather than nesting for a rebinding, but that
  is only sound when the old binding is not captured, which is an analysis this
  backend does not have.
* **Macros.** `assert_eq!(a, b)` is `(assert (= a b))` or `(assert (equal a b))`
  by the `==` rule; `assert!(c)` is `(assert c)`. `panic!("msg")` is
  `(error "msg")`, which signals rather than returning -- the same end as Rust's
  unwinding panic, and it satisfies whatever the function's `declaim` promises,
  since no value is ever produced. `rust/option`'s `unwrap` is where it appears.
  `error` takes a *format control* rather than a string to print, so a `~` in
  the message would be read as a directive and is doubled on the way out.

  A `panic!` with arguments stays a marker, since `{}` has to become a
  directive -- but that is a smaller problem here than the one `README.md`
  records for Zig, where `println!("{}", x)` needs the argument's type to choose
  between an integer and a string. Common Lisp's `~a` prints either, so the
  eventual mapping needs no type information; what it needs is the parsing of
  Rust's format syntax, shared with `println!`.

## Desugar passes

`desugar` takes the backend name and runs a per-backend list. What this backend
runs, and why the rest stay off:

| Pass | Common Lisp | Why |
|---|---|---|
| `binary` | yes | reference-operand derefs, then erased |
| `compound_assignment` | no | `incf`/`decf` are native, and the rest fold in the translator |
| `generic` | no | type parameters are erased; turbofish would be noise |
| `integer_literal` | no | Common Lisp integers are unbounded, so a literal never needs its width pinned |
| `match_ergonomics` | yes | binding modes made explicit, then erased |
| `self_type` | yes | `Self` spelled out, there being no such name in the target |
| `shadowing` | no | Common Lisp permits shadowing as Rust does |

`self_type` is the first pass written *for* this backend rather than inherited,
and the first the Zig backend deliberately declines. Zig has a `Self` of its
own -- `zig/bitset.zig` emits `fn withCapacity(bits: usize) Self` against a
`const Self = @This()` -- so resolving the name away would make that output
worse, where Common Lisp has no such name and must spell the type out
everywhere.

It is also the pass with the least in it: the impl block carries its own type,
so no SCIP is consulted, and the rewrite is one ident to another. What makes it
worth having is where the resolution *was*. Doing it in the translator meant a
`RefCell<Option<String>>` set and cleared around each `impl`, and a
`resolve_self` call at each of the seven sites that read a type name -- one per
path half in `struct_named` and `const_named`, plus `variant_keyword`,
`method_name`, `const_name`, `translate_call`, and `translate_type`. Every one
of those was a place to forget, and `Self { .. }` was broken for exactly that
reason: `struct_named` had been written without it. The pass deletes the state
and all seven calls, and makes forgetting impossible, because `Self` no longer
exists by the time emission runs.

The one wrinkle is the span. `README.md`'s discipline is that a synthetic node
carries a `call_site` span so SCIP can never resolve there; this pass instead
keeps the original `Self` ident's span. That is the safe direction and the more
useful one: rust-analyzer records an occurrence of the impl's type at exactly
that range, so a query there answers with the type the name now spells, where a
`call_site` span would merely stop answering.

The passes this tree does not have -- `destructuring`, `try_expression`,
`type_alias` -- are on the other backends' side and unexamined here;
`try_expression` in particular waits on a representation for `Result` and `?`.

The `compound_assignment` row was the interesting one: it looked like the first
case where a backend wants a pass for some operators and not others, which the
all-or-nothing pass list cannot express. Doing both halves in the translator
dissolves the problem rather than solving it, and costs nothing, because the
operators CL lacks need a translator decision anyway.

## Not implemented yet

The translator covers functions, locals, control flow, arithmetic, indexing,
`len`, `assert`, structs, enums, methods, and tuples in return position.
Everything below leaves a `todo` marker rather than disappearing --
`(todo "expr")` inline, so it is loud, and `;; TODO: mod` at top level, so the
rest of the file still loads. Seven of the fifteen Rust examples translate
marker-free.

* **`equalp` as a third equality.** Structs compare by identity under `equal`,
  so `assert_eq!` on a struct answers wrongly. No current example compares two
  structs -- `geometry` compares their fields -- so this has not bitten yet.
* **`Option` and `Result`.** `None` is `nil` and `Some(x)` is `x` today, which
  `rust/iter` exercises and `rust/div` pins in a signature and an `if let` -- it
  is only sound while the payload can never itself be `nil` or false. No fixture
  tests that bound yet: `rust/option` does *not*, defining its own `Option`,
  which is a plain generic enum and takes the `defstruct` path. What is still
  missing is `match` on a `core` `Option`: it reaches the `etypecase` path,
  finds no variant structs, and leaves a marker. `Result` and `?` have no design
  at all.
* **Method calls.** Only slice `len` is recognized. `wrapping_*` has a design
  above (`ldb`) but no code, since the examples that use it -- `rust/hash`,
  `rust/random` -- need structs and constants first.
* **Strings and chars.** `design/string.md`'s question reappears: CL strings are
  character vectors, not byte vectors, so `&str` and `&[u8]` cannot share a
  representation the way they do in OCaml.
* **Slicing, casts, `println!`.** All markers today.
* **Drop.** CL is garbage-collected with no destructors; `unwind-protect` is the
  only scope-exit hook. `design/drop.md`'s `defer x.drop()` has no direct
  counterpart.
* **Portability.** See [Portability](#portability): the output runs on SBCL and
  ECL alike, but only SBCL enforces the type declarations.

## Portability

Two implementations run the fixtures: SBCL 2.6.7 and ECL 26.5.5, both wired
into `test_test.sh`. All seven target files load and pass their own tests on
both, silently. What differs is not syntax but what a declaration *means*.

### Declarations are checked on SBCL and ignored on ECL

| | SBCL | ECL |
|---|---|---|
| `declaim ftype`, argument out of range | `TYPE-ERROR` | not checked |
| `declaim ftype`, result out of range | `TYPE-ERROR` | not checked |
| inline `(declare (type ...))` | `TYPE-ERROR` | not checked |
| `deftype` union, wrong variant passed | `TYPE-ERROR` | not checked |
| `defstruct` slot `:type` at construction | `TYPE-ERROR` | `TYPE-ERROR` |
| `defstruct` slot `:type` at `setf` | `TYPE-ERROR` | not checked |

ECL is within its rights: the standard makes declarations other than `special`
advisory, and says the consequences of violating one are undefined rather than
signalled. `(optimize (safety 3))` does not change ECL's answer either -- the
column above is the same at every safety setting tested.

This costs the backend the claim in
[Declarations are the overflow check](#declarations-are-the-overflow-check).
It holds on SBCL, where `gcd(2^32, 1)` signals; on ECL the same file returns
`4294967297` and the Rust program's debug-mode panic is simply gone. The
translation stays *correct* either way -- every fixture passes on both -- but
the overflow check is an SBCL property, not a Common Lisp one.

What survives on both is the `defstruct` slot type, and only at construction.
That is a real fraction of the checking rather than none: a struct field
holding an out-of-range value is caught by `make-point` on ECL too, since slot
type checking is a different mechanism from declaration checking.

### What agrees everywhere

* **Shadowing is required, not an SBCL quirk.** Defining `gcd` without
  `(:shadow #:gcd)` is an error on both -- `SYMBOL-PACKAGE-LOCKED-ERROR` on
  SBCL, `SIMPLE-PACKAGE-ERROR` on ECL. Only the wording is implementation
  specific.
* **`t` cannot be bound** on either (`SIMPLE-PROGRAM-ERROR` both times), so the
  `t` -> `t2` rename is portable.
* **`ecase` fall-through signals on both**, which makes the payload-free enum
  the one encoding whose check survives ECL. The reason is worth stating,
  because it is the general shape of the whole portability problem: `ecase` is
  an *operator*, and an operator's behavior is specified, where a declaration's
  enforcement is not. Taking `lisp/direction.lisp` apart confirms the split --
  a `declaim ftype` naming the `member` type is enforced on SBCL and ignored on
  ECL, exactly as the table says, while the `ecase` inside `opposite` signals a
  `TYPE-ERROR` on both. Where the data-carrying encoding leans on declarations
  for its checking, this one leans on dispatch, and only the latter is portable.
* **`equal` and `equalp`** answer identically on structs, vectors, and strings,
  so the equality rule needs no per-implementation case.
* **`loop`'s parallel `for` clauses** stop with the shortest sequence on both,
  which is what the `zip` translation rests on.
* **`fixnum` is implementation-defined by design** -- 62 bits on this SBCL, 61
  on this ECL. That is why `usize` maps to `fixnum` rather than to a width:
  an index's width is the target's.

### Definition order is not a problem

`design/recursion.md`'s SCC ordering is unnecessary here, which was previously
untested. `lisp/recursion.lisp` is the check: `is-even` calls `is-odd` before
`is-odd` is defined, and `sbcl --script` loads and runs the file without a
warning, because a `defun` body is compiled against a name rather than a
definition. Items are emitted in source order.
