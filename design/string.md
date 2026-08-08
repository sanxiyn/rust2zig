# Strings (OCaml)

How Rust's `&str` and `&[u8]` should be represented in the OCaml backend.
**Implemented** in `src/translate/ml/string.rs`, with `rust/hash` / `ml/hash` as
the golden pair. Driven by `rust/hash` — the Test section of
`design/integer.md` left this as the open design question. The OCaml snippets
here are verified against OCaml 5.4.1.

Unlike `design/integer.md` this is not an OCaml-only concern; the Zig backend
answered it first, and the answer is worth reading before choosing a different
one.

## What Zig did

Zig has no string type, and the backend leans on that entirely.

* `translate_type` (`zig/ty.rs:78`) maps `&str` to `[]u8` — the *same node*
  `&[u8]` gets from the `Type::Slice` branch two lines above. The two Rust types
  collapse into one Zig type.
* `as_bytes()` is therefore the **identity**, dropped from the output
  (`zig/call.rs:95`).
* A `syn::Lit::Str` becomes a Zig string literal (`zig/expr.rs:213`), which
  already coerces to `[]const u8`.
* `.len()` is `.len` and `bytes[i]` is `bytes[i]`, with no special casing. A
  `u8` slice indexes like any other slice.

`zig/hash.zig` is the whole story:

```zig
fn fnv1aHashStr32(input: []const u8) u32 {
    return fnv1aHash32(input, null);
}
```

The `as_bytes` vanished, `"foobar"` stayed a literal, and nothing about `u8` was
special-cased.

The thing to notice is *which way* the collapse went. Zig did not give `[]u8`
string powers; it observed that a Rust `str` **is** bytes and mapped it onto the
uniform slice representation. Slices stayed uniform.

## The rule

That exact move is unavailable in OCaml. OCaml's uniform slice representation is
`'a array`, so collapsing in Zig's direction gives `&str` -> `int array`, and
`"foobar"` comes out as `[| 102; 111; 111; 98; 97; 114 |]`. That is the one
outcome clearly worth avoiding.

So the collapse goes the other way:

> `&str` and `&[u8]` are both represented as OCaml `string`.

This is not an approximation. OCaml's `string` is an immutable byte string with
O(1) length and O(1) indexing — the precise semantics of `&[u8]`, and of `&str`
minus the UTF-8 invariant, which nothing in the representation needs to enforce
because Rust already did.

The two Rust types being one OCaml type is what makes `as_bytes()` the identity,
exactly as in Zig. It is also what lets `fnv1a_hash_32`, whose parameter is
`&[u8]`, be called with a `&str` that has been through `as_bytes`.

## Where the decision lives

Barely in `ty.rs`. The ml backend emits almost no types: function parameters are
unannotated (`let fnv1a_hash_32 bytes limit =`), and `translate_type` has no
`syn::Type::Reference` arm at all, so `&[u8]` rarely reaches a type position
that prints. `is_string_type` is therefore consulted at the top of
`translate_type`, before the path walk, rather than in `map_type_name`: it has to
answer for `&str` and `&[u8]`, which are a `Type::Reference` and a `Type::Slice`
and so never reach a type *name* at all.

The decision materializes in four operations:

| Rust | OCaml | Where |
|------|-------|-------|
| `"foobar"` | `"foobar"` | `translate_lit`, a new `syn::Lit::Str` arm |
| `s.as_bytes()` | identity, dropped | `translate_method_call`, by SCIP moniker |
| `bytes.len()` | `String.length bytes` | `translate_method_call`, type-directed |
| `bytes[i]` | `Char.code bytes.[i]` | `translate_index`, type-directed |

The first two are additions. The last two were unconditional (`Array.length`,
`ArrayGet`) and became type-directed: `is_string_expr` resolves the receiver
through the shared `translate::ty::expr_type` plus `peel_ref` — the same
machinery `int.rs`'s `collect_widths` uses. An expression whose type SCIP cannot
resolve is assumed not to be a string, so the array lowering stays the fallback
and no existing output moves.

Note that `.len()` keeps its SCIP moniker gate and gains the type dispatch
*inside* it, rather than replacing it: the moniker establishes that this is a
slice `len` at all, and the receiver type then picks the module.

`ArrayGet` gained a sibling `StringGet` in `ast/ml.rs`, printing `s.[i]` at the
same precedence.

## Interaction with `design/integer.md`

`Char.code` yields an `int`, which is exactly how `u8` is represented, so the
two designs compose without a special case. The worked example there becomes one
`Char.code` deeper:

```rust
hash ^= bytes[i] as u32;
```

```ocaml
hash := Int32.logxor !hash (Int32.of_int (Char.code bytes.[!i]))
```

This is also why indexing must stay type-directed rather than being switched
globally: `expr_type`'s `syn::Expr::Index` arm is what tells the cast that
`bytes[i]` is a `u8` in the first place, and it reads the *Rust* slice type, so
it is unaffected by the representation choice.

## Worked example: `rust/hash`

Verified to produce `0xbf9cf968` for `"foobar"` under OCaml 5.4.1. This is
`design/integer.md`'s worked example with the four operations above applied.

```ocaml
let fnv1a_hash_32 bytes limit =
    let prime = fnv_prime_32 in
    let hash = ref fnv_offset_basis_32 in
    let i = ref 0 in
    let len = match limit with
        | Some v when 0 < v && v < String.length bytes -> v
        | _ -> String.length bytes
    in
    while !i < len do
        hash := Int32.logxor !hash (Int32.of_int (Char.code bytes.[!i]));
        hash := Int32.mul !hash prime;
        i := !i + 1
    done;
    !hash

let fnv1a_hash_str_32 input =
    fnv1a_hash_32 input None

let foobar = "foobar"
```

`fnv1a_hash_str_32` is the payoff: the `as_bytes` is gone, and the function is
what an OCaml programmer would have written.

## Mutation

`string` is immutable, so a `&mut [u8]` cannot be one. OCaml's answer is
`bytes`, with `Bytes.length` / `Bytes.get` / `Bytes.set` and `Bytes.of_string` /
`Bytes.to_string` at the boundary.

This is the same shape as integer escalation: one default representation,
escalated when the crate is observed to need more, decided once per Rust type
for the whole crate. It should be a level rather than code until a fixture
needs it — no current fixture mutates a byte slice.

Note the asymmetry with integers: a byte slice's mutability is written in the
Rust type (`&mut [u8]`), so detecting it needs no analysis pass, only a decision
about granularity. Whole-crate is wrong here in a way it was not for integers,
since a crate can legitimately have both an immutable input buffer and a mutable
output one, and `&[u8]` and `&mut [u8]` are already distinguishable Rust types.
Keying on the mutability of the reference is the obvious first cut.

## `char` is a different problem

Rust's `char` is a Unicode scalar value; OCaml's `char` is a byte. `rust/regex`
uses `char`, `.chars()`, and `len_utf8`, none of which this design addresses,
and choosing `string` for `&str` does not commit us on any of them — an OCaml
`string` is a byte string whether or not we ever decode it.

Zig has not solved this either: there is no `zig/regex.zig` golden. That both
backends are stuck at the same place suggests it is a problem about Rust's
`char`, not about either target, and it should get its own design rather than
being folded in here.

## Test

| Path | Role |
|------|------|
| `rust/hash`, `ml/hash` | golden pair |

`rust/hash` is the fixture for this, and it is the fixture for
`design/integer.md` too. It emits with no `TODO` left, and `test_test.sh hash`
passes on all three of Rust, Zig, and OCaml, so the FNV-1a result agrees across
backends.

Landing it also needed one printer change, which belongs to neither design: a
`let` binding whose right-hand side does not fit on a line now takes an indented
block with `in` on its own line, so `let len = match limit { ... }` renders as a
`match` rather than falling through to `(* TODO: expr *)`. The gate is
`is_simple`, the predicate that already decides the same question for a match
arm's right-hand side.

Behavioral parity is checked the usual way, `dune test` on the emitted tree
against `cargo test` on the input.

## Alternatives considered

* **Keep slices uniform: `&str` and `&[u8]` both `int array`.** The direct
  translation of what Zig did, and it needs no type-directed dispatch at `.len()`
  or `[i]` — both stay as they are, and `as_bytes` is still the identity.
  Rejected on the literal: `"foobar"` becomes `[| 102; 111; 111; 98; 97; 114 |]`,
  which is unreadable, allocates per use, and cannot be compared with `=` against
  anything a reader would recognise. The backend exists to produce idiomatic
  OCaml, and no OCaml programmer represents a string as an `int array`.
* **`&str` -> `string`, `&[u8]` -> `int array`.** The most literal reading of the
  two Rust types, keeping non-`u8` slices and `u8` slices uniform with each
  other. Rejected because it makes `as_bytes()` a real conversion —
  `Array.init (String.length s) (fun i -> Char.code s.[i])` — which is O(n),
  allocates, and appears in the output at a place where Rust wrote nothing at
  all. Zig's identity mapping is the better precedent.
* **`&[u8]` -> `bytes` uniformly**, immutable or not. One representation covers
  both mutability cases, so the mutation level above disappears. Rejected because
  a string literal is then `Bytes.of_string "foobar"` at every use, `=` on
  `bytes` is mutable-structure equality, and the common case pays for a case no
  fixture has. `bytes` stays the escalation target, not the default.
* **A `Slice` abstraction over `string` and `array`.** Would let `.len()` and
  `[i]` stay uniform without a type-directed dispatch. Rejected for the reason
  `design/integer.md` rejected a `Uint32` module: it is a runtime library shipped
  with the output, and the backend's stdlib references are a closed set.
