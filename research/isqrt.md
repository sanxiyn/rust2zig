# Integer square root

Status and roadmap for translating a real leaf crate end to end, using
`integer-sqrt` as the first `.crate`-shaped target. The point is not the
algorithm; it is to drive the first genuinely external, un-curated input
through the pipeline and surface the gaps it hits — the way the 40-line
`bitset` extract found three (shift typing, non-void discard, `assert!`).

## Goal

Minimum viable `.crate` translation: hand rust2zig a published crate and get
buildable Zig. Scope for the first pass is deliberately narrow:

* **Leaf** — no dependencies.
* **No macros** — no derive, no attribute, no function-like macros.
* **No std / no alloc** — `#![no_std]`, no heap.

This is not empty. Even under these constraints a real crate exercises code
the curated examples never did.

## `.crate` ingestion is nearly free

A `.crate` is a gzipped tarball that unpacks to `<name>-<version>/`
containing a Cargo package (`Cargo.toml` + `src/`) — exactly the input
`main.rs` already takes. So the ingestion path is:

```
untar <name>-<version>.crate  ->  existing package-directory code path
rust-analyzer scip .          ->  whole-crate SCIP index
```

The leaf constraint is what makes the SCIP step tractable with no network:
there is no dependency graph to resolve. The word ".crate" in the goal does
less work than it looks; the real content is "multiple files + whatever a
real leaf throws at us."

## The victim crate

`integer-sqrt` (`derekdreery/integer-sqrt-rs`). Its real form already fails
our constraints in the predicted way — the same lesson as "bitset is not
fixedbitset":

```rust
impl<T: num_traits::PrimInt> IntegerSquareRoot for T { ... }
```

* **Not leaf**: depends on `num-traits`.
* **Generic over a foreign trait** `PrimInt`, using `T::zero()`, `T::one()`,
  `.leading_zeros()`, `.unsigned_shl/shr()`, `.cmp()`, `core::cmp::Ordering`.

So even the simplest real crate on crates.io has a dependency and a generic
blanket impl. We extract the essence and drop the deps/generics, the way
`bitset` was extracted from `fixedbitset`.

## De-generic'd fixture (level 1)

Faithful to the algorithm, concrete `u64`, no trait, no deps:

```rust
pub fn integer_sqrt(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }
    let shift: u32 = (63 - value.leading_zeros()) & !1;
    let mut bit: u64 = 1u64 << shift;
    let mut n = value;
    let mut result: u64 = 0;
    while bit != 0 {
        if n >= result + bit {
            n = n - (result + bit);
            result = (result >> 1) + bit;
        } else {
            result = result >> 1;
        }
        bit = bit >> 2;
    }
    result
}
```

## Gaps

Grounded against the currently handled `syn::Expr` / `syn::Item` sets.

### New (this fixture forces them)

1. **`value.leading_zeros()` -> Zig `@clz(value)`.** A core integer intrinsic
   exposed as a method. Same shape as the `.len()` -> `.len` special-case:
   match the SCIP moniker (`core::num`...`leading_zeros`) and emit the
   builtin. Precedented, clean.
2. **`& !1` — bitwise-NOT on an integer.** Today `!` is handled only as
   boolean not (`!b.contains(...)`). Rust overloads `!` for bool and int;
   **Zig splits them: `!` for bool, `~` for int.** Needs the same
   type-driven dispatch as deref-insertion and shift typing — ask SCIP
   whether the operand is bool or integer, emit `!` vs `~`. New, but slots
   into existing machinery.

### Already solved (bitset dividend)

3. `1u64 << shift`, `>> 1`, `>> 2` — covered by the `@as`-typed-literal +
   `@intCast`-shift-amount work (commit `shift`).
4. Top-level `pub fn`, `while`, `if`/`else`, early `return`, `let mut`
   shadowing — all handled.

Net: a **2-gap** level-1 fixture (`@clz`, `!`-vs-`~`), riding on shift work
already shipped.

## Proposed Zig output (level 1)

Hand-written target; `@clz` and `~` marked.

```zig
const std = @import("std");

fn integerSqrt(value: u64) u64 {
    if (value == 0) {
        return 0;
    }
    const shift: u32 = (63 - @clz(value)) & ~@as(u32, 1);   // @clz, ~
    var bit: u64 = @as(u64, 1) << @intCast(shift);
    var n = value;
    var result: u64 = 0;
    while (bit != 0) {
        if (n >= result + bit) {
            n = n - (result + bit);
            result = (result >> @intCast(1)) + bit;
        } else {
            result = result >> @intCast(1);
        }
        bit = bit >> @intCast(2);
    }
    return result;
}
```

Details to confirm against the translator during implementation:

* `@clz(value)` on `u64` yields `u6`; the surrounding `63 - _` and `& _`
  arithmetic must land on a consistent width (likely `u32` via the annotated
  `shift`). Watch for Zig integer-width errors here — a plausible third gap.
* Shift-amount `@intCast` on literal amounts (`>> 1`) may be simplifiable,
  but keep it uniform with the existing shift lowering for now.

## The trait dial (defer to level 2)

The free-function fixture drops the trait. Keeping `impl IntegerSquareRoot
for u64` instead forces the oldest deferred unknown in the project
(trait / dispatch translation, flagged 2026-04-06):

* Zig cannot attach methods to a primitive (`u64`), so `n.integer_sqrt()`
  must lower to a free function `integerSqrt(n)`.
* The default method (`integer_sqrt` calling `integer_sqrt_checked().expect(...)`)
  drags in `Option::expect` -> `orelse @panic(...)`.

Do **not** let this ride in on the coattails of the intrinsic-methods
exercise. Level 1 is pure free function; the trait is a deliberate,
separately-scoped level 2.

## Levels

### Level 1: free-function `u64` (this doc)

* Fixture: `rust/isqrt` -> `zig/isqrt.zig`.
* Gaps: `@clz`, `!`-vs-`~` (type-driven), plus possible integer-width
  fallout around `@clz`.
* Establishes the first real `.crate`-shaped end-to-end path.

### Level 2: user-defined trait on a primitive

* Keep `pub trait IntegerSquareRoot` + `impl ... for u64`.
* Trait method on primitive -> free function; default method body;
  `Option::expect` -> `orelse @panic`.
* First real trait-translation data point.

### Level 3+: toward the real crate

* Blanket `impl<T: PrimInt>` -> generic-as-comptime over an integer bound.
* `num-traits` dependency -> either translate the dep (leaf-of-leaf) or
  map `PrimInt` intrinsics (`leading_zeros`, `unsigned_shl/shr`, `zero`,
  `one`) to Zig builtins directly.
* `core::cmp::Ordering` / `.cmp()`.
* Multi-file / `mod` / `use` once a fixture actually spans files.

## Test

| Path | Role |
|------|------|
| `rust/isqrt`, `zig/isqrt.zig` | Level 1 golden pair |

* `#[test]`: check a few values (`0`, `1`, `15 -> 3`, `16 -> 4`,
  `u64::MAX`-ish) with `assert_eq!` in Zig-expected `(expected, actual)`
  order.
* Regenerate SCIP with `./build_index.sh isqrt`.
* Golden compare via `./test.sh isqrt`; behavioral parity via
  `./test_test.sh isqrt`.

## Next steps

1. Lay down `rust/isqrt` + `zig/isqrt.zig` level-1 golden pair.
2. Implement `@clz` (moniker special-case) and `!`-vs-`~` (type-driven).
3. Resolve any `@clz` integer-width fallout.
4. Then level 2 (trait on primitive).
