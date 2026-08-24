# Trait bounds (Zig)

How a Rust trait bound becomes a `comptime` check in the Zig declaration that
carries it. **Not implemented** — this document is a design only, and it is
blocked on translating `trait` items at all (`syn::Item::Trait` is in
`item_kind`'s unsupported list today).

**Scope: user-defined traits.** A bound naming a trait the crate declares, whose
content is a set of methods. Marker and operator traits from `core` —
`PartialEq`, `PartialOrd`, `Ord`, `Copy`, `Clone`, `Sized`, `Send`, `Sync` — are
out of scope for now; see [Out of scope](#out-of-scope) for why they are a
different problem and not the one worth solving first.

This is a Zig-only concern. OCaml erases type parameters and their bounds
together — see [Other backends](#other-backends).

## The mismatch

Rust checks a generic **once, at its definition**. Zig checks it **at every
instantiation**, because a generic function's body is not analyzed until it is
called — there is no way to type-check a body against an abstract `T`.

That difference is usually described as a cost, and `design/generic.md` accepts
it when it drops bounds: "Dropping a bound therefore cannot produce silently
wrong output — it can only move an error from the definition to the
instantiation, which is Zig's normal behaviour anyway."

That is true as far as it goes, and it gives up something Rust states in the
source. `fn total<T: Shape>(l: &[T])` declares a contract in its signature; the
translated `fn total(comptime T: type, l: []const T)` does not, and neither a
reader nor the compiler can recover it from the body.

## What rustc already proved

The important asymmetry is that we are not translating arbitrary generic code.
We are translating **known-good Rust**, which means rustc has already discharged
the definition-side obligation: the body of `total` is well-formed for *every*
`T` satisfying `Shape`. Nothing in it can fail for a conforming type.

So a check placed at the top of the function is not merely an earlier error. If
the check is accurate, it is the **only** place an instantiation can fail — every
possible instantiation error has been hoisted to one line that names the
contract. Zig still does the checking at instantiation time; what changes is that
the failure is attributed to the bound rather than discovered somewhere in the
body.

This is the same posture as `design/drop.md`'s "known-good Rust (rustc already
checked); we do not re-implement borrowck". We are not verifying anything. We
are transcribing a fact rustc established into a form the target can enforce.

## The rule

> A trait becomes a namespace holding a `require` predicate over types, and each
> bound becomes one `comptime` call to it at the top of the declaration that
> carries the bound.

```rust
pub trait Shape {
    fn area(&self) -> f64;
}

pub fn total<T: Shape>(l: &[T]) -> f64 { ... }
```

```zig
const Shape = struct {
    fn require(comptime T: type) void {
        switch (@typeInfo(T)) {
            .@"struct", .@"union", .@"enum", .@"opaque" => {},
            else => @compileError(@typeName(T) ++ " does not implement Shape: not a container type"),
        }
        if (!@hasDecl(T, "area")) {
            @compileError(@typeName(T) ++ " does not implement Shape: missing area");
        }
    }
};

fn total(comptime T: type, l: []const T) f64 {
    comptime Shape.require(T);
    ...
}
```

**Why the trait becomes a namespace rather than a free `requireShape`.** A trait
has no Zig counterpart as a type, but it does have a name the source wrote, and
Zig's ordinary idiom for a name that groups declarations is a `struct` used as a
namespace. Keeping the trait's own name as a declaration means the output still
*says* `Shape`, the predicate cannot collide with a function of a similar name,
and there is an obvious home for anything else a trait needs later (default
method bodies, in particular). It also means the trait item translates to
something instead of being dropped, which is the smallest possible answer to
"what does a `trait` item become".

**Why `@hasDecl` is the check.** This design assumes the shape a trait `impl`
takes: `impl Shape for Square` puts `area` on `Square` as an ordinary method,
the same as an inherent impl, since Zig has no separate place to put it. Then
"does `T` implement `Shape`" is exactly "does `T` declare the trait's methods",
which is what `@hasDecl` asks. The `@typeInfo` switch in front of it is not
decoration: `@hasDecl` is a compile error on a non-container type such as `i32`,
so the guard has to come first.

**The check is a statement, not a wrapper.** `comptime Shape.require(T);` is one
line at the top of the body, in declaration order with the bounds it came from.
Nothing else about the function changes, and a function with no bounds is
unaffected.

## Where the check goes

Four positions, from the four places Rust can attach a bound:

| Rust | Zig |
|---|---|
| generic free fn `fn total<T: Shape>` | first statement of the fn body |
| generic method's own parameter `fn and<U: Shape>` | first statement of the method body |
| generic type `struct Pair<T: Shape>` | first statement of the container function, before `return struct { ... }` |
| `impl<T: Shape> Pair<T>` | first statement of *each method* of that impl |

The last row is the subtle one, and it does not follow from
`design/generic.md`'s rule that a type's parameters are bound by the container
function. That rule is right for the *parameter* — `Pair<T>`'s `T` is bound once
by `fn Pair(comptime T: type) type`, and the `impl<T>`'s own `T` is erased. The
bound does not erase with it, because Rust permits an impl whose bounds are
**stronger than the type's**:

```rust
struct Pair<T> { ... }
impl<T: Shape> Pair<T> { fn total(&self) -> f64 { ... } }
```

`Pair<i32>` is a legal type; `Pair::<i32>::total` is not. The container function
is instantiated for `i32` regardless, so a check there would reject a legal
program — exactly what [Permissiveness](#permissiveness) forbids. The check
belongs on each method the impl provides. When the impl's bounds merely repeat
the type's, the check is redundant but harmless; deduplicating it is an
optimization, not a correctness requirement.

Bounds are read from **both** `TypeParam::bounds` and `Generics::where_clause` —
`fn total<T: Shape>` and `fn total<T>() where T: Shape` are the same declaration
and must produce the same check.

## Permissiveness

> The predicate may fail to reject an illegal instantiation. It must never
> reject a legal one.

A false rejection breaks a program that works today — a regression. A false
acceptance is only the status quo: the error surfaces later, in the body, which
is where it surfaces now. The two failure modes are not symmetric, so the
predicate errs toward accepting.

This constrains the design in a specific way: `@hasDecl` checks a *name*, not a
signature. A `T` with `fn area(self: T, scale: f64) f64` passes a check it should
fail. Tightening it with `@TypeOf(T.area)` is possible and is deliberately **not**
level 1, because a signature comparison has to get right the things that make
signatures differ without making them wrong — `self` by value versus by pointer,
a method that is itself generic, a return type mentioning `Self`. Getting any of
those wrong rejects a legal program. Name-only is the permissive choice, and the
one to start from.

It also means the honest claim in [What rustc already proved](#what-rustc-already-proved)
is weakened in proportion: the more permissive the predicate, the less it is
true that *every* instantiation error lands on the check. Both statements are
worth keeping in view at once — the check is where errors should land, and a
permissive check is why some still will not.

## What it actually catches

Worth being precise, because the obvious argument oversells it. Compare a
`Circle` with no `area` passed to `total`, with and without the check:

```
error: total.Circle does not implement Shape: missing area
    comptime Shape.require(T);
```

```
error: no field or member function named 'area' in 'total.Circle'
    for (l) |e| sum += e.area();
```

Zig's native error is already good. What the check adds:

* **Attribution.** The error names `Shape` and points at the contract; the
  native error names a use site and leaves the reader to infer which contract
  was violated. With two bounds, or a method called three levels down through
  other generic functions, that inference is the whole difficulty.
* **Bounds the body does not exercise.** This is the case Zig misses *entirely*.
  `fn f<T: Shape>(x: T) {}` — Rust rejects `f(circle)`; Zig compiles it, because
  nothing in the body ever asks for `area`. The same applies to a method reached
  only through a branch that comptime evaluation prunes. Here the check is not a
  better error, it is the only error.
* **The contract in the output.** A reader of the Zig sees what the Rust
  signature said. This is the benefit that applies to every generic function,
  including the ones that would have failed informatively anyway.

## Implementation

Sketch, once `trait` items translate:

1. Analyze — record each user trait by SCIP symbol with its method names, from
   `syn::Item::Trait`'s `TraitItem::Fn` entries.
2. Items — emit the trait as `const Name = struct { fn require(comptime T: type) void { ... } };`,
   one `@hasDecl` test per method after the `@typeInfo` guard. A trait with no
   methods emits a `require` that checks only the guard.
3. Bounds — for each generic declaration, collect `TypeParam::bounds` plus the
   matching `where` predicates, keep the ones resolving to a recorded trait
   symbol, drop the rest (see [Out of scope](#out-of-scope)), and emit one
   `comptime <Trait>.require(<param>);` per surviving bound, in source order, as
   the leading statements of the body chosen by
   [Where the check goes](#where-the-check-goes).
4. Supertraits — `trait Solid: Shape` makes `Solid.require` call `Shape.require`
   first. This falls out of the same collection step and needs no new mechanism,
   but it does need cycle protection if the frontend ever hands us a cyclic
   hierarchy.

Nothing here needs a new query. The bound is written in the source, and whether
a given type satisfies it is Zig's question, not ours — see
[Durability](#durability).

## Test

No fixture exercises this; one has to be added with the feature.

| Path | Role |
|------|------|
| `rust/bound` (proposed) | a trait with one method, two implementing types, a generic fn bounded by it, and a generic fn whose body never calls the method |

The last item is the one that matters: it is the case that fails in Rust,
compiles in Zig without the check, and fails with it. `test_test.sh` cannot
cover a *rejection*, since both suites only run programs that compile, so the
negative case needs either a comment in the fixture or a separate
compile-failure harness. That harness does not exist and is probably not worth
building for one case.

## Out of scope

### Marker and operator traits

`PartialEq`, `PartialOrd`, `Ord`, `Copy`, `Clone`, `Sized`, `Send`, `Sync` —
everything the bound might name that the crate did not declare. They stay
dropped, as `design/generic.md` describes. They are a different problem in three
ways:

* **Some are vacuous in Zig.** Every Zig value is copyable and sized, and there
  is no thread-safety marker. `Copy`, `Clone`, `Sized`, `Send`, `Sync` have
  nothing to check, and emitting a `require` for them would be noise asserting
  a tautology.
* **The rest are not method sets.** `T: PartialEq` on `i32` is not a question
  about declarations — it is a question about which types `==` accepts, so the
  predicate would be a hand-written classification over `@typeInfo` rather than
  something generated from a declaration the source contains.
* **They need a prelude, and user traits do not.** A predicate for `PartialEq`
  has to come from somewhere, and output is one `.zig` file per crate — so it
  would have to be duplicated into every file that uses it, or wait for
  multi-file output. A user trait's predicate is generated from the user's own
  `trait` item, which is already in the same file. This asymmetry is most of the
  reason to do user traits first.

And the payoff is smallest exactly there: the body of a `T: PartialEq` function
uses `==`, so Zig already errors at the use. The bounds-not-exercised case, which
is where a check earns its place, essentially does not arise for operator traits.

### Bounds with generic arguments or associated types

`T: Into<U>`, `I: Iterator<Item = T>`. `@hasDecl` cannot express either, and
associated types need a representation decision this document does not make.
Dropped like the rest.

### Bounds on types that are not type parameters

`where Pair<T>: Shape`. The check has nowhere natural to attach. Dropped.

### `dyn Trait` and trait objects

Out of scope entirely, and a much larger question than bounds — dispatch, vtable
representation, and object safety. This document covers only the static case,
where a trait exists to constrain a `comptime` type parameter. A trait that is
*only* ever used as a bound needs nothing else; one used as `dyn Shape` needs a
design that does not exist yet.

### Signature checking

`@hasDecl` by name only, for the reason given in
[Permissiveness](#permissiveness). Comparing `@TypeOf(T.area)` against the
trait's declared signature is a natural level 2.

## Durability

By `PLAN.md`'s test this is on the durable side, which is worth stating because
it is unusual for anything touching generics.

The feature **requires no new inference**. It does not ask whether `Square`
implements `Shape` — Zig answers that at instantiation. It only transcribes a
bound the source writes down syntactically. So it needs nothing from
`translate::ty::expr_type`, nothing from trait resolution, and no widening of
`desugar::generic`'s shape matcher, and none of it becomes dead code at the
frontend migration. `PLAN.md`'s generics freeze is a freeze on *reconstructing
inference*, and this is the other kind of work: deciding how the target expresses
a contract the source states.

## Other backends

OCaml needs none of this. Its `let` is inferred polymorphic and the inferred
type is at least as general as the Rust one, so a bound erases along with the
type parameter it constrains, exactly as `design/generic.md` describes.

Whether OCaml can translate a trait at all is a separate and harder question —
it has no duck typing over records, so a trait method call on a type parameter
has no obvious target — but that is trait *support*, not bounds, and it is not
this document's problem. The bound itself is the part OCaml genuinely does not
need.

## Alternatives considered

* **Keep dropping bounds, and emit them as a doc comment.** Zero risk, zero
  noise beyond a line of text, and it recovers the contract for a human reader —
  which is one of the three benefits above. Rejected as the whole answer because
  it recovers only that one: it cannot catch the unexercised-bound case, and it
  gives the compiler nothing. Worth doing *in addition*, and it is what the
  out-of-scope bounds above should get if anything.
* **Witness types.** Generate a struct implementing exactly the bound and force
  Zig to analyze the body against it in a `comptime` block. This is the only
  construction that recovers real definition-site checking — property (b), the
  body verified once — rather than instantiation-site checking with a better
  error. Rejected on output: it requires a synthetic type *and* a synthetic call
  with dummy arguments for every generic function, which is a large amount of
  code no human would write, to re-prove something rustc already proved. It is
  the right answer for a verifier and the wrong one for a transpiler.
* **`anytype` instead of `comptime T: type`.** Rejected for
  `design/generic.md`'s reasons, which are unchanged here: it cannot name the
  type. It is also strictly worse for bounds, since there is no parameter to
  attach a check to.
* **Check inside `rust2zig` and emit nothing.** The translator could verify the
  bound itself and refuse to emit. Rejected as pointless: rustc already did it,
  the fixture had to compile as Rust first, and it would put nothing in the
  output — the check exists to travel with the code, not to gate translation.
* **A flat `requireShape(T)` instead of a `Shape` namespace.** Fewer
  declarations and slightly less indirection. Rejected because it manufactures a
  name where the source had one, and it leaves a `trait` item translating to
  nothing with its predicate floating free of it.
