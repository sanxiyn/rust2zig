# Pattern matching

How Rust's `match`, `if let`, and the patterns inside them are translated.
**Common Lisp: implemented** (`src/translate/lisp/pat.rs`), as four dispatch
forms chosen on two questions. **Zig: implemented** (`src/translate/zig/pat.rs`,
`zig/expr.rs`), as a `switch` plus two special-cased scrutinee types.
**OCaml: implemented** (`src/translate/ml/pat.rs`), as patterns mapped to
patterns.

Facts are verified against Zig 0.16.0, OCaml 5.5.0 (with dune 3), SBCL 2.6.7,
and ECL 26.5.5.

`doc/lisp.md`'s "Pattern matching" section is the Common Lisp reference and is
not repeated here; this document is the cross-backend view, the ranked list of
holes, and an argument that **guards and nested patterns are one feature rather
than two**.

## What a Rust match is

A scrutinee and a list of arms, each an optional-guarded pattern and a body.
Patterns are *structural*: they nest arbitrarily, bind at any depth, alternate
with `|`, and match literals, ranges, slices, and references.

Two properties are inherited free, and both are load-bearing:

* **rustc has checked exhaustiveness and reachability.** No backend re-derives
  either. Every "this is safe because" below eventually rests on this.
* **First match wins.** Arms are ordered, and every target preserves the order
  it is given, so nothing has to be sorted or re-checked.

## Patterns versus prongs

The three targets do not differ in spelling here; they differ in kind.

**OCaml has patterns.** `match` accepts the same structural, nesting, binding,
alternating patterns Rust has, plus `when` guards. Translation is a
*mapping* -- `translate_pat` recurses over `syn::Pat` and emits the
corresponding `Pattern`, and depth costs nothing.

**Zig and Common Lisp have dispatch forms.** A Zig `switch` prong tests one
value (or a union tag) and optionally captures the *whole* payload; a Common
Lisp `case` clause tests a value and an `etypecase` clause tests a type.
Neither can express "matches this shape, and inside it that shape". So
translation is a *flattening*:

> a pattern becomes **one test plus a list of slot reads**.

Both backends encode exactly that. Zig's `translate_match_pat` returns
`(Vec<Node>, Vec<Capture>)`, where the `Capture`s are `Field`/`Index`/`Whole`
accessors read off the prong's payload capture. Common Lisp's `translate_pat`
returns `(key, Vec<Binding>)`, where each `Binding` is a `defstruct` reader
named `struct-field`. Same idea twice, in different vocabularies.

Everything below follows from that split. Depth-1 patterns -- a variant with
ident bindings, a literal, a wildcard, an alternation of those -- flatten
exactly. Anything deeper does not, and the two flattening backends are where
the holes are.

| Pattern feature | Zig | Common Lisp | OCaml |
|---|---|---|---|
| variant, no payload | prong `.north` | `:north` key / struct type | constructor |
| variant with ident bindings | prong capture + accessors | slot readers + `declare` | constructor pattern |
| literal | prong key | `case` key | **silently `_`** (loud, see below) |
| `_` | `else` prong | `t` clause, dropping the `e` form | `_` |
| or-pattern, no bindings | `.a, .b =>` | key list, or `(or ...)` type | native (unexercised) |
| or-pattern with bindings | marker | marker | native (unexercised) |
| nested pattern | **silently flattened away** | **silently flattened away** | native |
| guard | **silently dropped** (except `Option`) | marker | `when` (unexercised) |
| binding mode (`ref`) | capture by `*` | free -- a struct is a reference | free |
| `while let`, `let else`, `@`, range, slice | marker | marker | marker |

## Common Lisp

`translate_match` picks one of four forms on two independent questions -- do the
arms dispatch by value or by type, and is there a `_`:

| | no `_` | with `_` |
|---|---|---|
| by value | `ecase` | `case` |
| by type | `etypecase` | `typecase` |

The `e` forms signal at runtime when nothing matches, which is a runtime echo of
the exhaustiveness rustc already checked; a `_` arm makes that error
unreachable, so a match carrying one drops to the plain form. `doc/lisp.md`
argues this at length, along with the `t`/`nil` key escape and why an
or-pattern needs no expansion (a `case` key designator may be a *list*, and a
type specifier composes with `or`).

Two mechanics worth having here because the other backends have counterparts:

* **The scrutinee is bound when it is not already a symbol** (`%match`, a name
  no translated Rust identifier can collide with), because a type clause's
  accessors name it again. A value dispatch reads no slots, so it needs no
  binding.
* **Bindings carry their types.** Each slot read is emitted with a `declare`
  from the field's declared type, which is what keeps SBCL checking inside arms.

A guard makes `translate_arm` return `None`, which makes the whole match a
`(todo "match")` marker. That is the right failure and the other backends should
match it.

## Zig

`translate_match` picks one of three lowerings by asking what the arms' patterns
*name* -- a proxy for the scrutinee's type that costs no `Scip::type_at`, since
a `Some` or `Ok` pattern can only appear over the matching type:

1. any arm matching a `Some`/`None` moniker -> `translate_match_option`,
2. otherwise any arm matching `Ok`/`Err` -> `translate_match_result`,
3. otherwise -> `Node::Switch`.

The two special cases exist because Zig's optional and error union are not
unions and cannot be switched on. `Option` becomes a labeled block of one `if`
per arm, each `break :blk`-ing its value, with the last arm as the block's
result (sound because rustc checked exhaustiveness). `Result` becomes
`if (r) |v| … else |e| …`. See `design/result.md`.

The general case is a `switch`: prong keys come from `translate_match_pat`, a
payload-carrying variant captures the whole payload and the arm's bindings
become `const` declarations reading fields or tuple indices off it, and a `_`
arm prints as `else` (an empty pattern list in the printer).

`switch (s.*)` on a deref'd `&self` and `|*p|` captures come from the
`match_ergonomics` pass making binding modes explicit -- see
[Where the desugar pass sits](#where-the-desugar-pass-sits).

### Zig facts

| # | Case | Result |
|---|---|---|
| 1 | `switch` on a tagged union, capturing `\|v\|` | compiles |
| 2 | `switch` on a struct | **error: switch on struct with auto layout** |
| 3 | `switch` on a `[]const u8` | **error: switch on type '[]const u8'** |
| 4 | `switch` on an int missing cases, no `else` | **error: switch must handle all possibilities** |
| 5 | `switch` on an enum covering every variant *plus* `else` | **error: unreachable else prong; all cases already handled** |

Facts 2 and 3 bound what a Zig switch can be a translation *of*: matching a
tuple or a string, both ordinary in Rust, has no switch form at all and needs
the same labeled-block lowering guards need.

Fact 5 is a live hazard, not a hypothetical: Rust permits an arm list that
covers every variant and then a `_` (rustc warns "unreachable pattern" but
compiles), and translating it faithfully produces Zig that does not compile.
Loud, so it is a nuisance rather than a bug -- but it is the one place where
*more* redundancy in the Rust input makes the output worse.

## OCaml

`translate_pat` maps `syn::Pat` to `Pattern` structurally and recursively:
constructors with a payload pattern, record patterns for struct variants
(closed or open on `..`), tuples, and idents. Variant-ness is decided by
`is_variant`, which routes core enum members (`Ok`, `Err`, `Some`, `None`)
through monikers -- `design/result.md` records why asking SCIP's `kind_at`
instead was a silent-miscompile bug.

Everything unhandled falls to `Pattern::Var("_")`, which is a catch-all rather
than a marker. That would be the worst failure mode of the three -- except that
the target catches it:

| # | Case | Result |
|---|---|---|
| 1 | two arms that both collapse to `_`, built with dune's dev profile | **`Error (warning 11 [redundant-case]): this match case is unused.`** |

Any pattern that silently degrades to `_` produces a second unreachable `_` arm
(a Rust match with only one arm and no binding does not exist in practice), so
dune's warnings-as-errors turns the silent hole into a build failure. The
translator is unsafe here and the pipeline is safe; `test_test.sh` is what
closes it. That is worth stating plainly because it is *not* a property of the
translator and it will not survive being run outside dune's dev profile.

The one pattern class this actually bites is literals: `ml/pat.rs` has no
`syn::Pat::Lit` arm, so `match n { 0 => a, _ => b }` collapses to two `_` arms.
No `ml/` fixture contains a literal pattern, so nothing exercises it today.

## Where the desugar pass sits

`match_ergonomics` (`src/desugar/match_ergonomics.rs`) runs for **all three
backends** -- it is outside the `zig` gate in `desugar/mod.rs`. When a match's
scrutinee is a `&T`/`&mut T` ident and no arm has an explicit reference
pattern, it wraps the scrutinee in `*` and annotates each binding `ref` (or
`ref mut`).

It is a pass and not a translator lowering because the result is still valid
Rust: it turns implicit Rust into explicit Rust, per `doc/desugar.md`'s rule.
The payoff differs per backend, which is the argument for it being shared:

* **Zig** needs it. The `*self` scrutinee lowers to `switch (self.*)` through
  the existing `translate_unary`, and each `ref` binding becomes a `|*p|`
  capture and `&field` accessors, so each Zig capture's type matches the Rust
  binding-mode-derived type and the deref insertion in `binary` works uniformly
  inside arms.
* **Common Lisp and OCaml** need nothing from it -- a structure is already a
  reference in both -- and are unharmed, since `translate_pat` ignores the
  binding mode and the erased `*` collapses to its operand.

A pass that one backend needs and the others tolerate is the cheap case. The
comparison worth drawing is with the guard problem below, which is the opposite:
a rewrite that *cannot* be a pass.

## The holes, ranked

### 1. Zig silently drops guards

`translate_match_arm` never reads `arm.guard`. The `Option` path handles guards
(`zig/expr.rs:316`) and the `Result` path refuses them (`zig/result.rs:115`),
but the general switch path ignores them.

```rust
match shape {
    Shape::Circle(c) if c.radius > 0 => a,
    _ => b,
}
```

emits `.circle => |c| a` and takes the first arm for every circle, radius or
not. This is the only **silent wrong answer** anywhere in match translation, and
`rust/hash`'s `Some(v) if 0 < v && v < bytes.len()` shows guards are ordinary
enough to reach it.

The immediate fix is one line -- refuse, as the `Result` path already does,
which turns a wrong answer into a `TODO` marker. Do that first, independently of
the lowering below.

### 2. Nested patterns are silently flattened away

Both flattening backends collect a binding only when the sub-pattern is a bare
ident, and *skip* anything else with no marker:

* Zig `translate_match_pat`, `Pat::Struct` / `Pat::TupleStruct`:
  `if let syn::Pat::Ident(pi) = …` -- other sub-patterns push no capture.
* Common Lisp `translate_pat`: `if let Some(name) = self.binding_name(elem)` --
  `binding_name` answers only for `Pat::Ident`.

So `Shape::Dot(Point { x, .. })` matches every `Shape::Dot`, and `x` is unbound
rather than the arm being rejected. Two failures at once: the inner test is not
performed, and the body loses a binding it needs. Losing the binding usually
surfaces as a compile error downstream (an undeclared identifier), which is what
has kept this from being noticed -- but nothing guarantees it, and an inner
pattern that binds *nothing* (`Shape::Dot(Point { x: 0, .. })`) fails silently
and completely.

Same one-line principle: a sub-pattern that is not an ident should make the arm
refuse. OCaml is unaffected.

### 3. Bindings inside an or-pattern

Rust allows `Shape::Dot(p) | Shape::Line(p, _)`, each alternative binding the
same name. Both flattening backends refuse it (a marker), and correctly: the
accessor differs per alternative (`shape-dot-v0` vs `shape-line-v0`), so one
clause cannot read one slot. OCaml handles it natively and no fixture uses it.

This is a refusal rather than a hole; it becomes reachable through the same
lowering as guards.

### 4. `&mut` scrutinee captures are `*const`

The `match_ergonomics` pass records `ref mut` for a `&mut` scrutinee, but
`translate_match_arm` ignores the mutability and always emits `|*x|` on a
deref'd const pointer. The captures should be `*T`. Recorded in the README's
Bugs section; no example exercises it, and a write through such a capture would
be a Zig compile error, so it is loud.

### 5. `while let`, and `if let` on anything but `Some`

`syn::Expr::Let` reaches `translate_expr` in both backends and lands on the
`todo("expr")` catch-all, so a `while let` is a marker in the condition. `if let`
is special-cased in both, for `Some` only -- Zig captures the optional, Common
Lisp binds the option to the name and tests the binding, which the `nil`
encoding makes exact. Any other pattern is a marker.

## Guards and nested patterns are one feature

The natural reading is that guards need a conditional and nested patterns need
recursion, so they are separate work. They are not: **both are a test that a
prong cannot express**, and both are blocked on the same missing lowering.

A Zig prong tests one tag and then commits. A guard adds a second test after the
tag; a nested pattern adds a second test *inside* it. In either case, failing
the second test must fall through to the *next arm*, and neither a Zig `switch`
prong nor a Common Lisp `case` clause has fall-through. That single missing
capability is the whole problem.

The general lowering both need is the one the `Option` path already emits: a
labeled block with one test per arm, each `break :blk`-ing its body.

```zig
blk: {
    switch (shape) { .circle => |c| if (c.radius > 0) break :blk a, else => {} }
    break :blk b;
}
```

Generalized, each arm becomes "test, bind, `break :blk` on success", and control
reaching the next statement *is* the fall-through. Nested patterns are then the
same shape with an `and`-ed condition, and or-patterns with bindings become one
test per alternative. Common Lisp's version is the same construction with
`block` and `return-from`, and its clause bodies can also carry a plain `cond`
where the tests are cheap.

So the roadmap is one item, not three:

* **Level 1 (do first, independently).** Refuse -- guards, non-ident
  sub-patterns -- so every hole above becomes a marker. Small, and it removes
  the only silent wrong answer in the feature.
* **Level 2.** Generalize `translate_match_option`'s labeled-block shape into
  the fallback lowering for any match a `switch` cannot express: guards, nested
  patterns, or-patterns with bindings, and (facts 2 and 3) tuple and string
  scrutinees. Keep the `switch` for the depth-1 case, which is nearly every arm
  in practice and is the output a human would write.
* **Level 3.** The `&mut` capture mutability, which is independent of both.

OCaml needs none of this. Guards are `when`, nested patterns are patterns, and
the only OCaml-side work is the missing `Pat::Lit` arm -- which is a literal
mapped to a constant pattern, not a lowering.

## Test

| Path | Role |
|------|------|
| `rust/direction`, `zig/direction.zig`, `lisp/direction.lisp` | Payload-free enum: `switch` on an enum with `.north` prongs, and `ecase` over keywords. Its `vertical` is the or-pattern fixture in both -- `.north, .south =>` and `((:north :south) t)` |
| `rust/geometry`, `zig/geometry.zig`, `lisp/geometry.lisp` | Data-carrying enum: tuple and struct variants, `&self` scrutinee through `match_ergonomics`, `by_ref` captures (`\|*p\|`, `&_circle.center`), and the `etypecase` with `declare`d slot readers |
| `rust/option`, `zig/option.zig`, `lisp/option.lisp` | A crate-defined `Option` that the moniker check keeps on the switch path rather than the optional path |
| `rust/calc` | The `core` `Result` match (`Ok(value)` / `Err(_)`), which becomes `if (r) \|v\| … else \|e\|` |
| `rust/hash` | The `core` `Option` match, and the only guard in any fixture -- both on the same arm (`Some(v) if 0 < v && …`), which is why the general-path guard hole has never been hit. `zig/hash.zig` is where the labeled if-chain is visible |
| `rust/div` | `if let Some`, the other half of the `Option` story, in both backends |
| `rust/regex` | Unfixtured, and the source of the pressure: literal patterns (`0`/`1`/`_` on a `Vec` length), a `char` scrutinee, and a bare-binding arm |

No fixture has a nested pattern, an or-pattern with bindings, a `while let`, a
tuple scrutinee, or a guard outside the `Option` path. Every hole in this
document is therefore unexercised, which is exactly why they should become
markers before a fixture arrives that needs them.

The cheapest fixture that would earn its keep is a guard on a data-carrying
enum arm, since it is the one case that is silently wrong today, and it is two
lines in `rust/geometry`.

## Not implemented yet

1. Guards outside the `Option` path (refuse now, lower later).
2. Nested patterns in Zig and Common Lisp.
3. Or-patterns with bindings.
4. `Pat::Lit` in the OCaml backend.
5. `while let`, `let else`, `@` bindings, range patterns, slice patterns.
6. Tuple and string scrutinees in Zig.
7. `&mut` scrutinee capture mutability.

## Not planned

* Re-deriving exhaustiveness or reachability. rustc has done it, the input is
  known-good, and every form the backends emit is chosen on the assumption --
  `ecase` signalling and Zig's `else`-less switch are echoes of the check, not
  re-runs of it.
* Decision-tree compilation. rustc and the OCaml compiler both compile matches
  into decision trees; the backends emit one test per arm in source order
  instead. The output is meant to be read, and a human writing these targets
  writes the arms in order too.
* `matches!`, and patterns in function parameters (`fn f((a, b): (i32, i32))`).
