# Structs

How a Rust struct and its `impl` blocks are translated. **Implemented in all
three backends**: `src/translate/zig/item.rs`, `src/translate/lisp/item.rs`,
and the OCaml backend's item translation. Driven by `rust/geometry` (a struct
with a `&mut self` method), `rust/bitset` (an associated function and `&self` /
`&mut self` methods), and `rust/random` (associated constants and `Self`).

Enums are a separate question and mostly live elsewhere: the encodings are in
`doc/lisp.md`'s "Structs and enums" and the dispatch in `design/match.md`. What
is here is the part the two share -- a variant with fields *is* a struct in
every backend -- plus everything an `impl` block brings.

## The same fixture, three ways

`rust/geometry`'s `Point`, which is two fields and one `&mut self` method:

```rust
pub struct Point { pub x: i32, pub y: i32 }

impl Point {
    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.x += dx;
        self.y += dy;
    }
}
```

```zig
const Point = struct {
    const Self = @This();

    x: i32,
    y: i32,

    fn translate(self: *Self, dx: i32, dy: i32) void {
        self.x += dx;
        self.y += dy;
    }
};
```

```lisp
(defstruct point
  (x 0 :type (signed-byte 32))
  (y 0 :type (signed-byte 32)))

(declaim (ftype (function (point (signed-byte 32) (signed-byte 32)) null) point-translate))
(defun point-translate (self dx dy)
  (incf (point-x self) dx)
  (incf (point-y self) dy)
  nil)
```

```ocaml
module Point = struct
    type t = {
        mutable x : int;
        mutable y : int;
    }

    let translate self dx dy =
        self.x <- self.x + dx;
        self.y <- self.y + dy
end
```

Four axes separate these, and each is answered independently below: where
methods live, how the receiver is spelled, how fields are named and typed, and
what happens to `pub`.

## Namespace: Zig has one, and the others invent one

Rust's `impl` block is a per-type namespace. Only Zig has that natively -- a
struct is a container, and a declaration inside it is scoped to it -- so the
Zig backend's `analyze` collects impl blocks onto their type and emits the
methods *inside* the `struct { … }`, which is also why the container needs no
naming scheme at all.

The other two invent one:

* **Common Lisp prefixes.** One namespace for everything, so a method takes the
  type as a prefix: `p.translate(3, 4)` is `(point-translate p 3 4)`. An
  associated function is named the same way (`(bit-set-with-capacity 16)`),
  there being no receiver to tell them apart once both are plain functions.
  `doc/lisp.md` records why CLOS -- which *does* have per-type method names --
  was rejected: it turns one Rust function into N definitions and emission
  stops being structural.
* **OCaml nests a module.** `module Point = struct type t = … end`, with the
  type conventionally named `t` and methods as ordinary functions taking the
  receiver first, so the call site is `Point.translate p 3 4`. This is the same
  answer as the Common Lisp prefix with the language supplying the separator.

One consequence is worth stating: **Rust's method-vs-associated-function
distinction survives only in Zig**, where `p.translate(…)` and
`Point.withCapacity(…)` are different call syntaxes. In the other two both are
plain applications, and the receiver is just the first argument.

## Receivers, and where mutation comes from

| Rust receiver | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `self` | `self: Self` | `self` | `self` |
| `&self` | `self: *const Self` | `self` | `self` |
| `&mut self` | `self: *Self` | `self` | `self` |
| how mutation reaches the caller | the pointer | a structure *is* a reference | a `mutable` field |

Zig is the only backend that spells the receiver's mutability, and it is
therefore the only one where the receiver rules interact with anything else:
`design/cell.md`'s `Cell` erasure promotes a `&self` receiver to `*Self` when
the method writes through a `Cell`, and `design/result.md`'s level 3 would
promote one again to record a diagnostic.

Common Lisp needs nothing. A structure is already a reference, so
`(incf (point-x self) dx)` mutates the caller's value, and slots are mutable
unconditionally.

OCaml is the interesting one, because a record is a *value* and mutability is
per field. `ml/bitset` shows the analysis at work:

```ocaml
type t = {
    mutable data : int;
    length : int;
}
```

`data` is assigned by `put` and `toggle`; `length` never is, and stays
immutable. So the OCaml backend is the only one where a struct field's
mutability is a property inferred from the crate rather than read off the
receiver. `doc/ml.md` is where that lives.

## Fields

* **Names.** Unchanged in Zig and OCaml; kebab-cased in Common Lisp
  (`BitSet { data, length }` -> `bit-set` with slots `data` and `length`;
  a `first_name` field would be `first-name`).
* **Types.** Zig and OCaml annotate structurally. Common Lisp emits `:type` on
  every slot, which is what makes SBCL check it, and therefore needs a **slot
  default that satisfies the declared type** -- `0` for a numeric slot, `nil`
  otherwise (`slot_default`). The default is never the value the program uses,
  since every construction site passes every field; it exists only so the
  `defstruct` is well-formed.
* **Tuple fields.** A tuple *variant*'s fields are positional in every backend:
  Common Lisp names them `v0`, `v1` (`fields_of`), Zig emits an anonymous tuple
  type as the payload (`line: struct { Point, Point }`), OCaml a constructor
  with a tuple argument. A tuple *struct* is a different story -- see
  [Holes](#the-holes-ranked).
* **Struct-variant fields** stay named in all three: Zig's
  `circle: struct { center: Point, radius: i32 }`, Common Lisp's
  `(defstruct shape-circle (center nil :type point) …)`, OCaml's inline record.

## Construction

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `Point { x: 1, y: 2 }` | `Point{ .x = 1, .y = 2 }` | `(make-point :x 1 :y 2)` | `{ Point.x = 1; y = 2 }` |
| `Self { … }` | `Self{ … }` | `(make-rand32 …)` | `{ … }` |
| `Shape::Dot(p)` | `.{ .dot = p }` | `(make-shape-dot :v0 p)` | `Shape.Dot p` |
| `Shape::Circle { center, radius }` | `.{ .circle = .{ … } }` | `(make-shape-circle :center p :radius 1)` | `Shape.Circle { center = p; radius = 1 }` |

OCaml's record literal is qualified **on its first field only** and only from
outside the module -- `{ Point.x = 1; y = 2 }` in `ml/geometry/test/test.ml`,
against the bare `{ data = 0; length = bits }` inside `ml/bitset`'s own module.
That is idiomatic OCaml (the first label picks the record type and the rest
follow), and it is a per-site decision the translator has to make rather than a
property of the type.

Zig **preserves the source's spelling** of the type: `rust/bitset` writes
`BitSet { … }` and gets `BitSet{ … }`, `rust/random` writes `Self { … }` and
gets `Self{ … }`. The other two never see `Self` at all, because the
`self_type` desugar pass rewrites it to the enclosing block's type before
translation -- Zig skips that pass precisely because it has a `Self` of its own
to emit.

Common Lisp's constructor is `make-<struct>` with keyword arguments, which is
what `defstruct` gives for free and is why the constructor name has to be in
the package's export list beside the type.

## Associated constants

`rust/random`'s `Rand32::DEFAULT_INC`:

| | spelling | at the use site |
|---|---|---|
| Zig | `const defaultInc: u64 = …` inside the container | `defaultInc`, unqualified -- it is already in scope |
| Common Lisp | `(defconstant +rand32-default-inc+ …)` at top level | the same name; the `+…+` convention and the type prefix are the whole namespacing |
| OCaml | a `let` inside the module (`design/recursion.md`'s target output; no fixture yet) | `Rand32.default_inc` |

Zig prints associated consts *ahead of* the fields in the container body, since
a container's declarations and its fields are different things and grouping
them reads better. Common Lisp attaches no `declaim` to a constant: the value is
a literal the compiler already sees, and a constant cannot be assigned, so
there is nothing a declaration would catch.

## Visibility

**`pub` reaches only the Common Lisp backend**, as the package's `:export`
list -- the type, its constructor, its accessors, its methods, and its
constants:

```lisp
(:export #:point #:make-point #:point-x #:point-y #:point-translate …)
```

Zig drops it: there is no `pub` anywhere in `zig/*.zig`, so every declaration
is file-private. OCaml drops it too, having no generated `.mli`.

This costs nothing today, because a translation unit is one file and its tests
are inside it. It becomes the first question to answer the day a translation
spans modules, and the Common Lisp side is the one that already has the
answer -- `is_public` is consulted at `analyze` time and the export list falls
out of it.

## Generics

`rust/option`'s `Option<T>`, the only generic fixture:

* **Zig**: a function returning a type. `fn Option(comptime T: type) type {
  return union(enum) { … } }`, with methods inside it and further comptime
  params after `self` (`fn @"and"(self: Self, comptime U: type, …)`).
* **Common Lisp**: erased. A slot takes no `:type`, the parameter disappears,
  and a bound disappears with it.
* **OCaml**: a parameterized type, `type 't t`.

Generic *structs* have no fixture; `rust/option` is an enum. Nothing in the
struct path is generic-specific, so it should fall out, but it is untested.

## Traits and derives

`analyze` attaches trait impls to their type exactly like inherent ones, and
looks at the trait path for one thing only: `core::ops::drop::Drop`
(`zig/mod.rs:113`), which drives `design/drop.md`. Everything else is emitted as
an ordinary method of the container.

That is fine for a trait nobody calls through, and silently wrong for one the
language calls implicitly. `impl PartialEq for X` becomes a method `eq` that
`==` never reaches (`design/equality.md`); `impl Display` would become a method
`fmt` that nothing calls. Derives are dropped outright, which is correct --
every target compares and copies structs without being asked.

## The holes, ranked

### 1. A tuple struct panics the Zig backend

`translate_struct` reads `field.ident.as_ref().unwrap()`. For
`pub struct Meters(u32);` the fields are unnamed, `ident` is `None`, and the
translator **panics** rather than emitting a marker. Every other unsupported
construct in the backend leaves a `TODO`; this one takes the process down, and
`rust/regex`'s neighborhood (newtypes are ordinary in parser code) is where it
would first be hit.

The fix is the one Common Lisp already made: name the fields positionally
(`v0`, `v1`), which is also what the Zig backend does for tuple *variants*
today. A unit struct needs nothing -- it is an empty field list, and
`rust/regex`'s `pub struct Error;` and `ParserBuilder;` already translate.

### 2. Visibility is dropped by two backends

Above. Latent until modules exist, and cheap to keep: `is_public` is already
computed on the Common Lisp side, and the Zig one would only have to prepend
`pub `.

### 3. Field/method namespace collision

Zig's `union(enum)` fields and methods share a namespace, so a Rust `Ok`/`Err`
variant pair becomes fields `ok`/`err` that collide with methods of those
names. This is the README's standing bug, and it also bounds
`design/equality.md`'s level 3, whose generated `eql` method is another name the
container would own.

### 4. Struct update syntax and layout attributes

`Point { x: 1, ..base }`, `#[repr(C)]`, `#[repr(packed)]`, and alignment are
unhandled everywhere. The update syntax is the only one with a natural
translation in all three (a copy plus overrides -- Zig has no sugar, Common
Lisp has `copy-<struct>`, OCaml has `{ base with x = 1 }`), and no fixture
needs it.

### 5. Recursive and boxed field types

`rust/regex`'s `Concat { asts: Vec<Ast> }` and `Ast::Literal(Box<Literal>)`
need `Vec` and `Box` first, which are `doc/lisp-regex.md`'s inventory rather
than a struct question. Worth noting only because a self-referential struct is
where Zig's `Self` and OCaml's recursive-module rules would both have something
to say, and neither has been tested.

## Test

| Path | Role |
|------|------|
| `rust/geometry` | The three-way fixture above: a struct with a `&mut self` method, plus tuple and struct *variants* of an enum, in all three backends |
| `rust/bitset` | An associated function returning `Self`, `&self` and `&mut self` methods, and construction by type name |
| `rust/random` | Associated constants, `Self { … }` construction, and `Self` as a return type -- the fixture that pins the `self_type` pass |
| `rust/option` | The only generic container: `fn Option(comptime T: type) type` against Common Lisp's erasure |
| `rust/drop` | Trait impls that `analyze` treats specially (`design/drop.md`) |

No fixture has a tuple struct, a unit struct outside `rust/regex`, a generic
struct, a struct update expression, or a `#[repr]` attribute. The first of
those is the one that crashes rather than degrades, so it is the one worth
adding first -- a newtype is two lines in `rust/bitset`.

## Not implemented yet

1. Tuple structs (Zig panics; Common Lisp already handles them).
2. `pub` in the Zig and OCaml output.
3. Struct update syntax.
4. Generic structs, untested rather than known-broken.
5. `#[repr]`, packing, and alignment.

## Not planned

* CLOS for Common Lisp methods. Rejected in `doc/lisp.md`: it makes emission
  non-structural. Revisit only if name collisions become common.
* Trait dispatch of any kind -- `dyn Trait`, generic bounds resolved to impls,
  blanket impls. `design/bound.md` covers what comptime bounds do instead.
* Preserving field order for layout purposes. Every backend emits fields in
  source order, but none of them promises the target does anything with it, and
  a Rust `#[repr(Rust)]` struct has no guaranteed layout to preserve anyway.
