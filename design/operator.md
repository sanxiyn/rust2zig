# Operators

How Rust's arithmetic, bitwise, shift, and logical operators are translated,
what happens on overflow, and what the `checked_` / `saturating_` /
`overflowing_` families would take. **The ordinary operators and
`wrapping_add` / `wrapping_sub` / `wrapping_mul` / `wrapping_shl` /
`rotate_right` are implemented in all three backends**; nothing from the other
three families is.

`==` and `!=` are `design/equality.md`, which is a type-driven question the rest
of these are not. How an integer is *represented* is `design/integer.md`
(OCaml) and `doc/lisp.md`'s integer section (Common Lisp); this document is
about what the operators do once the representation is settled.

## Overflow is the whole story

Rust's `+` panics on overflow in debug and wraps in release, and
`wrapping_add` is how a program says it meant the second one. The translation
targets **debug semantics**, because that is the checked one and the one
`cargo test` -- and therefore `test_test.sh` -- actually runs.

What each target does with `250u8 + 10`:

| Target | Result |
|---|---|
| Rust (debug) | panic: attempt to add with overflow |
| Zig | **panic: integer overflow** |
| Common Lisp, undeclared | `260` -- integers promote to bignums; there is no overflow |
| Common Lisp, declared `(unsigned-byte 8)` | **SBCL: `TYPE-ERROR`. ECL: `260`** |
| OCaml | silently wraps at 63 bits (`max_int + 1` is negative) |

Only Zig reproduces Rust's behavior. Common Lisp has no overflow to reproduce,
so `doc/lisp.md` re-imposes one through slot and variable type declarations --
which is why that document calls declarations "the overflow check", and why the
`test_test.sh` matrix runs both SBCL and ECL: the check exists on one
implementation and not the other. OCaml has neither, which is the premise of
`design/integer.md`'s escalation rule.

This ordering -- Zig exact, SBCL approximate, ECL and OCaml absent -- is worth
holding onto, because every family in [Forward](#forward-checked-saturating-overflowing)
below is a different way of asking the same question.

## The tables

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `a + b`, `a - b`, `a * b` | same | `(+ a b)`, `(- a b)`, `(* a b)` | same |
| `a / b` | `a / b`; **signed needs `@divTrunc`** | `(truncate a b)` | `a / b` |
| `a % b` | `a % b`; signed -> `@rem(a, b)` | `(rem a b)` | `a mod b` |
| `a & b`, `\| `, `^` | same | `logand`, `logior`, `logxor` | `land`, `lor`, `lxor` |
| `a << b` | `a << @intCast(b)` | `(ash a b)` | `lsl` |
| `a >> b` | `a >> @intCast(b)` | `(ash a (- b))` | `asr` / `lsr` by signedness |
| `a && b`, `a \|\| b` | `and`, `or` | `(and …)`, `(or …)` | `&&`, `\|\|` |
| `!b` (bool) | `!b` | `(not b)` | `not b` |
| `!x` (integer) | **unhandled** | `(lognot x)` | `lnot x` (unverified) |
| `-x` | `-x` | `(- x)` | `-x` |
| `a += b` | `a += b` | `(incf a b)`, else `setf` | `compound_assignment` pass |
| `a.wrapping_add(b)` | `a +% b` | `(ldb (byte N 0) (+ a b))` | `Int32.add` after escalation |
| `a.rotate_right(n)` | `std.math.rotr(T, a, n)` | shift pair | -- |

Two entries in that table are gaps rather than translations; both are in the
[holes](#the-holes-ranked).

## Division and remainder are three different problems

* **Common Lisp** has neither operator. `/` on integers is exact and yields a
  ratio (`(/ 7 2)` is `7/2`), so Rust's truncating division is `truncate`. And
  `mod` floors where `rem` truncates -- verified, `(rem -7 2)` is `-1` and
  `(mod -7 2)` is `1` -- so Rust's `%` is `rem`. Picking `mod` would be wrong
  on exactly the negative operands a test rarely covers.
* **Zig** refuses both on signed integers:
  `division with 'i32' and 'i32': signed integers must use @divTrunc,
  @divFloor, or @divExact`. `%` is handled (`rem_is_signed` -> `@rem`); `/` is
  not.
* **OCaml** needs nothing. `(-7) / 2` is `-3` and `(-7) mod 2` is `-1`, both
  truncating toward zero exactly as Rust does. This is the only row in this
  document where OCaml is the target that requires no adaptation and both
  others do.

Division by zero fails everywhere -- Rust panics, Zig panics, Common Lisp
signals `DIVISION-BY-ZERO`, OCaml raises `Division_by_zero` -- so nothing is
silently wrong, and no backend does anything special about it.

## Shifts

* **Zig types the shift amount** by the width of the value being shifted, so
  every non-literal amount is wrapped in `@intCast` (`shift_amount`). A literal
  is exempt, being comptime-known.
* **Zig's `<<` discards the bits shifted out** rather than trapping: `200 << 1`
  on a `u8` is `144`, verified, with no panic. Two consequences. It is why
  `wrapping_shl` maps to a plain `<<`. And it is the one place the Zig backend
  does *not* reproduce Rust's debug behavior, since Rust panics when the shift
  amount is at or past the width -- the two disagree only there, not on the
  discarded bits.
* **Rust's `>>` is arithmetic on signed and logical on unsigned.** Common Lisp's
  `ash` with a negative amount is arithmetic (`(ash -8 -1)` is `-4`), which is
  right for signed and also right for unsigned, since a Common Lisp integer has
  no width and an unsigned value is non-negative. OCaml has two operators and
  the choice is real: `-8 asr 1` is `-4` while `-8 lsr 62` is `1`.
  `design/integer.md`'s signedness section is where that decision lives.
* **`ash` grows without bound**: `(ash 1 64)` is a bignum, not `0`. So a shift
  that Rust would truncate needs the `ldb` mask that wrapping arithmetic uses.

## Logical operators, and `!`

`&&` and `||` short-circuit in every target and translate directly. Zig accepts
`a and b or c` without parentheses, so no special handling is needed there.

Rust's `!` is **two operators sharing a token**: logical negation on `bool` and
bitwise complement on integers. Common Lisp's `translate_unary` asks
`expr_type` and emits `not` or `lognot` accordingly. The Zig backend does not
ask: it always emits `Node::BoolNot`, so `!x` on an integer produces
`expected type 'bool', found 'u8'`. Zig's spelling is `~`.

## Compound assignment

Three different answers, for three different reasons:

* **Zig** has the operators, including the wrapping forms, so `+=` is `+=`. The
  fold `x = x.wrapping_mul(y)` -> `x *%= y` exists because Rust has no wrapping
  compound assignment; it is restricted to path and field places, where
  evaluating the place twice is harmless.
* **Common Lisp** has `incf` and `decf` and nothing else, so `+=` and `-=` use
  them (with the `1` case shortening to `(incf x)`) and the rest fold into
  `(setf x (op x y))` in the translator -- not a desugar pass, because the
  target's asymmetry is what forces it.
* **OCaml** has no compound assignment at all, so `compound_assignment` is an
  OCaml-only desugar pass that rewrites `x += y` into `x = x + y` while it is
  still Rust, and the `ref` cell spelling (`x := !x + y`) falls out of the
  normal assignment path.

## Precedence: nothing is parenthesized

No backend's printer inserts parentheses. The only parentheses in the output are
the ones the Rust source wrote, carried through as `syn::Expr::Paren`. That is:

* **safe in Common Lisp by construction** -- the output is prefix;
* **safe in Zig by coincidence** -- Zig's precedence table has the same order
  Rust's does (`* / %` above `+ -` above shifts above `&` above `^` above `|`
  above comparisons above `and` above `or`);
* **unsafe in OCaml**, where `land` / `lor` / `lxor` sit at *multiplicative*
  precedence and `lsl` / `lsr` / `asr` above even that, while Rust puts all of
  them *below* `+`.

| Rust source | Rust value | Emitted OCaml | OCaml value |
|---|---|---|---|
| `8 & 4 + 1` | `0` | `8 land 4 + 1` | **`1`** |
| `1 << 2 + 3` | `32` | `1 lsl 2 + 3` | **`7`** |
| `8 \| 4 * 2` | `8` | `8 lor 4 * 2` | **`24`** |

These are silent wrong answers, not compile errors. Nothing in the fixtures
reaches them: the only bitwise code on the OCaml side is `ml/bitset`, whose
`self.data land 1 lsl bit` mixes `land` with `lsl`, and there the two languages
happen to agree (`data land (1 lsl bit)` either way).

One caveat on scope: `src/print/ml.rs` is not in this checkout, so whether that
printer parenthesizes anything cannot be checked here. What the emitted
`ml/bitset` shows is that it does not parenthesize *everything*, and the table
above is what the exposure looks like.

The fix is the ordinary one -- print with precedence, parenthesizing a child
whose precedence is lower than its parent's. Only OCaml needs it, but making
all three printers do it removes the reliance on a coincidence that a future
target may not share.

## Wrapping arithmetic

Dispatch is by moniker (`core::num::wrapping_add`, …), not by method name, so a
user-defined `wrapping_add` on a crate type does not get hijacked -- the same
discipline as every other intrinsic.

| | how it wraps |
|---|---|
| Zig | native operators: `+%`, `-%`, `*%`. Exact match, one token each |
| Common Lisp | compute exactly, then mask: `(ldb (byte 64 0) (+ a b))`. Needs the width, which is what `unsigned_bits` supplies |
| OCaml | a *representation* change, not an operator: the type escalates to `Int32.t` / `Int64.t` and `Int32.add` wraps natively (`design/integer.md`) |

The Common Lisp form is the interesting one: it is the only place where the
absence of a width becomes a cost rather than a convenience, and it is why
`unsigned_bits` exists at all. It also bounds what is supported -- `ldb` yields
an unsigned bit pattern, so a *signed* wrapping operation would need a
sign-extension step that is not written. `unsigned_bits` answers only for
unsigned types, so `i32::wrapping_add` falls through to the ordinary method-call
path and leaves a marker rather than a wrong answer.

Zig's `wrapping_shl` -> `<<` is above; `rotate_right` -> `std.math.rotr(T, x, n)`
is the one intrinsic that needs `T` spelled out, resolved from the receiver, and
an unresolvable receiver leaves a plain method call rather than a wrong type
argument.

## Forward: checked, saturating, overflowing

Rust has four families over each arithmetic operation. One is implemented; the
other three are not, and they are worth planning together because **Zig has a
native form for every one of them**, which no other feature in this project can
say.

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `a.wrapping_add(b)` | `a +% b` | `(ldb (byte N 0) (+ a b))` | `Int32.add a b` |
| `a.saturating_add(b)` | `a +\| b` | clamp helper | clamp helper |
| `a.overflowing_add(b)` | `@addWithOverflow(a, b)` -> `.{ value, bit }` | value plus a range test | helper |
| `a.checked_add(b)` | `@addWithOverflow` + `if (ov == 1) null else v` | value or `nil` | helper returning `option` |
| `a.strict_add(b)` | `a + b` | `(+ a b)` under its declaration | -- |

### Zig facts

| # | Case | Result |
|---|---|---|
| 1 | `250 +\| 10` on `u8` | `255` -- saturating add is an operator |
| 2 | `200 <<\| 1` on `u8` | `255` -- saturating shift too |
| 3 | `@addWithOverflow(250, 10)` on `u8` | `.{ 4, 1 }` -- value and overflow bit |
| 4 | `250 + 10` on `u8` at runtime | panic: integer overflow |
| 5 | `@divTrunc(-7, 2)` | `-3`, matching Rust's `/` |

Fact 3 is the useful one: Zig's builtin returns a two-element tuple, and Rust's
`overflowing_add` returns `(T, bool)`. The shapes are the same, so the
translation is a rename plus the tuple support the backend already has for
`rust/geometry`'s multi-value returns.

### What each family costs

* **`saturating_*` is the cheapest.** One operator in Zig (facts 1 and 2), and
  elsewhere a clamp against the type's bounds. Do it first.
* **`checked_*` needs no new representation.** It returns `Option<T>`, and each
  backend already has one: Zig's `?T`, Common Lisp's `nil` erasure, OCaml's
  `option`. It is a lowering onto machinery that exists.
* **`overflowing_*` needs no new representation either**, tuples being
  supported, and in Zig it is nearly the identity.
* **`strict_*` is already the default** in debug semantics, so it is a no-op in
  Zig and needs nothing anywhere.

### The prerequisite the non-Zig backends share

All three families need **the bounds of a Rust integer type** -- `u8` saturates
at `255`, `i32` at `2147483647`. Nothing computes that today; `unsigned_bits`
and `int_bits` are half of it (a width), and the missing half is turning a
width and a signedness into a min/max pair. One small function unblocks
saturating and checked in both non-Zig backends, and it belongs next to
`int_bits` in `src/translate/ty.rs` where both backends can reach it.

### One Common Lisp constraint, from the overflow table

The natural shape -- compute, then fix up -- interacts with declarations. The
computation itself is always safe, since Common Lisp promotes to a bignum. What
is not safe is *storing* an unclamped intermediate into a declared place: the
verified `TYPE-ERROR` in the overflow table above came from assigning `260` to a
variable declared `(unsigned-byte 8)` on SBCL.

So a saturating or checked helper must return an already-clamped value and the
translator must not emit a `setf` of the raw sum first. This falls out naturally
if the helper is a function call, and it rules out the tempting rewrite of
`x = x.saturating_add(y)` into `(setf x (+ x y))` followed by a clamp.

## Test

| Path | Role |
|------|------|
| `rust/hash` | `wrapping_mul` in a loop -- the fixture that forces wrapping in every backend, and the trigger for `design/integer.md`'s escalation |
| `rust/random` | `wrapping_add`, `wrapping_mul`, `rotate_right`, and shifts by non-literal amounts (the `@intCast` path); its assertion pins an exact PCG output, so a wrong wrap fails loudly |
| `rust/bitset` | `&`, `\|`, `^`, `<<`, and the `x & (1 << b) != 0` bit test that Common Lisp rewrites to `logbitp` |
| `rust/gcd`, `rust/div` | `%` and `/` on unsigned values, and `!=` against zero |
| `rust/calc` | The nearest thing to checked arithmetic in the tree, and instructive for not being it: `add` computes `a + b` and compares against a *domain* limit (`LIMIT: u32 = 1000`), so its `Error::Overflow` has nothing to do with `u32`'s bound. `checked_add` is what it would be if the bound were the type's |

No fixture uses signed division, `!` on an integer, a signed wrapping
operation, or any of `checked_` / `saturating_` / `overflowing_`. The first two
are compile errors in the emitted Zig, so a fixture would fail loudly the day
one is added.

## The holes, ranked

### 1. Signed division emits invalid Zig

`translate_binary` special-cases `%` for signed operands and not `/`. Zig
rejects both, so `a / b` on two `i32`s emits `a / b` and the emitted file does
not compile.

This is the smallest fix in this document: `rem_is_signed` already answers the
question, and `@divTrunc` is the counterpart to `@rem` -- verified to match
Rust's truncation on negatives (fact 5). The two should be one helper.

### 2. `!x` on an integer emits invalid Zig

Above. The Common Lisp backend already does the type test that the Zig backend
needs; the Zig spelling is `~`.

### 3. OCaml precedence

Above. The only silent hole here, and the only one that needs a printer change
rather than a translator one.

## Not implemented yet

1. `@divTrunc` for signed `/` (hole 1).
2. `~` for integer `!` (hole 2).
3. Precedence-aware printing, for OCaml (hole 3).
4. `saturating_*`, `checked_*`, `overflowing_*`, in that order.
5. Signed wrapping arithmetic in Common Lisp (`ldb` plus sign extension).
6. Float arithmetic -- `research/float.md` is the plan, and none of the
   operators above have been checked against `f32` / `f64` behavior.

## Not planned

* Release-mode overflow semantics. Rust's release build wraps, the translation
  targets debug, and a target that panics is the one that finds bugs. A program
  that means to wrap says `wrapping_add`.
* `i128` / `u128` arithmetic. No target has a native 128-bit integer -- Zig
  does, in fact, but the other two would need bignum or pair representations
  that nothing has designed.
* Operator overloading (`impl Add for T`). `design/struct.md` records that trait
  impls other than `Drop` are emitted as ordinary methods; routing `+` to one is
  the same unimplemented dispatch as routing `==` to a hand-written
  `PartialEq`.
