# Drop elaboration

Status and roadmap for elaborating Rust `Drop` into idiomatic Zig.
Owned values that need destruction are lowered using analysis in `analyze`
and emission in the Zig backend.

## Design

* Known-good Rust (rustc already checked); we do not re-implement borrowck.
* Lifetimes stay erased; moves and drops are runtime-relevant and must
  be reflected in Zig.
* Prefer idiomatic Zig: `defer x.drop()` when destruction is unconditional
  on scope exit; alive flags only when destruction is path-sensitive.
* Do not introduce flags when drops are unconditional: level 1 output
  must stay free of `x_alive` noise (`drop` test guards this).
* Keep high-level structure (no MIR). Path-sensitive facts are computed on
  the `syn` tree for a bounded dialect.
* Prefer collecting facts in `analyze` and using them in translation.

## Implementation

1. Analyze (`src/translate/zig/drop.rs`, called from `Translator::analyze`)
   * Record types with `impl Drop` (`drop_types`, SCIP symbol of the type).
   * For each `needs_drop` binding (local or param), compute `DropInfo`
     keyed by the binding's SCIP symbol.
2. Emit
   * Unconditional scope-end drop → `var` + `defer x.drop()`.
   * Conditional drop → `x_alive` flag, `defer { if (x_alive) x.drop(); }`,
     and `x_alive = false` immediately before a whole-value move.
   * Explicit `drop(x)` → `x.drop()`.
   * `impl Drop for T` → method `fn drop(self: *Self)` on the struct.

## DropInfo

* `name`: source binding name
* `may_move`: moved on at least one exit path of the binding's scope
* `must_move`: moved on every exit path
* `has_drop_call`: `drop(x)` appears in the scope

Derived:

| Condition | Emission |
|-----------|----------|
| not in map (no `Drop`) | ordinary `const`/`var` |
| `!may_move` | `defer x.drop()` (level 1) |
| `must_move` | no defer (ownership left the scope) |
| `may_move && !must_move` | alive flag + conditional defer (level 2) |
| `has_drop_call` (and still need mut place) | force `var` so `x.drop()` is legal |

Move detection treats whole-value uses as moves (call args, `return x`,
tail `x`). Non-moves: field access, `&x`, method receivers (auto-ref).
Clears use SCIP symbols of the moved path, not bare names.

Path-sensitive analysis covers structured control flow without loops:
`if` / `else` / `else if`, `match`, `return`, tail expressions, nested
blocks. Loops are out of scope for current analysis (level 3).

## Test

| Path | Role |
|------|------|
| `rust/drop`, `zig/drop.zig` | Level 1: never-moved vs always-moved |
| `rust/drop2`, `zig/drop2.zig` | Level 2: conditional whole-value moves |

Shared scaffolding: `Ticket` with `Drop` logging into statics (simplified
so the translator can emit it). Tests assert drop counts and order.

## Implemented

### Level 1: unconditional

* Never moved in scope → `defer x.drop()`.
* Always moved out (`return x`, always passed by value) → no defer.
* Params that are dropped at end of function → rebind `_x` / `var x = _x`
  (Zig params are const) + defer, same pattern as `mut` params.
* Nested blocks → block-local defers; Zig `defer` LIFO matches Rust reverse
  declaration order at a scope.
* Explicit `drop(x)` in tests → `x.drop()`; force `var` when needed.

### Level 2: conditional (no loops)

* Path analysis distinguishes may vs must move.
* Flags only when `may_move && !must_move`.
* Clear flag on the statement that performs the move (not on outer
  `if`/`match` as a whole).
* Cases covered in `drop2`: `if`/`else`, early `return` with move, `match`,
  nested `if`, two independent conditionals.

## Not implemented yet

### Level 3: loops

* `while` / `loop` / `for` with moves of `needs_drop` values.
* Drop flags or re-init patterns across iterations; continue/break exits.
* Requires extending the exit lattice over loop back-edges (not done).

### Level 4: partial

* Moving `s.field` while other fields of `s` remain owned.
* Per-field (or residual) liveness; drop glue for the unmoved remainder.

### Level 5

* Unwind / panic paths (today: normal exits only; panic ~ abort is fine
  for current tests).
* `ManuallyDrop`, `mem::forget`, `ptr::read` / `drop_in_place`.
* Generics: `needs_drop` for type params / fields of generic types.
* Closures capturing `needs_drop` values; `Drop` on enums with data.
* Drop order for complex nesting beyond "defers at each scope".
* Mapping `Drop` to Zig conventions (`deinit`) as a policy choice.
* Translating real `std` types (`String`, `Vec`, ...) once lang items /
  alloc runtime exist.
