# Research: minimum viable `.crate` translation

This directory collects research toward the next milestone: hand rust2zig a
published crate and get buildable Zig. Each doc takes one small, real crate
and works out — before any implementation — what gaps it forces and how they
should translate. It is the same method as `bitset` (extracted from
`fixedbitset`): pick real code, strip what is out of scope, discover the
gaps empirically.

These are **research notes, not committed plans**. The roadmaps here describe
intended designs; the code is the source of truth for what is actually
implemented.

## Scope

Deliberately narrow for the first pass:

* **Leaf** — no dependencies (keeps whole-crate SCIP indexing tractable
  offline).
* **No macros** — no derive, attribute, or function-like macros.
* **No std / no alloc** — `#![no_std]`, no heap.

Not empty: even under these constraints, a real crate exercises code the
curated examples never did. A recurring finding — call it "bitset is not
fixedbitset" — is that *every* candidate fails the constraints in its real
form (a dependency, a generic bound, a macro-stamped impl), so each fixture
is a hand-edited extraction of the essence.

## `.crate` ingestion is nearly free

A `.crate` is a gzipped tarball that unpacks to a Cargo package directory
(`Cargo.toml` + `src/`) — exactly what `main.rs` already takes. Ingestion is
`untar` then `rust-analyzer scip .` on the unpacked dir. The leaf constraint
is what makes the SCIP step work with no network. The real content of the
milestone is "multiple files + whatever a real leaf throws at us," not the
tarball mechanics.

## Targets

| Doc | Crate | Axis | Key new gaps |
|-----|-------|------|--------------|
| [hash.md](hash.md) | `const-fnv1a-hash` | wrapping arithmetic | `wrapping_mul` -> `*%`; widening `as` -> `@as`/`@intCast` |
| [random.md](random.md) | `oorandom` | stateful struct + intrinsics | narrowing `as` -> `@truncate`; `rotate_right` -> `std.math.rotr`; `u128`; assoc `const` |
| [isqrt.md](isqrt.md) | `integer-sqrt` | bit intrinsics | `leading_zeros` -> `@clz`; `!` int -> `~` (type-driven) |
| [float.md](float.md) | `libm` `truncf` | floating point / bit-cast | `f32`/`f64`; `to_bits`/`from_bits` -> `@bitCast`; signedness `as` |
| [hasher.md](hasher.md) | `fnv` | traits / dispatch | tuple struct / newtype; trait-impl erasure; trait bound -> `comptime T: type` |

## Dependency spine

The fixtures are not independent; later ones reuse machinery earlier ones
build.

```
hash  (wrapping *% , widening as)
 ├── random  (adds narrowing @truncate, rotate_right, u128)
 └── hasher  (adds tuple struct, trait erasure; L2: comptime trait bound)

isqrt  (!/~ type-driven not)
 └── float  (reuses ~; adds f32, @bitCast, signedness as)
```

* **`hash` is the root.** Its `wrapping_mul` -> `*%` and widening `as` are
  reused by both `random` and `hasher`. Do it first.
* **`isqrt` seeds `float`** with the type-driven `!`-vs-`~` dispatch.
* **`hasher` level 2 is the climax:** a trait bound translated to
  `comptime H: type` with structural method dispatch. It extends the
  existing type-param generics (min / Option) to trait-bounded generics and
  closes the project's largest open question (trait translation, flagged
  2026-04-06). Everything before it is groundwork.

## Suggested order

1. **`hash`** — cleanest leaf, trait-free; buys the two most reused gaps
   (wrapping arithmetic, `as` casts). Best first real `.crate`-shaped path.
2. **`isqrt`** — orthogonal, small (`@clz`, `~`); can go in parallel.
3. **`random`** — first stateful-struct algorithm; adds the `@truncate` /
   `rotate_right` / `u128` layer on top of `hash`.
4. **`float`** — opens the floating-point domain; reuses `isqrt`'s `~`.
5. **`hasher`** — traits. Level 1 (erased) is modest; level 2 (comptime
   trait bound) is the thesis payload — schedule it deliberately, not as a
   coattail of level 1.

## Cast taxonomy (cross-cutting)

`syn::Expr::Cast` is unhandled today and shows up across four docs; the arms
land incrementally:

| Cast | Example | Zig | First appears |
|------|---------|-----|---------------|
| widening | `u8 as u32` | `@as` / `@intCast` | [hash.md](hash.md) |
| narrowing | `u64 as u32` | `@truncate` | [random.md](random.md) |
| same-width signedness | `i32 as u32` | `@bitCast` / `@intCast` | [float.md](float.md) |
| float <-> int bits | `f32::to_bits` | `@bitCast` | [float.md](float.md) |

## Intrinsic-method dispatch (cross-cutting)

Several gaps are Rust core methods that map to a Zig builtin / operator /
`std` call. All follow the `.len()` -> `.len` pattern: match the SCIP
moniker on the call and emit the target form.

| Rust | Zig | Doc |
|------|-----|-----|
| `.wrapping_mul/add/sub()` | `*%` / `+%` / `-%` | [hash.md](hash.md) |
| `.rotate_left/right()` | `std.math.rotl/rotr` | [random.md](random.md) |
| `.leading_zeros()` | `@clz` | [isqrt.md](isqrt.md) |
| `.to_bits()` / `from_bits()` | `@bitCast` | [float.md](float.md) |

## Not yet targeted

Axes a real crate will eventually force, with no fixture yet:

* **Modules / multi-file** (`mod`, `use`) — the next structural gap; mostly
  emission work since resolution is already SCIP-symbol-driven.
* **Macro / derive expansion** — the deep tension: syn is pre-expansion,
  SCIP is post-expansion. Every fixture here edits macros away to dodge it.
* **`Result` + `?`** (`syn::Expr::Try`), or-patterns / guards, const
  generics, type aliases (`Item::Type`), top-level `const` (`Item::Const`).
* **Dependencies, features / `cfg`, std translation** (`rust.*` mirroring
  `std.*`) — out of MVP scope by construction.
