# PCG random

**Implemented (levels 1-2).** `rust/random` -> `zig/random.zig` passes both
suites. The fixture that landed is `Rand32` with `rand_u32` plus the `new` /
`new_inc` constructors, so both gaps below are done and level 2 came along with
them. Level 3 (`Rand64` over `u128`) and level 4 (`rand_range`) are untouched.
The extras this fixture forced beyond the two predicted gaps were associated
`const` items in an `impl` block and `wrapping_shl`; see the README notes.

Status and roadmap for a stateful-struct `.crate` target: `oorandom`
(PCG-XSH-RR). Shares MVP scope and `.crate` ingestion notes with
[isqrt.md](isqrt.md). This is the natural "next size up" from
[hash.md](hash.md): a struct with inherent methods running a real algorithm,
bitset-shaped but arithmetic-heavy. Companion targets: [hash.md](hash.md),
[isqrt.md](isqrt.md).

## Why this crate

* **No runtime dependencies**, `#![no_std]`, no alloc.
* No custom macros; only `#[derive(Copy, Clone, Debug, PartialEq, Eq)]` on
  the structs — strip the derive (not needed for the algorithm or a test),
  the same edit class as everywhere else.
* Inherent methods on a struct (`Rand32` / `Rand64`) — no traits to resolve.
* Adds two gaps beyond [hash.md](hash.md): **narrowing `as`** and the
  **`rotate_right` intrinsic** — and exercises **`u128`** via `Rand64`.

## Real source (essence)

```rust
pub struct Rand32 { state: u64, inc: u64 }

impl Rand32 {
    const MULTIPLIER: u64 = 6364136223846793005;
    const INCREMENT: u64 = 1442695040888963407;

    pub fn new(seed: u64) -> Self {
        let mut r = Rand32 { state: 0, inc: (Self::INCREMENT << 1) | 1 };
        r.rand_u32();
        r.state = r.state.wrapping_add(seed);
        r.rand_u32();
        r
    }

    pub fn rand_u32(&mut self) -> u32 {
        let oldstate: u64 = self.state;
        self.state = oldstate
            .wrapping_mul(Self::MULTIPLIER)
            .wrapping_add(self.inc);
        let xorshifted: u32 = (((oldstate >> 18) ^ oldstate) >> 27) as u32;
        let rot: u32 = (oldstate >> 59) as u32;
        xorshifted.rotate_right(rot)
    }
}
```

`Rand64` is the same shape over `u128` state. `rand_range` (uses
`core::ops::Range`) is out of scope for level 1.

## Gaps

Grounded against the handled `syn::Expr` / `syn::Item` sets. Builds on
[hash.md](hash.md) (wrapping arithmetic, `as` casts).

### New (beyond hash.md)

1. **Narrowing `as`: `(oldstate >> 27) as u32` from `u64` -> Zig
   `@truncate`.** Distinct from hash.md's widening `as`. `syn::Expr::Cast`
   must branch on source vs target width (from SCIP types): widen ->
   `@as` / `@intCast`, narrow -> `@truncate`. This fixture is where the
   narrowing arm gets built.
2. **`xorshifted.rotate_right(rot)` -> `std.math.rotr(u32, xorshifted,
   rot)`.** Another intrinsic-method special-case (like `.len()` -> `.len`,
   `leading_zeros` -> `@clz`, `wrapping_mul` -> `*%`), but this one maps to
   a `std.math` call rather than a builtin or operator. Match the SCIP
   moniker (`core::num`...`rotate_right`).

### Exercised (may already work)

3. **`u128`** state in `Rand64` — `ty.rs` was taught 128-bit types; this is
   the first fixture that actually uses one in arithmetic. Confirm literals
   and `wrapping_mul` on `u128` translate cleanly.
4. **Associated consts** `Self::MULTIPLIER` / `Self::INCREMENT` — check
   whether const items in an `impl` block and `Self::CONST` reference are
   handled; if not, this is a bonus gap (top-level / associated `const` was
   flagged as unhandled — `Item::Const`).

### Already solved

5. Struct definition + inherent methods, `&mut self` receiver, field
   access/assignment, shifts (`>>`), `^`, `let mut`, method calls on `self`
   — all handled (bitset / geometry2 / shift work).

## Proposed Zig output (level 1, `rand_u32` only)

```zig
const std = @import("std");

const Rand32 = struct {
    const Self = @This();

    const multiplier: u64 = 6364136223846793005;

    state: u64,
    inc: u64,

    fn randU32(self: *Self) u32 {
        const oldstate: u64 = self.state;
        self.state = oldstate *% multiplier +% self.inc;                 // *% , +%
        const xorshifted: u32 = @truncate(((oldstate >> @intCast(18)) ^ oldstate) >> @intCast(27)); // narrowing as
        const rot: u32 = @truncate(oldstate >> @intCast(59));            // narrowing as
        return std.math.rotr(u32, xorshifted, rot);                      // rotate_right
    }
};
```

Details confirmed during implementation:

* **Operator precedence** of `*%` / `+%` matches the Rust method chain: `*%`
  binds tighter than `+%`, so the emitted `oldstate *% multiplier +% self.inc`
  groups as the chain does and needs no added parentheses. The behavioral test
  is what actually pins this down.
* Associated const naming follows the existing top-level convention:
  `MULTIPLIER` -> `multiplier` via `screaming_to_camel`, use sites included.
* `@truncate` does resolve its result type from the emitted `const
  xorshifted: u32` annotation. This works because `let` bindings always carry
  an annotation; a narrowing cast in a position with no expected type would
  not have one to resolve against.

## Levels

### Level 1: `Rand32` + `rand_u32` (this doc)

* Fixture: `rust/random` -> `zig/random.zig`.
* Gaps: narrowing `as` -> `@truncate`; `rotate_right` -> `std.math.rotr`.
* Depends on [hash.md](hash.md) landing wrapping arithmetic + `as` first.

### Level 2: `new` + seeding

* Constructor with self-calls (`r.rand_u32()` for side effect — a non-void
  discard, already handled via the `discard non-void expression` work),
  `<<`, `|`, `wrapping_add`.

### Level 3: `Rand64` over `u128`

* Same algorithm, `u128` state. First real `u128` arithmetic fixture.

### Level 4: `rand_range`

* `core::ops::Range` argument, bounded-modulo reduction. Pulls in `Range`
  outside a for-loop and `u64`-widened multiply.

## Test

| Path | Role |
|------|------|
| `rust/random`, `zig/random.zig` | Level 1 golden pair |

* `#[test]`: fixed seed, assert the first few `rand_u32()` outputs match
  known PCG-XSH-RR vectors. `assert_eq!` in Zig `(expected, actual)` order.
* Regenerate SCIP with `./build_index.sh random`.
* Golden compare via `./test.sh random`; behavioral parity via
  `./test_test.sh random` (checks Zig produces the same stream).

## Next steps

Done: the golden pair, narrowing `as` -> `@truncate`, `rotate_right` ->
`std.math.rotr`, associated consts, and (unforeseen) `wrapping_shl` -> `<<`.
Level 2's `new` needed nothing new: the `let _ = rng.rand_u32();` discards
already worked.

Remaining here, in order of how much they buy:

1. Level 3 (`Rand64` over `u128`) — the point of it is confirming `u128`
   literals and arithmetic survive, which nothing has exercised yet.
2. Level 4 (`rand_range`), which pulls in `core::ops::Range` outside a
   for-loop.

Two limits of the cast work worth recording, neither reachable from this
fixture. A cast whose operand is a struct field (`self.state as u32`) does not
resolve, because `Scip::type_at` answers for variables and parameters but not
fields — it falls back to `@as`. And a cast that changes signedness needs
`@bitCast` (same width) or a `@truncate` of matching signedness (narrowing);
both currently emit `@as`. Each is a Zig compile error rather than a silent
miscompile, which is why neither blocks anything yet.
