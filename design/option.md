# Option

How `core::option::Option` is translated. **Implemented in all three backends**,
each onto the target's own notion of absence: Zig's optional `?T`, Common
Lisp's `nil`, OCaml's `option`. What is implemented is construction, `if let`,
`match`, `?`, and `unwrap` -- not the combinator API.

The `?` operator is shared with `Result` and its dispatch is
`design/result.md`; matching is `design/match.md`; a *crate-defined* enum named
`Option` is an ordinary enum and follows `design/enum.md`; comparing two
`Option`s is `design/equality.md`.

## Why `Option` is not translated as an enum

It is an enum in Rust, and every backend can already encode an enum
(`design/enum.md`). It is special-cased anyway, in all three, because **every
target already has an absence type**, and using it is the difference between
output that reads like the target language and output that reads like a
transliteration.

The check is by moniker (`core::option::Option::Some`, `…::None`,
`…::unwrap`, `core::option::branch`), never by name. That is what keeps a
crate-defined enum called `Option` on the ordinary path -- `rust/option` is the
fixture, and it becomes `union(enum)` in Zig and `option-some` / `option-none`
structures in Common Lisp with no special treatment at all.

## The mapping

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `Option<T>` in a type | `?T` | `(or null T)` | `t option` |
| `Some(x)` | `x` -- erased, coerces | `x` -- erased | `Some x` |
| `None` | `null` | `nil` | `None` |
| `if let Some(x) = e` | `if (e) \|x\|` | `(let ((x e)) (if x …))` | lowered to a `match` |
| `match` on an `Option` | labeled block, one `if` per arm | **marker** | native `match` |
| `e?` | `e orelse return null` | **marker** | `let* x = e in` over `Option.bind` |
| `x.unwrap()` | **marker** | `(or x (error "…"))` | `Option.get` |
| `assert_eq!(None, e)` | `expectEqual(null, e)` | `(equal nil e)` | `assert (None = e)` |

Three cells are gaps rather than translations, and they are not the same gap in
each backend -- see [the holes](#the-holes-ranked).

The Common Lisp type declaration is worth reading twice: `Option<usize>` is
declared `(or null fixnum)`, so the encoding is not merely a convention in the
emitted code, it is what the `declaim` says and what SBCL checks.

## Two encodings nest; one flattens

Rust's `Option<T>` is a wrapper: `Some(None)` and `None` are different values,
and so are `Some(false)` and `None`. Whether the translation preserves that is
the whole design question, and the three targets answer differently.

### Zig facts

| # | Case | Result |
|---|---|---|
| 1 | `??i32`, comparing a plain `null` against `@as(?i32, null)` | distinguishable -- the outer and inner `null` are different values |
| 2 | `?bool` holding `false` | `a != null` and `a.? == false` both hold |
| 3 | `a.?` on a `?i32` | the payload; this is `unwrap` |
| 4 | `a orelse 7` | the default; this is `unwrap_or` |

### OCaml facts

| # | Case | Result |
|---|---|---|
| 1 | `Some None <> None` | `true` |
| 2 | `Some false <> None` | `true` |

So Zig's optional and OCaml's `option` are both genuine wrappers. Neither has a
soundness condition, and neither needs one.

### The Common Lisp encoding flattens

`Some(x)` erases to `x` and `None` is `nil`. Verified consequences:

| Rust value | Common Lisp | distinguishable from `None`? |
|---|---|---|
| `Some(0)` | `0` | yes |
| `Some(false)` | `nil` | **no** |
| `Some(None)` | `nil` | **no** |
| `None` | `nil` | -- |

and `if let Some(x) = e` on a `Some(false)` takes the **else** branch.

`doc/lisp.md` states the condition this rests on -- a payload that is never
`nil` or false -- and the table is what violating it costs: a silent wrong
answer, not an error. What it excludes is precise: `Option<bool>`,
`Option<Option<T>>`, `Option<()>`, and an erased `Option<T>` instantiated at
`bool`. Nothing else. `Option<i32>`, `Option<usize>`, `Option<&str>`, and
`Option<SomeStruct>` are all safe, because `0`, `""`, and a structure are all
true in Common Lisp.

The type declaration shows the flattening from the other side:
`Option<bool>` declares as `(or null boolean)`, and `boolean` is
`(member t nil)`, so `nil` is a member of the type twice over and the
declaration cannot separate the cases either.

### Why keep it, and what level 2 is

The erasure is why `lisp/div.lisp` and `lisp/iter.lisp` read like Lisp: a
function that may fail returns `nil`, the caller tests it, and nothing wraps
anything. That is what a Common Lisp programmer writes, and switching to a
uniform wrapper would make every `Option`-returning function in the crate
noisier to serve two payload types that no fixture has.

**The safe encoding already exists in the tree.** A crate-defined `Option` is
translated as an ordinary data-carrying enum -- `(defstruct option-some (v0 …))`,
`(defstruct option-none)`, `(deftype option () '(or option-some option-none))` --
and `lisp/option.lisp` pins it. So level 2 is not new machinery:

> When the payload type is `bool`, another `Option`, or unknown, translate a
> `core::option::Option` through the ordinary enum encoding instead of the
> `nil` erasure.

The payload type is available from the same `type_argument` the `(or null T)`
declaration already reads, so the decision is per type and cheap. Two caveats:

* The two encodings then coexist in one program, which is fine because a value's
  encoding follows its payload type and values of different payload types never
  meet -- **except through an erased generic**. `position<T>`'s `Option<usize>`
  is concrete, but a `fn f<T>(x: Option<T>)` would have no payload type to
  decide on, which is why "unknown" is on the list above and must pick the safe
  encoding.
* Until level 2 exists, the honest alternative is a marker: refuse an
  `Option<bool>` or a nested `Option` rather than emit the erasure. That is a
  three-line check and it converts the only silent hole in this document into a
  visible one.

## The API surface

Handled: construction, `if let`, `match`, `?`, `unwrap`. Everything else is
unhandled in every backend. What the natural lowerings are, for the ones a real
crate reaches for first:

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `x.unwrap_or(d)` | `x orelse d` | `(or x d)` | `Option.value x ~default:d` |
| `x.is_some()` | `x != null` | `x` itself, or `(not (null x))` | `Option.is_some x` |
| `x.is_none()` | `x == null` | `(null x)` | `Option.is_none x` |
| `x.map(f)` | needs return-only generics (`TODO.md`) | `(and x (funcall f x))` | `Option.map f x` |
| `x.ok_or(e)` | `x orelse return error.E` | -- (no `Result` encoding yet) | `Option.to_result` |

`unwrap_or` is the cheapest and the most common, and in Zig it needs nothing new
-- `orelse` is already emitted for `?`. `map` is the one with a real
prerequisite, and `TODO.md` has the staged plan: it is blocked on a type
parameter that appears only in the return type, not on anything `Option`-shaped.

Note the Common Lisp column is where the erasure pays off: `unwrap_or` is `or`,
`is_some` is the value itself in a boolean context, and `map` is `and`. Those
are the idioms the encoding was chosen for -- and each one is also a place where
an `Option<bool>` would go wrong, which is the same trade seen from the inside.

## The holes, ranked

### 1. `Option<bool>` and nested `Option` in Common Lisp

Above. The only silent hole here.

### 2. Zig handles `Result::unwrap` but not `Option::unwrap`

`zig/call.rs` special-cases `core::result::Result::unwrap` and nothing else;
the `core::option::Option::unwrap` moniker is in the shared table
(`moniker.rs:25`) and only the Common Lisp backend uses it. So `x.unwrap()` on
an optional emits a Zig method call to a method that does not exist.

Loud, and the lowering is one token: `x.?` (fact 3). It matters now rather than
later because `rust/regex` calls it four times, all as
`self.…checked_add(1).unwrap()` -- which also makes it the first place
`design/operator.md`'s `checked_*` family and this document meet.

### 3. `?` and `match` on an `Option` are markers in Common Lisp

The Lisp `translate_expr` has no `syn::Expr::Try` arm at all, so `e?` is a
`todo("expr")` marker regardless of flavor. And a `match` whose arms are core
`Some` / `None` patterns fails in `translate_pat` -- `struct_named` finds no
structure for `Option::Some`, since the erasure defines none -- so the whole
match becomes `(todo "match")`.

Only `if let Some(x)` is lowered, which is why `rust/div` (an `if let`) has a
Lisp fixture and `rust/hash` (a `match` with a guard on `Some`) does not. The
natural lowerings both exist: `?` in a statement position is the same
bind-and-test the `if let` already emits, and a two-arm `Some`/`None` match is
an `if` on the bound value. Neither is more than the `if let` path generalized.

### 4. The combinators

Above. `unwrap_or` first, `map` last (it needs the generics work).

## Test

| Path | Role |
|------|------|
| `rust/div`, `zig/div.zig`, `lisp/div.lisp`, `ml/div` | The three encodings side by side: `?u32` / `(or null fixnum)` / `int option`, plus `if let Some` in all three |
| `rust/iter` | `Option<usize>` returned from a search, and `assert_eq!(None, …)` in all three backends |
| `rust/hash` | The `match` on an `Option` with a guard -- the labeled-block lowering in Zig, and the reason there is no Lisp fixture |
| `rust/calc` | `?` on an `Option` (`half` / `quarter`), which is the `orelse return null` flavor in Zig and `Option.bind` in OCaml |
| `rust/option` | The control: a crate-defined enum named `Option` that gets none of this |
| `rust/regex` | Unfixtured, and where `Option::unwrap` and `checked_*` first become necessary |

No fixture has an `Option<bool>`, a nested `Option`, or any combinator beyond
`unwrap`. The first two are the ones that would fail silently, so they are worth
adding to `rust/div` as soon as level 2 or the marker exists -- and not before,
since today they would simply produce a wrong Lisp answer.

## Not implemented yet

1. Level 2 for Common Lisp, or the marker that precedes it.
2. `Option::unwrap` in Zig (`.?`).
3. `?` and `match` on an `Option` in Common Lisp.
4. `unwrap_or`, `is_some`, `is_none`, `ok_or`, and the rest of the combinators.
5. `Option::map`, blocked on return-only generic parameters (`TODO.md`).

## Not planned

* Preserving `Option`'s niche layout guarantees (`Option<&T>` being pointer-
  sized, and so on). Zig's optional pointer has the same property by
  coincidence, and neither of the other targets has a comparable notion.
* `Option<&mut T>`, `as_ref` / `as_mut` / `take` / `replace`, which are about
  moving ownership through an optional rather than about `Option` itself.
* Treating a user type as an `Option` because it looks like one. The moniker
  check exists precisely to make that impossible.
