# Floating point (truncf)

Status and roadmap for the floating-point / bit-cast axis, using `libm`'s
`truncf`. Shares MVP scope and `.crate` ingestion notes with
[isqrt.md](isqrt.md). This opens a number domain (f32/f64) untouched by
every prior fixture. Companion targets: [hash.md](hash.md),
[random.md](random.md), [isqrt.md](isqrt.md), [hasher.md](hasher.md).

## Why this crate

`libm` is `#![no_std]`, no alloc, no deps — the reference soft-float math
library. It is the natural probe for the one axis nothing else touches:
**floating-point values and reinterpreting their bits.**

As with `integer-sqrt`/`PrimInt` and `oorandom`, the *current* crate already
fails "concrete leaf": modern `libm` implements `trunc` generically over
`F: Float` (`x.exp_unbiased()`, `F::SIG_MASK`, `F::from_bits`, ...) — a
float analogue of the `num-traits` situation. So we take the classic
concrete `truncf` (the long-stable musl algorithm the generic version
replaced) as the fixture, the same extraction move as `bitset` from
`fixedbitset`.

## Real source (classic concrete `truncf`)

```rust
pub fn truncf(x: f32) -> f32 {
    let mut i: u32 = x.to_bits();
    let mut e: i32 = ((i >> 23) & 0xff) as i32 - 0x7f + 9;
    let m: u32;
    if e >= 23 + 9 {
        return x;
    }
    if e < 9 {
        e = 1;
    }
    m = (-1i32 as u32) >> e;
    if (i & m) == 0 {
        return x;
    }
    i &= !m;
    f32::from_bits(i)
}
```

(The real source also invokes `force_eval!(x + x1p120)` — a volatile read
that forces the FE rounding side effect. It is semantically a no-op for our
translation; edit it away.)

## Level 1 fixture

The function above verbatim (minus `force_eval!`), as `rust/float`.

## Gaps

Grounded against the handled `syn::Expr` / `syn::Item` sets. Builds on
[hash.md](hash.md) / [random.md](random.md) (`as` casts) and reuses the
`!`-vs-`~` dispatch from [isqrt.md](isqrt.md).

### New (this fixture forces them)

1. **`f32` type.** No floating-point type has appeared before. `ty.rs`
   handles integer widths and `usize`/`isize`; add `f32`/`f64` -> Zig
   `f32`/`f64`. Float literals too, though `truncf` has none.
2. **`x.to_bits()` -> Zig `@bitCast(x)` (f32 -> u32); `f32::from_bits(i)`
   -> `@bitCast(i)` (u32 -> f32).** The reinterpret-cast axis. These are
   intrinsic-method / associated-fn special-cases (moniker dispatch, like
   `.len()` / `@clz`) that both lower to Zig's single `@bitCast`, with the
   target type coming from context (the `: u32` annotation / the `f32`
   return). This is the load-bearing new capability.
3. **Signedness-reinterpret `as`: `-1i32 as u32` (i32 -> u32) and
   `... as i32` (u32 -> i32).** Same-width, signedness-only casts. Distinct
   from hash.md's widening (`@as`) and random.md's narrowing (`@truncate`):
   same-width signed<->unsigned is Zig `@bitCast` (or `@intCast` when the
   value is provably in range). `syn::Expr::Cast` grows a third arm.

### Reused

4. **`!m` (integer bitwise-NOT) -> `~m`.** The type-driven `!`-vs-`~`
   dispatch from [isqrt.md](isqrt.md); here the operand is `u32`.
5. **Shifts** `>>` on `u32` (unsigned, logical) and the intCast'd amount —
   the shift work from the `shift` commit. Note `-1i32 as u32 >> e` is a
   *logical* shift because the value is `u32`; confirm the signedness of
   the shift is driven by the (post-cast) operand type, not the literal.

### Already solved

6. Early `return`, `if`, `let mut`, `let m;` (deferred init then single
   assignment), integer literals, `&`, `>>` — handled.

## Proposed Zig output (level 1)

```zig
const std = @import("std");

fn truncf(x: f32) f32 {
    var i: u32 = @bitCast(x);                          // to_bits
    var e: i32 = @as(i32, @bitCast((i >> @intCast(23)) & 0xff)) - 0x7f + 9; // as i32 (signedness)
    if (e >= 23 + 9) {
        return x;
    }
    if (e < 9) {
        e = 1;
    }
    const m: u32 = @as(u32, @bitCast(@as(i32, -1))) >> @intCast(e);         // -1i32 as u32
    if ((i & m) == 0) {
        return x;
    }
    i &= ~m;                                            // !m
    return @bitCast(i);                                 // from_bits
}
```

Details to confirm during implementation:

* `(i >> 23) & 0xff` is `u32`; casting to `i32` is same-width so `@bitCast`
  is correct, but the value is provably small — `@intCast` would also work
  and read cleaner. Decide one rule for same-width signedness casts (prefer
  `@bitCast` for "reinterpret", `@intCast` for "value-preserving") and note
  it; `truncf` is a good place to pin the convention.
* `@bitCast` result type must be inferable — it is here from the `var i:
  u32` / `f32` return annotations. Ensure annotations are emitted at every
  `@bitCast` site.
* Zig `@bitCast` requires equal bit widths (f32<->u32 ok). Guard against
  emitting it for width-changing casts (those are `@as`/`@truncate`).

## Levels

### Level 1: `truncf` (this doc)

* Fixture: `rust/float` -> `zig/float.zig`.
* Gaps: `f32`; `to_bits`/`from_bits` -> `@bitCast`; signedness `as`.
* First floating-point fixture.

### Level 2: `f64` sibling (`trunc`)

* Same algorithm at `f64`/`u64`; confirms width-parametric `@bitCast`.

### Level 3: arithmetic-carrying float fns (`floorf`, `ceilf`)

* Actual `f32` arithmetic (`x + huge`, comparisons), float literals,
  `force_eval` rounding semantics — decide whether to model or drop.

### Level 4: the generic `F: Float` form

* The real crate's generic implementation: generic-as-comptime over a
  float bound, mapping `F::to_bits` / `F::SIG_MASK` etc. to Zig builtins.
  Related to the trait work in [hasher.md](hasher.md) and the `PrimInt`
  direction in [isqrt.md](isqrt.md) level 3.

## Test

| Path | Role |
|------|------|
| `rust/float`, `zig/float.zig` | Level 1 golden pair |

* `#[test]`: `truncf(3.7) == 3.0`, `truncf(-3.7) == -3.0`, `truncf(3.0)`,
  `truncf(0.0)`, a subnormal, and a large value that hits the early
  `e >= 23 + 9` return. `assert_eq!` in Zig `(expected, actual)` order.
  Float equality here is exact (results are integral or unchanged).
* Regenerate SCIP with `./build_index.sh float`.
* Golden compare via `./test.sh float`; behavioral parity via
  `./test_test.sh float`.

## Next steps

1. Lay down `rust/float` + `zig/float.zig` level-1 golden pair.
2. Implement `f32`/`f64` in `ty.rs`.
3. Implement `to_bits`/`from_bits` -> `@bitCast` (moniker dispatch, target
   type from context).
4. Add the signedness-reinterpret arm to `syn::Expr::Cast`; pin the
   same-width cast convention.
5. Then level 2 (`f64` `trunc`).
