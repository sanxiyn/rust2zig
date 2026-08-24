# Enums

How a Rust enum is encoded. **Implemented in all three backends.** Driven by
`rust/direction` (payload-free, added because no earlier example had a C-like
enum), `rust/geometry` (tuple and struct variants), and `rust/option` (a
generic enum with methods).

This is about the *encoding* only. Dispatch over it is `design/match.md`; a
variant that carries fields is a struct and follows `design/struct.md`;
comparing two values of one is `design/equality.md`; `Result` and error sets
are `design/result.md`; and `doc/lisp.md`'s "Structs and enums" is the full
Common Lisp account, of which this repeats only what the comparison needs.

## One predicate, computed twice

Every encoding question here turns on a single property: **does any variant
carry data**. Both splitting backends compute it, from opposite sides:

```rust
// zig/mod.rs
let has_data = e.variants.iter().any(|v| !v.fields.is_empty());
// lisp/item.rs
let plain = e.variants.iter().all(|variant| variant.fields.is_empty());
```

The OCaml backend does not ask, because it has nothing to choose between.

| | payload-free | data-carrying |
|---|---|---|
| Zig | `enum { north, south }` | `union(enum) { dot: Point, … }` |
| Common Lisp | `(deftype direction () '(member :north …))` | one `defstruct` per variant + a `deftype` union |
| OCaml | `type t = North \| South` | `type t = Dot of Point.t \| …` |

```zig
const Direction = enum { north, east, south, west };

const Shape = union(enum) {
    dot: Point,
    line: struct { Point, Point },
    circle: struct { center: Point, radius: i32 },
};
```

```lisp
(deftype direction () '(member :north :east :south :west))

(defstruct shape-dot (v0 nil :type point))
(defstruct shape-line (v0 nil :type point) (v1 nil :type point))
(defstruct shape-circle (center nil :type point) (radius 0 :type (signed-byte 32)))
(deftype shape () '(or shape-dot shape-line shape-circle))
```

```ocaml
type t = North | East | South | West

type t = Dot of Point.t | Line of Point.t * Point.t | Circle of { center : Point.t; radius : int }
```

The Common Lisp pair is the one that looks least alike, and it is also the pair
that differs in *kind*: a payload-free variant is a **value** (a keyword) and a
data-carrying variant is a **type** (a structure class). Everything downstream
follows that -- `ecase` against `etypecase`, `equal` against `equalp`, and an
export list that names constructors and accessors in one case and nothing at all
in the other, keywords living in the `KEYWORD` package where no two enums can
collide.

## Why both splitting backends split

The natural objection is that the split is unnecessary: a `union(enum)` can hold
all-void payloads, and a `defstruct` per variant works whether or not the
variant has slots. One encoding could cover both, and the translator would be
simpler.

It would also be observably worse, and in both backends **the same feature is
what notices: equality.**

### Zig facts

| # | Case | Result |
|---|---|---|
| 1 | `==` on two `enum` values | compiles |
| 2 | `==` on two `union(enum)` values, payloads all void | **error: operator == not allowed for type 'U'** |
| 3 | `==` between a `union(enum)` value and an enum literal (`a == .north`) | compiles -- this is the tag comparison |
| 4 | `expectEqual` on two `union(enum)` values | compiles, recurses |
| 5 | `switch` on an all-void `union(enum)` | compiles |
| 6 | `@intFromEnum(E.north)` | compiles, `0` |
| 7 | `@as(i32, E.north)` | **error: expected type 'i32', found 'E'** |
| 8 | a `union(enum)` with a field of a struct that contains the union by value | **error: dependency loop with length 2**; the same pair with a `*Concat` field compiles |

Fact 2 against fact 1 is the argument: encoding `Direction` as a `union(enum)`
would compile and dispatch (fact 5) and would silently cost `d1 == d2`.
`rust/direction`'s test asserts on returned directions, so the loss would be
immediate.

On the Common Lisp side the same thing happens one step further out:
`(equal :north :north)` is T, while two separately constructed structures are
`equal`-unequal and need `equalp` -- so encoding a payload-free enum as
structures would drag it into the aggregate case of `design/equality.md`, with
`equalp`'s case folding in tow.

So the split is not cosmetic in either backend, and the two backends split at
the same place for the same reason, which is why one predicate is enough.

## OCaml does not split, and pays nothing

A Rust enum *is* an OCaml variant type: constructors with or without arguments,
in one declaration, compared structurally, matched natively. There is no
predicate to compute and no second encoding to maintain.

This is the third place a functional target turns out to need no design where
Zig needs levels (`design/result.md` for `Result`, `design/equality.md` for
`==`). The pattern is consistent enough to state: **the constructs Rust
inherited from ML cost nothing in ML and cost a design document in Zig.**

The gap is in coverage rather than capability -- no `ml/` fixture has a
payload-free enum. `ml/option` and `ml/geometry` are both data-carrying, so the
`North | South` row of the table above is the encoding the backend would emit
and not one any golden file pins.

## Variants: names and namespaces

Rust gives each enum its own variant namespace. What each target does about it:

| | spelling | namespace |
|---|---|---|
| Zig | `camel_to_snake`: `Dot` -> `dot` | the container -- `Shape.dot`, and `.dot` where the type is known |
| Common Lisp | data-carrying: `shape-dot`; payload-free: `:north` | one namespace, so the enum name is a prefix -- except keywords, which need none |
| OCaml | unchanged: `Dot` | the module -- `Shape.Dot` |

A **unit variant inside a data-carrying enum** is the case where the three
diverge most, and each answer is deliberate:

* Common Lisp keeps it a `defstruct` with no slots (`(defstruct option-none)`),
  so that every arm of a dispatch is a type and none is an `(eql :none)` clause
  mixed into an `etypecase`.
* Zig emits a void-payload member (`none,` in `union(enum) { some: T, none }`),
  which is Zig's own spelling for the same thing.
* OCaml emits a constructor with no argument, which is what it already is.

## The enums that are never encoded

Three enums bypass all of the above, dispatched by moniker rather than by
shape:

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `core::option::Option` | `?T` | erased to `nil` | `option` |
| `core::result::Result` | `E!T` when `E` is payload-free | not yet encoded | `result` |
| a crate error type | an error set | -- | an ordinary variant |

`design/result.md` covers the second and third rows. What belongs here is the
first: `Option` is the one enum whose *encoding* differs from every other enum
in the crate, and the moniker check is what keeps that from leaking. A
crate-defined enum that happens to be named `Option` gets no special treatment
at all -- `rust/option` is the fixture that pins it, and it becomes
`option-some` / `option-none` structs in Common Lisp and a `union(enum)` in Zig
exactly as any other generic enum would.

## Generic enums

`rust/option`'s `Option<T>` is the only generic fixture in the tree:

* **Zig**: a function returning a type -- `fn Option(comptime T: type) type {
  return union(enum) { … } }` -- with methods inside and further comptime
  parameters after `self` (`fn @"and"(self: Self, comptime U: type, …)`).
* **Common Lisp**: erased. The slot is declared `t`, the parameter disappears,
  and a bound disappears with it.
* **OCaml**: `type 't t`, the parameter carried in the type expression.

Erasure is the reason `design/equality.md` cannot type `*e == v` in Common
Lisp: the encoding that costs nothing here is the encoding that destroys what
equality needs.

## Methods

An enum's `impl` blocks are collected and emitted exactly as a struct's
(`design/struct.md`): inside the Zig container, prefixed in Common Lisp
(`option-is-none`), inside the module in OCaml.

One hazard is enum-specific. **Zig's `union(enum)` fields and methods share a
namespace**, so a Rust `Result`-shaped enum whose variants are `Ok`/`Err`
produces fields `ok`/`err` that collide with methods of the same names. This is
the README's standing bug; it has no counterpart in the other two, where a
variant and a method are different kinds of name.

## The holes, ranked

### 1. Explicit discriminants are silently dropped

No backend reads `syn::Variant::discriminant`. `enum Color { Red = 1, Green = 2 }`
translates as though the values were not written, so `Color::Red as i32` (if it
worked at all -- see below) would answer `0`.

Zig has the feature natively (`enum(u8) { red = 1 }`), so the Zig fix is a
faithful one. Common Lisp and OCaml have no discriminant to attach: Common Lisp
would need the keyword-to-number mapping emitted as a function, and OCaml the
same. No fixture has a discriminant, and a C-like enum with values is common
enough in FFI-adjacent code that this should become a marker even before it
becomes a translation.

### 2. Casting an enum to an integer is unhandled

`translate_cast` knows nothing about enums, so `Direction::North as i32` emits
`@as(i32, .north)` -- a Zig compile error (fact 7), where the correct lowering
is `@intFromEnum`. Loud, so it is a gap rather than a bug, and it is the same
gap as the discriminants above seen from the use site.

### 3. `#[repr(u8)]` and `#[non_exhaustive]` are ignored

`#[repr]` matters only with discriminants, so it is item 1's dependency.
`#[non_exhaustive]` changes what rustc requires of a `match` in *other* crates,
not in this one, so ignoring it is right for a single-crate translation and
would need revisiting if a translation ever spans crates.

### 4. Recursive enums

`rust/regex`'s `Ast::Empty(Box<Span>) | Ast::Literal(Box<Literal>) |
Ast::Concat(Box<Concat>)` needs `Box` before it needs anything enum-shaped;
`doc/lisp-regex.md` has the inventory. Worth listing here because a recursive
`union(enum)` is where Zig's need for indirection becomes visible in the
encoding rather than in the field type: by fact 8, a union reachable from its
own field by value is a `dependency loop`, and the same declaration with one
pointer in the cycle compiles. So `Box<T>` cannot be erased to `T` the way
`Cell<T>` is (`design/cell.md`) -- in a recursive type it has to survive as
`*T`, and deciding *where* the pointer goes is the design that document will
need.

### 5. An uninhabited enum

`enum Void {}` has no variants, so `has_data` is false and the Zig backend would
emit `enum {}`. Untested, and unlikely to matter.

## Test

| Path | Role |
|------|------|
| `rust/direction`, `zig/direction.zig`, `lisp/direction.lisp` | The payload-free encoding in both splitting backends: a Zig `enum` and a `(member …)` type, plus the or-pattern arm |
| `rust/geometry`, `zig/geometry.zig`, `lisp/geometry.lisp`, `ml/geometry` | The data-carrying encoding in all three, with unit-free tuple and struct variants |
| `rust/option`, `zig/option.zig`, `lisp/option.lisp`, `ml/option` | A generic enum with methods, a unit variant inside a data-carrying enum, and the crate-defined-`Option` case the moniker check has to see through |
| `rust/calc`, `rust/regex` | A fieldless enum and a unit struct used as *errors*, which leave the enum encoding entirely (`design/result.md`) |

No fixture has an explicit discriminant, an enum-to-integer cast, a
`#[repr]`, a recursive enum, or a payload-free enum on the OCaml side. The
first two are one feature and are the ones worth adding, since they are the
only silent hole in this document.

## Not implemented yet

1. Explicit discriminants, in all three backends.
2. `enum as integer` casts (`@intFromEnum`, and a mapping function elsewhere).
3. `#[repr]` on an enum.
4. A payload-free OCaml fixture -- emission exists, nothing pins it.
5. Recursive enums, pending `Box`.

## Not planned

* Encoding a payload-free enum as the general form to save a code path. The
  Zig facts above are the argument against it, and the Common Lisp equality
  story is the same argument again.
* `enum` layout control, niche optimization, or anything that depends on Rust's
  representation choices. The translation targets the *language*, and none of
  the three targets exposes a comparable notion.
* Trait impls on enums beyond what `design/struct.md` records -- attached like
  inherent impls, with `Drop` the only trait the analysis inspects.
