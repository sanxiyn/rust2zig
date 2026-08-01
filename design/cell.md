# Cell erasure

Status and roadmap for translating Rust's `Cell<T>` into idiomatic Zig.
**Level 1 implemented.** The emitted output is verified against Zig 0.16.0.
Driven by `rust/regex`, where `Parser` holds a `Cell<Position>` so that the
parse methods can advance the position through `&self`.

## Design

* Zig has no interior mutability and no way to express it. `Cell<T>` is erased
  to `T`; the mutability it grants is re-expressed in the pointer types.
* A shared reference on the path to a `Cell` is not an artifact to work around
  — it *is* the `Cell` contract. `fn parser(&self) -> &Parser` deliberately
  reborrows a `&mut Parser` as shared, and compiles only because `Cell` hands
  the mutability back. So erasure has to restate that in Zig's terms, and
  belongs in the type mapping rather than in a separate analysis.
* Prefer a type-directed rule over dataflow. Constness is decided by the
  referent type alone, so there is no call-graph fixpoint and no interprocedural
  inference.
* Erase, do not shim. A faithful `Cell(T)` shim is not writable in safe Zig
  (see Alternatives).
* Promotion is safe, not merely expedient: Zig's `*T` carries no exclusivity
  guarantee (no implicit `noalias`), so the aliasing `Cell` permits keeps
  working. What is lost is documentation of intent, not semantics.
* `Cell::get` requires `T: Copy`, so a read erases to a value copy — exactly
  Zig's struct assignment semantics. No aliasing hazard is introduced.

## The rule

> A Rust shared reference `&T` translates to Zig `*T` rather than `*const T`
> when `T` is **Cell-bearing**.

`T` is Cell-bearing when it transitively owns a `Cell` **through owned fields**:

* a field whose type is `Cell<_>`, or
* a field whose type is itself a Cell-bearing struct (by value, or an array of
  such).

Ownership stops at indirection: a field of type `&U`, `&mut U`, or `*U` does
**not** make `T` Cell-bearing, even when `U` is. This cutoff is what keeps the
rule local, and it is licensed by fact 1 below — Zig constness does not
propagate through a pointer hop, so mutability only has to be granted down to
the first indirection.

The same set decides receivers: `&self` maps to `self: *Self` instead of
`self: *const Self` exactly when `Self` is Cell-bearing.

`design/result.md`'s level 3 (out-of-band diagnostics) needs the same promotion
for the same reason: a `&self` method that records a diagnostic on the receiver
is mutating through a shared reference, exactly as a `Cell` write does.

### Applied to `rust/regex`

| Type | Owns | Cell-bearing | Consequence |
|------|------|--------------|-------------|
| `Parser` | `pos: Cell<Position>` | yes | `&Parser` -> `*Parser`, so `fn parser(&self) -> &Parser` returns `*Parser` |
| `ParserI` | `parser: &'s mut Parser` | no (reference) | `&self` stays `*const ParserI` |

So `bump`'s write, `self.parser().pos = ...`, goes through a mutable pointer
returned by `parser()` while the receiver stays const. No receiver promotion, no
temporaries at call sites, no analysis pass.

## Zig facts

Verified against Zig 0.16.0; the design rests on these.

| # | Case | Result |
|---|------|--------|
| 1 | Write through a **pointer field** from a `*const` receiver | compiles |
| 2 | Write an **owned field** from a `*const` receiver | `error: cannot assign to constant` |
| 3 | `Cell(T)` shim with `set` via `@constCast` | compiles and passes, *including on a genuinely `const` object* |
| 4 | Call a `*Self` method on a temporary | `error: cast discards const qualifier` |

Fact 1 licenses the ownership cutoff. Fact 2 is why the owned case (level 2)
needs receiver promotion at all. Fact 3 is the argument against a shim: it is
silent UB, not a compile error. Fact 4 is what forces addressable bindings once
a receiver is promoted.

## Implementation

1. Analyze (`src/translate/zig/cell.rs`, called from `Translator::analyze`)
   * `cell_types: HashSet<String>` — SCIP symbols of Cell-bearing types,
     computed by `collect_cell_types` as a fixpoint over owned struct fields.
     `owns_cell` recurses through arrays and tuples and stops at any
     indirection.
2. Types (`ty.rs`)
   * `Cell<T>` -> `translate_type(T)`, gated on the `core::cell::Cell` moniker.
   * `Type::Reference` consults `cell_types` for the referent's symbol and
     emits `is_const: false` when it is Cell-bearing.
3. Receivers (`item.rs`)
   * `&self` -> `self: *Self` when `Self` is in `cell_types`; otherwise the
     existing `*const Self`. The receiver's type comes from `Scip::type_at` on
     the `self` token (`self: &Parser`), since the enclosing type is not
     otherwise available while translating a method.
4. Expressions (`call.rs` / `expr.rs`), all moniker-gated
   * `Cell::new(x)` -> `x`
   * `c.get()` -> `c`
   * `c.set(v)` -> `c = v`

The one emission wrinkle is `set`: it is a method call in Rust but an
assignment in Zig. `Cell::set` returns `()`, so it only ever appears in
statement position, and `translate_method_call` can return an assignment node
without a statement/expression mismatch.

## Test

| Path | Role |
|------|------|
| `rust/regex`, `zig/regex.zig` | Level 1: Cell reached through a pointer field |

`rust/regex` reaches its `Cell` through `ParserI`'s `&'s mut Parser`, so it
exercises level 1 only. Level 2 has no fixture yet.

`zig/regex.zig` does not exist yet: the fixture still hits unrelated gaps
(`Box`, `Vec`, `loop`, `Result`, `char`), so neither suite covers it and both
report `SKIP regex`. The Cell-bearing portion of the emitted output was checked
by compiling it under `zig test` in isolation.

## Alternatives considered

* **Dataflow-based return promotion.** Infer which returned pointers are
  written through and promote only those. Strictly more precise — it would keep
  `const` on a read-only accessor of a Cell-bearing type, where the rule above
  over-promotes. Rejected for now as much more machinery for the same answer on
  the only fixture we have; it can refine the type-directed rule later without
  invalidating output the rule already gets right.
* **Blunt promotion: `&self` -> `*Self` everywhere.** Self-consistent and
  trivial, but discards `const` project-wide to solve one type's problem.
* **Keep `Cell`, emit a Zig `Cell(T)` shim.** Not writable in safe Zig: `set`
  through a shared pointer requires `@constCast`, and writing through a
  `@constCast` of a `const` value is UB. Fact 3 shows the failure mode is
  silent — the shim compiled and the test passed while doing exactly that.
  Rendering a checked Rust construct as unchecked UB is strictly worse than
  erasure.
* **`@constCast` at the write sites instead of in a shim.** Same objection,
  spread over more places.
* **Inline trivial accessors** (`self.parser().pos` -> `self.parser.pos`).
  Sidesteps the accessor's `*const` return without addressing it. Rejected: it
  deletes a method the source declares, and it only works for one-line bodies —
  an accessor with any logic in it puts the problem straight back.

## Implemented

### Level 1: Cell through indirection

The rule, the erasure, and the `new` / `get` / `set` rewrites — everything
above. On `rust/regex` this emits `pos: Position` for the erased field,
`fn parser(self: *const Self) *Parser` for the promoted accessor return with an
unpromoted receiver, `self.parser().pos` for a `get`, and
`self.parser().pos = ...` for a `set`.

## Not implemented yet

### Cell-bearing enums

`collect_cell_types` walks struct items only, so a `Cell` in an enum variant
payload does not mark the enum Cell-bearing, and a `&Enum` on the path to a
write stays `*const`. The rule itself is agnostic — only the closure that
computes the set is not — so this is a matter of also scanning variant fields.
No fixture exercises it.

### Level 2: owned Cell-bearing receiver

`struct Counter { n: Cell<u32> }` with `fn incr(&self)`. The rule already gives
the right answer (`Counter` is Cell-bearing, so the receiver is `*Counter`),
but by fact 4 callers then need an addressable binding — `Counter::new().incr()`
does not translate without introducing one. Needs a fixture and a call-site
rewrite that materializes the temporary.

### Level 3: the rest of the `Cell` API

`replace`, `take`, `into_inner`, `get_mut`. All erasable through the same
moniker table; `replace` and `take` additionally need a temporary to hold the
old value, and unlike `get` they do not require `T: Copy`.

### Not planned

* `RefCell`. Its borrow flags are runtime state whose failure mode is a panic,
  so it cannot be erased — only shimmed, and unlike `Cell` a shim for it is
  actually writable. A separate problem.
* `UnsafeCell`, `OnceCell`, `LazyCell`, and the `sync` family (`Mutex`,
  `RwLock`, atomics).
