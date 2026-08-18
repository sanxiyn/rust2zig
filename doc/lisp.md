# Common Lisp backend

Backend-specific behavior for the `lisp` target. See the top-level `README.md`
for the shared pipeline, desugaring, and SCIP integration.

**Not implemented.** There is no `src/translate/lisp`, no `cargo run -- lisp`,
and no golden-diff runner. What exists is three hand-written target files --
`lisp/gcd.lisp`, `lisp/iter.lisp`, `lisp/sum.lisp` -- that fix the shape the
backend should emit, and this document records the rules they pin down. Every
claim below was checked against SBCL 2.6.4; each fixture runs with
`sbcl --script lisp/<name>.lisp` and is silent on success.

Unlike `doc/zig.md` and `doc/ml.md`, this document therefore describes a target,
not code. Sections name the rule and the fixture that forced it.

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

### Reserved names

Some names cannot be bound as variables at all. `let t = b;` in `rust/gcd`
cannot become `(let ((t b)) ...)`, because `t` is the true constant;
SBCL rejects it at compile time. `lisp/gcd.lisp` renames the binding to `t2`,
following the `shadowing` desugar pass's existing `v` -> `v2` convention.

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

`sum` over `[i32::MAX, i32::MAX]` and `gcd(2^32, 1)` both signal `TYPE-ERROR`.
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
| `for x in 1..=5` | `(loop for x from 1 to 5 ...)` |
| `for (i, x) in xs.iter().enumerate()` | `(loop for i of-type fixnum from 0 for x across xs ...)` |

CL's `below` and `to` are exactly Rust's `..` and `..=`. Compare `zig/sum.zig`,
which has no inclusive range (`1..=5` is emitted as `1..6`) and needs an
`@intCast` shim per loop variable; and `doc/ml.md`, which maps a half-open range
to `for i = 0 to Array.length xs - 1`.

A loop variable's type declaration uses `loop`'s own `of-type` syntax, not
`declare`.

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
  type-driven it belongs in the translator, not a desugar pass.
* **Compound assignment is half-native.** `total += x` is `(incf total x)` and
  `-=` is `decf`, so the Lisp backend should *not* run the
  `compound_assignment` desugar pass -- it would produce
  `(setf total (+ total x))`. But CL has no `*=` or `<<=`, so those still need
  the rewrite. The pass is currently all-or-nothing per backend, and Lisp wants
  it for some operators and not others.
* **Vectors.** `xs[i]` is `(aref xs i)`, `xs.len()` is `(length xs)`, and an
  array literal `[1, 2, 3]` is `#(1 2 3)`.
* **`Option` is `nil`.** `None` is `nil` and `Some(x)` is `x`, which is
  idiomatic -- CL's own `position` returns exactly that. It is only sound when
  the payload can never itself be `nil`/false, so it collapses `Some(false)`,
  `Some(nil)`, and `Option<Option<T>>`. This is the same shape as
  `design/result.md`'s payload-free test and needs the same explicit statement
  of when the erasure is legal. `rust/option` will decide it.
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
  by the `==` rule; `assert!(c)` is `(assert c)`. `panic!` and the rest are
  unexplored.

## Desugar passes

Not yet fixed, but what the fixtures imply:

| Pass | Common Lisp | Why |
|---|---|---|
| `binary` | yes | reference-operand derefs, then erased |
| `compound_assignment` | **partial** | `incf`/`decf` are native; `*=` is not |
| `destructuring` | yes | Common Lisp has no tuple assignment |
| `generic` | **no** | type parameters are erased; turbofish would be noise |
| `integer_literal` | probably not | Common Lisp literals are unbounded |
| `match_ergonomics` | yes | binding modes made explicit, then erased |
| `shadowing` | **no** | Common Lisp permits shadowing as Rust does |
| `try_expression` | unknown | depends on how `Result` and `?` are represented |
| `type_alias` | yes | same reasons as the other backends |

The `compound_assignment` row is the interesting one: it is the first case where
a backend wants a pass for some operators and not others, which the current
all-or-nothing pass list cannot express.

## Not implemented yet

Everything, in the sense that no translator exists. Beyond that, these questions
are unanswered because no fixture has reached them:

* **Structs and enums.** Whether a struct is a `defstruct`, a `defclass`, or a
  plain list, and whether an enum becomes a tagged list, a `deftype` union, or a
  class hierarchy. This also decides whether the crate-level package is enough
  or whether types need their own namespace as in the OCaml backend.
* **`Option` and `Result`.** The `nil` mapping above is provisional and
  `rust/option` will test it directly. `Result` and `?` have no design at all.
* **Closures.** CL captures lexically, so this should be as easy as OCaml and
  far easier than Zig -- but `let f = |x| ...` has to choose between `flet`,
  `labels`, and a `lambda` in a `let`, and the choice interacts with recursion.
* **Strings and chars.** `design/string.md`'s question reappears: CL strings are
  character vectors, not byte vectors, so `&str` and `&[u8]` cannot share a
  representation the way they do in OCaml.
* **Drop.** CL is garbage-collected with no destructors; `unwind-protect` is the
  only scope-exit hook. `design/drop.md`'s `defer x.drop()` has no direct
  counterpart.
* **Definition order.** CL tolerates forward references between `defun`s at the
  top level, so `design/recursion.md`'s SCC ordering is probably unnecessary --
  but this is untested, and a warning-free file may still want it.
* **A golden-diff runner.** `test_lisp.sh` should become the analogue of
  `test_ml.sh` once there is a backend to diff against. Until then the check is
  `sbcl --script lisp/<name>.lisp`.
* **Portability.** Everything here was checked on SBCL only. The package-lock
  error is SBCL-specific in its wording, though shadowing is required by the
  standard; `most-positive-fixnum` is implementation-defined by design.
