# Names

What happens to every identifier on the way out: case conversion, reserved
words, shadowing, namespacing, and the names the translator has to invent for
itself. **Implemented in all three backends**; the shared helpers are
`src/translate/name.rs` and the Zig-only renaming is `src/desugar/shadowing.rs`.

`doc/lisp.md`'s "Packages and names" is the full Common Lisp account and is not
repeated here. What this adds is the comparison, and the fifth question below,
which no other document covers.

Every backend answers the same four questions -- **case, reserved words,
shadowing, namespaces** -- and then a fifth that the source cannot ask, because
it is about names the translator mints.

## 1. Case

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `snake_case` function, parameter, local | `camelCase` | `kebab-case` | unchanged |
| `CamelCase` type | unchanged | `kebab-case` (`BitSet` -> `bit-set`) | module `BitSet`, type `t` |
| `SCREAMING_SNAKE` constant | `camelCase` | `+kebab-case+` | `snake_case` |
| field | unchanged | `kebab-case` | unchanged |
| enum variant | `snake_case` tag | `enum-variant` prefix, or `:keyword` | unchanged |

Each column is the target's own convention rather than a transliteration, which
is the point: the output should look like code written in that language, and a
reader of the Zig should not be able to tell that `sumOdd` was `sum_odd`.

The Common Lisp constant is the one that is not merely convention -- see
[reserved names](#2-reserved-words-three-different-problems) below.

## 2. Reserved words: three different problems

The three backends face three genuinely different obstacles, and each answer is
the only one its target allows.

### Zig escapes

`escape_zig` wraps a reserved word in `@"…"`: `Option::and` becomes
`fn @"and"(…)` and its call sites `x.@"and"(…)`. Verified: a function so named
compiles and calls normally. Applied to function and method names.

### OCaml renames

`escape_ml` appends an underscore: `new` becomes `new_`. There is no escape
syntax, so renaming is the only option. Verified: `let new = 1` is a syntax
error, `let new_ = 1` is fine.

### Common Lisp declares, and sometimes renames

Neither of the above applies, because the collision is not with the *syntax*.
Two distinct problems:

* **Defining a name the standard owns** is a package-lock violation. Verified:
  `(defun position (x) x)` in a package that `:use`s `COMMON-LISP` signals
  `SYMBOL-PACKAGE-LOCKED-ERROR`. The fix is not a rename but a declaration --
  `(:shadow #:position)` in the `defpackage`. `src/translate/lisp/standard.rs`
  carries the 978 external symbols as a table generated from SBCL, so the check
  is data rather than a heuristic.
* **Binding a name that names a constant** is impossible at all. Verified:
  `(let ((t 3)) t)` is a `SIMPLE-PROGRAM-ERROR`. Those 62 names have to be
  renamed, and `lisp/gcd.lisp` is where it shows -- `let t = b;` becomes
  `(let ((t2 b)) …)`. Names that only denote *functions* are bindable:
  `(let ((list 3)) list)` returns `3`, verified, so only *defining* `list`
  would be a problem.

So: **Zig escapes, OCaml renames, Common Lisp declares -- and renames for a
second, narrower reason.** The Lisp case is also the only one where the
obstacle scales with the crate: one `defstruct` mints `make-X`, `X-p`,
`copy-X`, and an accessor per field, and `define_struct` checks every one of
them against the table.

## 3. Shadowing

Rust allows a binding to shadow an outer one. The targets disagree, and the
disagreement is exactly why one desugar pass exists.

| | shadowing | consequence |
|---|---|---|
| Rust | allowed | -- |
| Zig | **forbidden across all nested scopes**; sibling scopes are independent | the `shadowing` pass renames |
| Common Lisp | allowed, exactly as Rust | no pass |
| OCaml | allowed | no pass |

Verified in Zig: an inner `const x` under an outer one is
`error: local constant 'x' shadows local constant from outer scope`, while two
sibling blocks each binding `x` compile.

`rust/closure` is the smallest demonstration, and the two backends' output of
the same three lines is the whole story:

```rust
let x = 3;
let double = |x| x * 2;
```

```zig
const x: i32 = 3;
const double = struct { fn call(_: @This(), x2: i32) i32 { return x2 * 2; } }{};
```

```lisp
(let* ((x 3)
       (double (lambda (x) (* x 2))))
  …)
```

Zig renames the closure parameter to `x2`; Common Lisp leaves both named `x`,
because it may.

### How the pass works, and why it is a pass

`src/desugar/shadowing.rs` walks with a stack of scopes -- pushed on entering a
function, block, closure, `for`, `if`, or match arm -- and on each binding picks
`name`, then `name2`, `name3`, … until the name is free in *every* enclosing
scope. The choice is recorded against the binding's SCIP symbol, and a second
`visit_mut` pass rewrites every ident whose symbol was renamed. Because the
rename is keyed on the symbol rather than the text, use sites follow
automatically and unrelated same-named bindings are untouched.

It rewrites inside macro invocations too: `Apply::visit_macro_mut` re-parses the
token stream as a comma-separated expression list, rewrites it, and re-emits --
which is what keeps `assert_eq!(x, …)` consistent with the renamed binding.

It is a *desugar* pass rather than a translator step because its output is
valid Rust: renaming a binding and its uses leaves a program that still
compiles and means the same thing. That is `doc/desugar.md`'s test, and this
pass passes it. The Common Lisp renaming for unbindable constants reuses the
same `v` -> `v2` convention but happens for an unrelated reason, and the Lisp
backend skips this pass entirely.

## 4. Namespaces

Rust has many: one per `impl` block, one per enum, one per module, and separate
type and value namespaces.

| | what the target has | what the backend does |
|---|---|---|
| Zig | a container is a namespace | methods and variants need no prefix; `Point.new`, `.dot` |
| Common Lisp | one namespace for functions, another for variables, a third for blocks | prefixes: `point-translate`, `shape-dot`, `+rand32-default-inc+` |
| OCaml | modules | `module Point` with `type t`; constructors and fields are module-scoped |

Two consequences recorded elsewhere and worth collecting here:

* **Zig's union fields and methods share one namespace**, so a `Result`-shaped
  enum with `Ok`/`Err` variants produces fields `ok`/`err` that collide with
  methods of those names. The README's standing bug; `design/enum.md` has it.
* **Common Lisp's block names are a fourth namespace.** `(block continue …)` is
  legal inside a package that `:use`s `COMMON-LISP` even though `CONTINUE` is an
  external symbol, and needs no `:shadow` -- verified. That is what makes
  `design/flow.md`'s `continue` translation free.

## 5. The names the translator invents

Every backend needs names the Rust source did not supply, and each has to
choose them so they cannot collide with a translated identifier.

| | invented name | for |
|---|---|---|
| Zig | `_name` | a rebound `mut` parameter, a range-loop capture |
| Zig | `_variant` | a match arm's payload capture (`_line`, `_circle`) |
| Zig | `blk` | every labeled block |
| Common Lisp | `%match` | a bound match scrutinee |
| Common Lisp | `+name+` | every constant |
| Common Lisp | `continue` | the loop-body block |

**Common Lisp's choices are sound by construction.** A translated identifier is
kebab-case -- letters, digits, and `-` -- so a name containing `%` or `+` cannot
be produced from Rust. The earmuffs are not decoration: a `defconstant` name
cannot later be bound as a variable (the same trap that renames `t`), so
earmuffing keeps the constant namespace disjoint from every name a local could
take. The translator would otherwise be minting its own reserved words.

**Zig's are not sound.** `_x` is a perfectly good Rust identifier, so:

* `fn f(mut a: u32, _a: u32)` rebinds `a` as `_a` and collides with the
  parameter already called `_a`;
* a match arm on `Shape::Line` whose body binds `_line` collides with the
  payload capture.

Both are duplicate-name errors in Zig, so they fail loudly rather than silently
-- but they fail, and the fix is the same one `blk` needs: mint through a
generator that consults the set of names already bound.

`blk` is the worse case, and the only silent one: every labeled block uses the
single name `BLOCK_LABEL`, so a labeled block nested inside another binds its
`break :blk` to the inner one. An `Option`-match arm whose body is itself a
block expression is the shape that hits it. It is the README's bug,
`design/option.md` names it, `design/flow.md` needs it fixed before labels can
work -- three documents waiting on one label generator.

## Test

| Path | Role |
|------|------|
| `rust/closure` | Shadowing: Zig renames `x` -> `x2`, Common Lisp does not |
| `rust/gcd` | Both Lisp mechanisms at once: `(:shadow #:gcd)` for the definition, and `t` -> `t2` for the unbindable constant |
| `rust/iter`, `rust/geometry` | `(:shadow #:position)`, `(:shadow #:min #:max)` -- the standard-symbol table doing its job |
| `rust/option` | `Option::and` -> `fn @"and"` in Zig and `option-and` in Lisp: the same collision answered by escaping and by prefixing |
| `rust/random` | Constants: `defaultInc` in Zig, `+rand32-default-inc+` in Lisp |
| `rust/sum` | Plain case conversion in three directions (`sum_odd` -> `sumOdd` / `sum-odd`) |

No fixture has a Rust identifier beginning with an underscore, which is why the
Zig invented-name collisions above are latent.

## The holes, ranked

1. **`blk` is a single name.** Silent; three documents depend on fixing it.
2. **Zig's `_name` convention can collide** with a source identifier. Loud, and
   fixed by the same generator.
3. **Zig union field/method collisions** (`design/enum.md`). Loud.
4. **Nothing checks Zig type names against Zig's own declarations.** A Rust
   struct named `Type` or `Allocator` would shadow nothing today -- there is no
   prelude to collide with, since `std` is reached through the `std` binding --
   but a Rust struct named `std` would collide with the import. Untested.

## Not implemented yet

1. A unique-label generator, for `blk`.
2. Collision-free minting of `_name` and `_variant`.
3. Module-level naming of any kind -- a crate is one file, so nothing yet needs
   a path.

## Not planned

* Preserving Rust's naming conventions in the output. `sum_odd` becoming
  `sumOdd` is deliberate, and the reverse -- emitting Rust-cased names into Zig
  -- would be a worse translation even though it is a smaller diff.
* Mangling. Every name in the output is one a human might have written; if a
  collision cannot be resolved by escaping or a suffix, it should be a marker
  rather than a mangled name.
* Round-tripping. Nothing needs to recover the Rust name from the emitted one.
