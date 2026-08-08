# Integer widths (OCaml)

Status and roadmap for representing Rust's fixed-width integers in the OCaml
backend. **Levels 1 and 2 implemented** in `src/translate/ml/int.rs`; there is
no golden fixture yet, because `rust/hash` is blocked on two gaps that have
nothing to do with integers (see [Test](#test)). Driven by `rust/hash`, whose
FNV-1a needs multiplication modulo 2^32, which 63-bit `int` cannot express. The
OCaml snippets here are verified against OCaml 5.4.1.

This is an OCaml-only concern. Zig has the Rust widths natively, so the whole
question is about a target whose native integer is neither 32 nor 64 bits wide.

## The observation that shapes everything

OCaml's `int` is 63-bit and signed. That is *wider* than every Rust integer
type up to 32 bits, so `int` is a faithful representation of `u8` / `i8` /
`u16` / `i16` / `u32` / `i32` — as long as the program never observes the
width. It can only observe it by wrapping, and in Rust a non-wrapping overflow
is a bug, not a semantics. Rust code that means to wrap has to say so.

So the question is not "which OCaml type represents `u32`". It is "where does
this crate depend on `u32` being 32 bits", and the answer is almost nowhere.
That is what lets the common case stay idiomatic.

## The rule

> A Rust integer type is represented as OCaml `int` unless it is **escalated**,
> in which case it is represented as `Int32.t` (`int32`) or `Int64.t`
> (`int64`).

A type is escalated when either holds:

* **It does not fit.** `i64` and `u64` cannot be represented in 63 bits at all,
  so they are escalated unconditionally, to `int64`. `i128` / `u128` have no
  OCaml counterpart at all; they keep mapping to `int`, which is wrong, and
  they need a bignum or a pair representation nobody has designed yet.
* **The crate observes its width.** Some operation in the crate depends on the
  type being exactly that many bits wide. Minimally `wrapping_mul` /
  `wrapping_add` / `wrapping_sub`; see [Triggers](#triggers).

`usize` and `isize` stay `int` in both cases. They are lengths and indices,
their width is the target's rather than the program's, and `int` is what OCaml
itself uses for `Array.length`.

## Granularity: whole-crate, per Rust type

Escalation is decided **once per Rust primitive type name, for the whole
crate**. If any `u32` in the crate is `wrapping_mul`'d, every `u32` in the
crate is `int32`.

This is coarser than it needs to be, and that is the point. Rust has already
typechecked the input, so two values interact only when they have the same Rust
type. One representation per Rust type is therefore consistent everywhere by
construction — across function boundaries, struct fields, array elements, and
match arms — and **no conversion is needed at any of them**. The only place a
representation changes is an `as` cast, which is exactly where Rust already
wrote a conversion down:

```rust
hash ^= bytes[i] as u32;        //  bytes: &[u8], int    ->  hash: u32, int32
```

```ocaml
hash := Int32.logxor !hash (Int32.of_int (Char.code bytes.[!i]))
```

The alternative — deciding per binding, so that one `u32` can be `int` and
another `int32` — is a unification problem over the whole crate, with
representation coercions at every boundary where the classes disagree. It buys
a nicer output only for a crate that mixes a width-sensitive and an ordinary
value *of the same Rust type*; the cost is a real inference pass. Per-type is
the trivial solution to that unification, and it should be shown to be
insufficient before being replaced.

Known cost: in a crate that does mix them, the ordinary value gets the boxed
representation and the ugly arithmetic that comes with it. The workaround
available to the input is the one Rust programmers already use — give the two
uses different types (`u32` and `usize`).

## Triggers

The escalation set is computed by `collect_widths`, a third collector in the
OCaml backend's `analyze` alongside `collect_types` and `collect_refs`: one
`syn::visit` walk producing `escalated: HashSet<String>` of Rust type names.
Each trigger reads its type from the **method's own symbol**, via
`Scip::self_type_at`: an intrinsic's moniker spells out the type it is an
implementation for, as in `num/impl#[u64]wrapping_mul().`. That is both simpler
and strictly more capable than typing the receiver expression, because these
symbols are foreign — `core`'s, not the crate's — and the index carries
`SymbolInformation` only for crate-local symbols. So there is no signature to
read a return type from, and `expr_type` cannot answer for a receiver that is
itself such a call. See `research/random.md`.

Level 1 (what `rust/hash` forces):

* `x.wrapping_mul(y)` / `wrapping_add` / `wrapping_sub` — escalate the type the
  intrinsic implements, which is the receiver's and the result's alike, since
  these return `Self`. Gated on the SCIP moniker, the same dispatch shape as
  `.len()` -> `Array.length`.

Later, in rough order of how much they buy:

* `x.rotate_left(n)` / `rotate_right`, `leading_zeros`, `trailing_zeros`,
  `count_ones`, `swap_bytes`, `to_ne_bytes` — all meaningless without a width.
* Unary `!x` on an integer. OCaml's `lnot` is 63-bit, so `!0u32` is
  `0x7fffffffffffffff` rather than `0xffffffff`.
* `overflowing_*` and `checked_*`, which report at a width.

Deliberately **not** a trigger:

* `<<`. Every Rust shift discards the bits leaving the type, so treating `<<`
  as a width observation would escalate the ordinary `1 << 3` in any crate that
  bit-twiddles at all. Left out on the judgement that a shift whose result
  overflows the Rust type is rare in code that is not already using an
  escalated type for another reason. This is a real unsoundness, and it is the
  status quo: `translate_binary` emits `lsl` today.
* `wrapping_shl`, for the same reason: what it wraps is the shift *amount*, not
  the value, so its value semantics are exactly `<<`'s. It is lowered with the
  amount masked by the width (`land 0x3f`), elided when the amount is a literal
  already in range. It does appear in `SELF_RETURNING`, which is about typing
  the result, not about escalating.
* A narrowing `as` cast (`x as u8`). With `int`, truncation is `land 0xff`; it
  needs no boxed type.

`collect_widths` cannot be a desugar pass. "This `u32` is an `Int32`" is a
statement about the target's representation and is not expressible in Rust, so
it fails `doc/desugar.md`'s test.

## Signedness

`Int32` is signed, but a two's-complement bit pattern is a bit pattern. For
`u32` it is bit-for-bit correct on:

* `mul`, `add`, `sub`, `neg`
* `logand`, `logor`, `logxor`, `lognot`
* `shift_left`
* structural `=` and `<>`, so `assert_eq!` translates unchanged

and it is **wrong**, needing the Rust type's signedness to select the operator:

| Rust op on `u32` | wrong | right |
|------------------|-------|-------|
| `/`              | `Int32.div` | `Int32.unsigned_div` |
| `%`              | `Int32.rem` | `Int32.unsigned_rem` |
| `<` `>` `<=` `>=` | `<` | `Int32.unsigned_compare a b < 0` |
| `>>`             | `Int32.shift_right` | `Int32.shift_right_logical` |

(`unsigned_div` / `unsigned_rem` / `unsigned_compare` / `unsigned_to_int` are
all OCaml 4.08+, and `Int64` has the same four.) A signed Rust type takes the
left column, an unsigned one the right, so the operator is selected from the
operand's Rust type — the same shape as the Zig backend's `rem_is_signed` gate
on `%` -> `@rem`. `rust/hash` uses only the safe set, so this can land after
level 1.

Note `int` does not have this problem: since all of `u32` fits in 63-bit `int`
as a non-negative number, the *unescalated* representation gets unsigned
comparison and division from OCaml's ordinary signed operators for free. It is
escalation that introduces the signedness question, which is one more reason to
escalate only on demand.

## Literals

An escalated literal needs OCaml's `l` / `L` suffix, and `translate_lit` has no
expected type. `ast/ml.rs` already models the suffix — `Constant::Integer(String,
Option<char>)`, Parsetree's `Pconst_integer of string * char option` — so the
only question is where the type comes from.

**Rejected: generalizing the `integer_literal` desugar pass.** That pass already
stamps a Rust type suffix onto an unsuffixed literal (`1` -> `1u32`) from SCIP,
and generalizing it from shift left-hand sides to every resolvable literal would
be a valid-Rust rewrite, so `doc/desugar.md` would place it there. It is not
viable because the pass is shared: the Zig backend's `translate_lit` re-emits a
suffixed literal as `@as(T, n)`, so stamping suffixes crate-wide would rewrite
every Zig golden to be full of casts. Splitting the pass per backend would give
up the thing that made desugar attractive.

**What landed:** the translator applies the suffix at the two positions where
the expected type is already in hand, via `translate_int_operand`:

* a `const` item's declared type (`translate_const`) — which is where the
  escalated literals live, since OCaml puts no annotation on the binding;
* an operand of an escalated binary operation (`translate_int_binary`), which
  has to know the representation anyway to pick `Int32.mul`.

Everywhere else — a struct literal field, a call argument, an unannotated
`let` — an escalated literal keeps no suffix and OCaml reports a type error.
That is the intended failure mode, but it is a real limit: `rust/random`'s
`Rand32 { state: 0, inc: ... }` emits `state = 0` where it needs `0L`. Closing
it properly means threading an expected type through the translator, which is
the work this section avoided.

**OCaml rejects an out-of-range decimal literal but wraps a hex, octal, or
binary one:**

```ocaml
let a = 0xbf9cf968l    (* fine: -1080231576l *)
let b = 3214149992l    (* Error: Integer literal exceeds the range of
                          representable integers of type int32 *)
```

A Rust `u32` literal at or above 2^31 therefore has to be emitted in hex when
it was written in decimal. Rust source for this kind of code is nearly always
hex already (`0x811c9dc5`), and integer literals keep their source spelling
today, so the re-spelling is a fallback rather than the normal path.

## Worked example: `rust/hash`

The target output, verified to produce `0xbf9cf968` for `"foobar"` under OCaml
5.4.1. `u32` is escalated (`wrapping_mul`); `usize` and `u8` are not.

```ocaml
let fnv_offset_basis_32 = 0x811c9dc5l
let fnv_prime_32 = 0x01000193l

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
```

Everything outside the `Int32` calls comes from elsewhere: the `ref` cells from
`collect_refs`, the `while`, the guarded `match` on `Option`, `i := !i + 1` from
the `compound_assignment` desugar, and the byte-string treatment of `bytes` from
`design/string.md`.

## Levels

### Level 1: escalate on `wrapping_*` (done)

1. `collect_widths` with `wrapping_*` as the only trigger, plus the
   does-not-fit rule for `i64` / `u64`.
2. `map_type_name` in `ty.rs` consults the escalation set: `u32` -> `int32`.
3. Literal suffixes at the two typed positions (see [Literals](#literals)).
4. `translate_int_binary` emits `Int32.mul` / `logxor` / ... for escalated
   operands; `wrapping_*` maps to the plain `Int32` operation, since `Int32`
   wraps by definition and Rust's non-wrapping `*` on an escalated type is the
   same call.
5. `syn::Expr::Cast` -> `translate_int_cast`, which was `Expression::Todo`.

### Level 2: signedness (done)

The four unsigned operators from [Signedness](#signedness), selected from the
operand's Rust type. Landed with level 1 rather than after it: the signedness is
already in hand at every one of those sites, and the alternative was shipping a
lowering known to be wrong for values at or above 2^31. **Unexercised** — no
fixture divides, compares, or right-shifts an escalated value.

### Level 3: the remaining width observations

The `leading_zeros` / `lognot` triggers, which is what `research/isqrt.md`
would need from this backend.

`rotate_*` was listed here too, and `research/random.md` now argues it out: an
`int32` rotate is `Int32.shift_left x 32` at a zero rotation, which OCaml leaves
unspecified, while the unescalated `int` form is well defined at every rotation
because `lsl` is defined up to `Sys.int_size`. Escalating on `rotate_*` would
therefore need a guard that not escalating does not.

## Test

| Path | Role |
|------|------|
| `rust/hash`, `ml/hash` | golden pair |

`rust/hash` emits the [worked example](#worked-example-rusthash) above verbatim,
with no `TODO` left, and `test_test.sh hash` passes on Rust, Zig, and OCaml
alike. Two gaps stood between the integer work and the fixture, neither of them
about integers, and both are now closed: `&str` / `.as_bytes()` by
`design/string.md`, and a `match` in a `let` initializer by a `print/ml.rs`
layout change.

`rust/random` would be the second fixture, and it exercises the `int64` half
(verified informally: `Int64.mul`, `shift_right_logical`, and the masked
`u64 as u32` cast all emit correctly). Of its integer blockers, `rotate_right`
is [level 3](#level-3-the-remaining-width-observations) and the struct-literal
`0L` is in [Literals](#literals); `wrapping_shl` is simply absent from
`WRAPPING`, and is not a plain `shift_left`, since Rust masks the shift amount
by the width.

Its non-integer blockers were the definition order of the `impl` block
(`design/recursion.md`) and associated consts, both since fixed.

A `wrapping_*` on a *field* did not dispatch either, for an unrelated reason
that is now fixed: `expr_type` had no `syn::Expr::Field` arm. A field use
resolves to the field's own symbol, which is crate-local and carries
`state: u64`, so the arm reads it with `type_at` — widened to accept
`Kind::Field` — without resolving the base expression at all.

Behavioral parity is checked the usual way, `dune test` on the emitted tree
against `cargo test` on the input.

## Alternatives considered

* **Everything to `Int32` / `Int64`, chosen by the Rust type alone.** Faithful
  and uniform, and it needs no analysis pass. Rejected because `i32` is Rust's
  default integer type, so nearly every ordinary loop counter and arithmetic
  expression in every fixture would become `Int32.add` — the opposite of the
  backend's goal of idiomatic OCaml.
* **Escalate the unsigned types only** (`u32` -> `int32`, `i32` -> `int`), on
  the theory that Rust code choosing an unsigned type is signalling that it
  cares about the bit pattern. Cheap and needs no analysis, and it happens to
  give the right answer for `rust/hash`. Rejected as a proxy: it is wrong in
  both directions (`u8` byte values and `usize` lengths are unsigned and want
  `int`; a wrapping `i32` PRNG wants `int32`), and the property it approximates
  — the width being observed — is directly detectable.
* **Convert at the operation rather than at the type**, leaving everything
  `int` and spelling a wrapping multiply as
  `Int32.to_int (Int32.mul (Int32.of_int a) (Int32.of_int b))`. Keeps the
  representation uniform, but `Int32.to_int` sign-extends, so the round trip
  does not preserve `u32` values above 2^31 without a mask — and the output is
  unreadable at exactly the place the reader most needs to follow the
  arithmetic.
* **A `Uint32` module of our own** wrapping `int` with masking after each
  operation. Correct and uniformly unsigned, but it is a runtime library
  shipped with the output, which the backend has so far avoided entirely — the
  stdlib references it emits are a closed set. Reconsider only if unsigned
  semantics turn out to be pervasive rather than local.
