# FNV-1a hash

Status and roadmap for the first genuinely leaf, trait-free `.crate`-shaped
target: `const-fnv1a-hash`. Shares the MVP scope and `.crate` ingestion notes
with [isqrt.md](isqrt.md); this doc records what is specific to the hash
fixture. Companion targets: [isqrt.md](isqrt.md), [random.md](random.md).

## Why this crate

Of the small crates surveyed, `const-fnv1a-hash` is the cleanest fit for a
first real `.crate`:

* **Leaf** — no dependencies.
* **`#![no_std]`**, no alloc.
* **No derives, no traits** — pure `const fn`s.
* The only macro (`fnv_hash_impl!`) exists solely to stamp out the
  32/64/128 variants. Edit it away by expanding one variant by hand, the
  same move as `bitset` from `fixedbitset` and the de-generic'd `isqrt`.

Crucially the two gaps it forces — **wrapping arithmetic** and **`as`
casts** — are the most broadly reused in the ecosystem (every hash, PRNG,
checksum, crypto primitive). Paying them off here has more leverage than the
narrower `@clz` / `~` from isqrt, which is why this is the preferred
level-1 MVP.

## Real source (essence)

```rust
pub const fn fnv1a_hash_32(bytes: &[u8], limit: Option<usize>) -> u32 {
    let prime = 0x0100_0193;
    let mut hash = 0x811c_9dc5;
    let mut i = 0;
    let len = match limit { Some(l) => l, None => bytes.len() };
    while i < len {
        hash ^= bytes[i] as u32;
        hash = hash.wrapping_mul(prime);
        i += 1;
    }
    hash
}
```

(Real crate also has `_64` / `_128` and `_str_` variants and a
`to_ne_bytes` / `from_ne_bytes` path in the 16-bit xor-fold variant. Out of
scope for level 1.)

## Level 1 fixture

Idiomatic Rust, concrete `u32`, drop `Option<usize>` limit, iterate the
slice directly:

```rust
pub fn fnv1a_hash_32(bytes: &[u8]) -> u32 {
    let mut hash: u32 = 0x811c9dc5;
    for byte in bytes {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(0x01000193);
    }
    hash
}
```

Note `for byte in bytes` over `&[u8]` yields `byte: &u8`, hence `*byte`
(existing slice-by-reference + deref-insertion handling from the `iter` /
binary-deref work).

## Gaps

Grounded against the handled `syn::Expr` / `syn::Item` sets.

### New (this fixture forces them)

1. **`hash.wrapping_mul(p)` -> Zig `hash *% p`.** Wrapping arithmetic. Rust
   spells it as intrinsic methods (`wrapping_mul` / `wrapping_add` /
   `wrapping_sub`); Zig has dedicated operators `*%` / `+%` / `-%`. Match
   the SCIP moniker (`core::num`...`wrapping_mul`) on the method call and
   emit the wrapping operator instead of a normal `.call`. Same
   moniker-dispatch shape as `.len()` -> `.len`. **Highest-leverage gap in
   the whole MVP** — real numeric crates use it constantly.
2. **`*byte as u32` -> Zig `@as(u32, *byte)` (widening) / `@intCast`.**
   `syn::Expr::Cast` is not handled at all today. Level 1 only needs the
   widening `u8 -> u32` case; general narrowing (`@truncate`) and
   int<->bool / int<->enum are deferred (see [random.md](random.md) for the
   narrowing case). Emit via the SCIP source/target types.

### Already solved

3. `^=` compound bitwise assignment, `for` over slice, `*byte` deref,
   `let mut`, integer literals — all handled.

## Proposed Zig output (level 1)

```zig
const std = @import("std");

fn fnv1aHash32(bytes: []const u8) u32 {
    var hash: u32 = 0x811c9dc5;
    for (bytes) |byte| {
        hash ^= @as(u32, byte);              // as cast (widening)
        hash = hash *% 0x01000193;           // wrapping_mul -> *%
    }
    return hash;
}
```

Details to confirm during implementation:

* `for (bytes) |byte|` binds `byte` by value (`u8`), so the Rust `*byte`
  deref should collapse to a plain `byte` on the Zig side — verify the
  slice-iteration lowering already does this (the `iter` example captures
  slice elements by reference; confirm the value/deref bookkeeping lands
  on `@as(u32, byte)`, not `@as(u32, byte.*)`).
* `0x01000193` as a `u32` literal in a `*%` with a `u32` lhs should need no
  explicit `@as`; confirm no spurious cast is emitted.

## Levels

### Level 1: `fnv1a_hash_32` over `&[u8]` (this doc)

* Fixture: `rust/hash` -> `zig/hash.zig`.
* Gaps: `wrapping_mul` -> `*%`; widening `as`.
* First truly-leaf, trait-free `.crate`-shaped end-to-end path.

### Level 2: the `limit: Option<usize>` variant

* `Option` param + `match limit { Some(l) => l, None => bytes.len() }`.
* Exercises `Option` in argument position and `.len()` fallback (handled),
  plus an index-based `while` loop over `bytes[i]` (indexing handled).

### Level 3: string and multi-width variants

* `fnv1a_hash_str_32(&str)` via `.as_bytes()`.
* `_64` / `_128` widths (u128 handled in `ty.rs`).
* The 16-bit xor-fold variant pulls in `to_ne_bytes` / `from_ne_bytes`
  (byte-order intrinsics -> Zig `@bitCast` / `std.mem`) — a new gap, defer.

## Test

| Path | Role |
|------|------|
| `rust/hash`, `zig/hash.zig` | Level 1 golden pair |

* `#[test]`: known FNV-1a vectors, e.g. empty slice -> `0x811c9dc5`,
  `b"a"` -> `0xe40c292c`, `b"foobar"` -> `0xbf9cf968`. `assert_eq!` in
  Zig `(expected, actual)` order.
* Regenerate SCIP with `./build_index.sh hash`.
* Golden compare via `./test.sh hash`; behavioral parity via
  `./test_test.sh hash`.

## Next steps

1. Lay down `rust/hash` + `zig/hash.zig` level-1 golden pair.
2. Implement `wrapping_mul` -> `*%` (moniker dispatch on method call) and
   widening `as` (`syn::Expr::Cast`).
3. Confirm slice-iteration value/deref bookkeeping under the `as` cast.
4. Then level 2 (`Option` limit) or pivot to [random.md](random.md) for
   narrowing `as` + `rotate_right`.
