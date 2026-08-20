# Common Lisp: decisions from `rust/random`

The decisions behind translating `rust/random` (PCG-XSH-RR, extracted from
`oorandom`), and the alternatives they were chosen over. `doc/lisp.md` states
the resulting rules; the code is canonical about how they are carried out.
Function names here are for looking that up, nothing more.

`random` is what settled these because its test asserts a *specific number*:

```rust
assert_eq!(2891073575, r1.rand_u32());
```

Every other Lisp fixture checks a shape or a small identity. This one checks
that a full PCG round -- two `wrapping_mul`s, a `wrapping_add`, two shifts, an
xor, two narrowing casts, and a rotate -- is bit-exact against Rust. A masking
rule off by one bit fails loudly instead of passing by luck, which is why the
integer rules were pinned here rather than on a smaller example using the same
operators.

## Constants are earmuffed

`Rand32::DEFAULT_INC` becomes `+rand32-default-inc+` (`const_name`,
`const_named`). Two decisions in that name.

The type prefix is the rule methods and variants already follow: Common Lisp has
no per-type namespace, so two types' `MULTIPLIER` have to be told apart at the
top level.

The earmuffs are not convention. A `defconstant` name cannot be bound as a
variable -- `(let ((c 3)) ...)` signals `SIMPLE-PROGRAM-ERROR` on both
implementations -- so a plain `default-inc` would make that name unusable for
every later local. That is the trap `doc/lisp.md`'s *Reserved names* renames `t`
out of, except minted by the translator and one per constant. `+` cannot appear
in a translated Rust identifier, so an earmuffed name is disjoint from every
name a local can take and the collision cannot arise. Same shape of argument as
keywords for a payload-free enum: pick a spelling the rest of the namespace
cannot reach.

No declaration accompanies a constant. The value is a literal the compiler
already sees and cannot be assigned, so there is nothing for one to catch.

**Known limit.** `defconstant` requires an `eql` value on re-evaluation, so a
*string* constant is not safe to load twice: SBCL signals `DEFCONSTANT-UNEQL`,
ECL accepts it. Every constant today is an integer and a translated file loads
once. `defparameter` is the spelling for values that are not `eql`-comparable,
and `rust/hash`'s `FOOBAR: &str` is the first that will need it.

## `Self` is a desugar pass, not a translator lookup

`Self` resolution lives in `src/desugar/self_type.rs`, not in the translator.

It began as the latter: a `RefCell<Option<String>>` set and cleared around each
`impl`, and a `resolve_self` call at each of the seven sites that read a type
name. `Self { .. }` was broken because `struct_named` had been written without
one, and the fix-in-place would have been an eighth call at a place someone
could forget again. The pass removes the state and every call site, and makes
forgetting impossible: `Self` no longer exists by the time emission runs.

It is a fair desugar by the project's own test -- the rewrite leaves valid Rust,
and the impl block carries its own type, so no SCIP is consulted.

**Zig declines it.** Zig has a `Self` of its own: `zig/bitset.zig` emits
`fn withCapacity(bits: usize) Self` against a `const Self = @This()`. Resolving
the name away would make that output worse, which is why the pass is per-backend
rather than universal.

**The span is kept.** `README.md`'s discipline gives synthetic nodes a
`call_site` span so SCIP can never resolve there; this pass reuses the original
`Self` ident's span instead. That is the safe direction: rust-analyzer records
an occurrence of the impl's type at exactly that range, so a query answers with
the type the name now spells, where a `call_site` span would merely stop
answering.

## `let _ = e;` is `e`, and opens no scope

A wildcard binds nothing, so what remains is the expression, evaluated for its
effect -- and it has to be kept: `random` discards two `rand_u32()` calls this
way, each advancing the state the asserted number depends on.

The subtlety is what follows it. A tuple pattern also cannot join a `let` group,
but its `multiple-value-bind` opens a scope, so the remaining statements nest
inside. A wildcard opens none, so they stay at the same level. Treating the two
alike would indent the rest of the function once per discarded call.

**Known limit.** A block whose last statement is `let _ = e;` is `()` in Rust
but yields `e`'s value here. Functions are covered by the trailing `nil` that
keeps their `declaim` honest, so only a block used as a value could show it, and
none does.

## Wrapping arithmetic is `ldb`

`a.wrapping_mul(b)` is `(ldb (byte 64 0) (* a b))` (`translate_wrapping`): the
operation on unbounded integers, cut back to the type's width. Checked against
Rust rather than reasoned about -- `0xFFFFFFFF * 0xFFFFFFFF` masked to 32 bits
is 1, `u64::MAX + 1` masked to 64 is 0, and the first PCG step agrees to the
digit.

**Unsigned only.** `ldb` yields a value's unsigned reading, so a signed receiver
would turn `(-1i32).wrapping_add(0)` into 4294967295. Sign extension on top
would fix it and nothing asks for one yet, so `unsigned_bits` filters to `u*`
and a signed receiver leaves a marker. `usize` is excluded with them: its width
is the target's, and a `fixnum` is 62 bits on this SBCL and 61 on this ECL.

**An unresolvable width leaves a marker rather than a guess.** A wrong width is
a wrong number, not a compile error.

**The receiver's type does not come from SCIP.** These methods belong to `core`,
and the index carries an occurrence naming `num/impl#[u64]wrapping_mul().` with
no `symbol_information` behind it, so there is no signature to read a return
type from. It does not need one: a wrapping operation has its receiver's type by
definition, and saying so in `expr_ty` is what keeps a *chain* of them typed,
which `oldstate.wrapping_mul(M).wrapping_add(self.inc)` requires. Parsing the
impl type out of the moniker, the way `binary_type_at` does, would work equally
well and needs more machinery.

## `rotate_right` expands into shifts

The one operation with no Common Lisp counterpart -- there is `ash`, and no
rotate -- so it becomes what it is made of (`rotate_right`):

```lisp
(logior (ash x (- n)) (ldb (byte 32 0) (ash x (- 32 n))))
```

Only the left shift is masked; `x >> n` cannot leave the width it started
inside. At `n = 0` the left shift carries the value clear of the mask and
contributes nothing, leaving `x`. Swept against Rust over eight cases including
`0` and `31`, identical on both implementations.

**Inline, not a helper `defun`.** A helper would evaluate each operand once and
lift the restriction below, at the price of emitting a function the Rust source
has no counterpart for. Between a duplicated variable and an invented
definition, the duplication is the smaller departure. The cost is that the
expansion names each operand twice, so it is restricted to operands without
effects (`is_place` -- a variable or a field of one, the same admission the Zig
backend makes for `translate_wrapping_assign`), and anything else leaves a
marker.

**Known limit.** Rust reduces the amount modulo the width and this does not, so
they part company for `n > width`. `random`'s amount is `oldstate >> 59`, hence
0-31 by construction. Recorded rather than fixed with a `mod` no input needs,
which is the call `README.md` already makes for `wrapping_shl` in Zig.

## A cast is a mask, or nothing

Unbounded integers make the two directions asymmetric (`translate_cast`):
narrowing a `u64` to `u32` is `(ldb (byte 32 0) x)`, and widening a `u8` to
`u32` is the operand untouched, the value already *being* that integer. Zig
needs a builtin either way, `@truncate` or `@as`, which is what makes this the
shorter rule. Both widths must be known, so an unresolvable operand leaves a
marker, as does `usize` at either end, for the reason above.

Recognition is split from expansion for both this and the rotation, so an
operation the translator knows but cannot express leaves its own marker rather
than the generic method-call one. "I know what this is and declined" is a
different report from "I do not know what this is."

## What carries over

`rust/hash` reuses the constants, the `ldb` wrapping, and the narrowing cast
unchanged. What it still needs is a `match` on a `core` `Option` with guards,
and `&str` / `as_bytes` -- and the second is a real design question, not a gap:
Common Lisp strings are character vectors rather than byte vectors, so `&str`
and `&[u8]` cannot share a representation the way they do in OCaml.

An observation worth keeping: none of these operations needed a new moniker.
`core::num::wrapping_*`, `rotate_right`, and `as_bytes` were already in
`src/translate/moniker.rs`, put there by the Zig backend. The suffixes are
rust-analyzer's spellings rather than anything derivable, so the second backend
to want one gets it for free.
