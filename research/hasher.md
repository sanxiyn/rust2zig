# Trait dispatch (FNV Hasher)

Status and roadmap for the trait / dispatch axis — the oldest deferred
unknown in the project (flagged "big unknown" on 2026-04-06). Uses `fnv`,
which is the trait-wrapped sibling of [hash.md](hash.md): the same FNV-1a
arithmetic, but delivered through a struct implementing core traits.
Companion targets: [hash.md](hash.md), [float.md](float.md),
[isqrt.md](isqrt.md), [random.md](random.md).

## Why this crate

* **No dependencies**, `#![no_std]`-capable, no alloc, no macros in the core.
* A tiny newtype struct implementing two **core** traits (`Default`,
  `core::hash::Hasher`) — so it forces the trait question without dragging in
  a large trait or a dep.
* The arithmetic is exactly [hash.md](hash.md)'s FNV-1a, so this fixture
  isolates *trait translation* from *everything else*: if it is harder than
  `hash`, the delta is purely the trait machinery.

The research payload is the thesis question restated for traits: **does a
Rust trait bound translate to a Zig `comptime T: type` with structural
method calls, the same way type-param generics already do?** The very first
design session already framed the answer's shape — "Zig iterator is a
convention, no such entity exists" — so the hypothesis is: **Rust trait ->
Zig structural convention; trait bound -> erased comptime `type`; method ->
duck-typed call.**

## Real source (essence)

```rust
pub struct FnvHasher(u64);

impl Default for FnvHasher {
    fn default() -> FnvHasher {
        FnvHasher(0xcbf29ce484222325)
    }
}

impl core::hash::Hasher for FnvHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        let FnvHasher(mut hash) = *self;
        for byte in bytes {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x100000001b3);
        }
        *self = FnvHasher(hash);
    }
}
```

## Two levels of the trait question

The key insight: **a trait matters only under generic dispatch.** When the
concrete type is used directly, the trait can be erased and its methods kept
as inherent methods. So split:

* **Level 1 (monomorphic, trait erased):** the fixture uses `FnvHasher`
  concretely. Drop the `Default` / `Hasher` trait *declarations* entirely;
  translate `default` / `write` / `finish` as inherent methods on the Zig
  struct. `h.write(bytes)` / `h.finish()` / `FnvHasher.default()`. No trait
  concept reaches Zig.
* **Level 2 (generic dispatch):** a function bounded by the trait,
  `fn hash_bytes<H: Hasher + Default>(bytes: &[u8]) -> u64`. *This* is where
  the trait must translate — to `comptime H: type`, with the body calling
  `h.write(...)` / `h.finish()` structurally and Zig checking the shape at
  instantiation. Same mechanism already sketched for `Fn`-bounds in the
  `Option::map` TODO ("comptime F: type ... Zig comptime checks the shape").

## Level 1 gaps

Grounded against the handled `syn::Expr` / `syn::Item` sets.

### New

1. **Tuple struct / newtype `FnvHasher(u64)`.** Only named-field structs
   (Ratio, Point) exist today. Zig has no tuple struct; represent as a
   single-field struct (`FnvHasher = struct { v0: u64 }`) and map field `.0`
   -> `.v0`. Includes the tuple-struct **pattern** `let FnvHasher(mut hash)
   = *self;` -> bind `hash` from `self.v0`, and tuple-struct **construction**
   `FnvHasher(hash)` -> `.{ .v0 = hash }`.
2. **Trait-impl erasure.** `impl Default for T` / `impl Hasher for T` must be
   recognized and their methods folded into the struct as inherent methods,
   the trait dropped. Distinguish from `impl Drop` (which is *kept* and
   mapped to a Zig method) — so `analyze` grows a notion of "erasable trait
   impl" vs the special `Drop` case.
3. **`default()` as constructor.** `Default::default()` -> a `fn default()
   Self` factory (or inline the initializer at the call site). Decide the
   convention.

### Reused

4. **`wrapping_mul` -> `*%`**, **`*byte as u64` widening `as`**, **slice
   iteration + deref** — all from [hash.md](hash.md).
5. **`*self = FnvHasher(hash)` whole-value assignment through `&mut self`**
   — reference / receiver work from `geometry2` / `inc`.

## Proposed Zig output (level 1)

```zig
const std = @import("std");

const FnvHasher = struct {
    const Self = @This();

    v0: u64,

    fn default() Self {
        return FnvHasher{ .v0 = 0xcbf29ce484222325 };
    }

    fn finish(self: *const Self) u64 {
        return self.v0;
    }

    fn write(self: *Self, bytes: []const u8) void {
        var hash = self.v0;
        for (bytes) |byte| {
            hash ^= @as(u64, byte);
            hash = hash *% 0x100000001b3;
        }
        self.v0 = hash;
    }
};
```

Details to confirm during implementation:

* Field naming for tuple structs: `.0` -> `v0` (or `@"0"`?). `v0` reads
  better and avoids the escaped-identifier path; pin the rule.
* The pattern `let FnvHasher(mut hash) = *self;` should lower to `var hash =
  self.v0;` (bind-by-value of a `Copy` field), consistent with the
  `mut`-binding preamble convention (`var x = _x;`).
* Whether `default` should be emitted at all, or the initializer inlined
  where `FnvHasher::default()` is called. Level 1 keeps the method for a
  faithful structural translation.

## Level 2 sketch (generic dispatch)

```rust
fn hash_bytes<H: core::hash::Hasher + Default>(bytes: &[u8]) -> u64 {
    let mut h = H::default();
    h.write(bytes);
    h.finish()
}
```

```zig
fn hashBytes(comptime H: type, bytes: []const u8) u64 {
    var h = H.default();
    h.write(bytes);
    return h.finish();
}
// call site: hashBytes(FnvHasher, bytes)
```

* Trait bound `H: Hasher + Default` -> erased to `comptime H: type`; no
  annotation, Zig checks method shape at instantiation.
* `H::default()` -> `H.default()`; `h.write` / `h.finish` -> structural
  calls (no `.call` closure indirection — these are real methods).
* This is the actual generic-as-comptime-*for-traits* test. If it works, it
  extends the existing type-param generics (min / Option) to trait-bounded
  generics, closing the largest thesis gap.

## Levels

### Level 1: monomorphic `FnvHasher`, trait erased (this doc)

* Fixture: `rust/hasher` -> `zig/hasher.zig`.
* Gaps: tuple struct / newtype; trait-impl erasure; `default` constructor.

### Level 2: generic `hash_bytes<H: Hasher + Default>`

* Trait bound -> comptime `type`; structural method dispatch.
* First trait-bounded-generic fixture; the thesis payload.

### Level 3: the trait as a first-class translated entity

* Only if a fixture needs `dyn Hasher` or a trait object / vtable. Likely
  out of scope for the "generic as comptime, no monomorphization" thesis —
  record where dynamic dispatch would force a representation, and whether it
  can be avoided.

## Test

| Path | Role |
|------|------|
| `rust/hasher`, `zig/hasher.zig` | Level 1 golden pair |

* `#[test]`: `let mut h = FnvHasher::default(); h.write(b"foobar");
  assert_eq!(0x..., h.finish());` with a known FNV-1a-64 vector. Zig
  `(expected, actual)` order.
* Level 2 test: `assert_eq!(hash_bytes::<FnvHasher>(b"foobar"), 0x...)`.
* Regenerate SCIP with `./build_index.sh hasher`.
* Golden compare via `./test.sh hasher`; behavioral parity via
  `./test_test.sh hasher`.

## Next steps

1. Land [hash.md](hash.md) level 1 first (`wrapping_mul` + `as` reused here).
2. Lay down `rust/hasher` + `zig/hasher.zig` level-1 golden pair.
3. Implement tuple-struct type / pattern / construction; trait-impl erasure
   in `analyze`; `default` constructor convention.
4. Then level 2 — trait bound -> `comptime H: type` — the real trait test.
