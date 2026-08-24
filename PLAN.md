# Long-term plan

Where the project is heading, and — more usefully day to day — how to tell
which of today's code is worth investing in.

The spine of this document is the eventual migration off `syn` + SCIP to a real
Rust frontend. The other long-term threads (crate ingestion, further backends)
are placed relative to it at the end.

## Where we are

`syn` for parsing, a rust-analyzer SCIP dump for semantics. Five direct
dependencies, no nightly, no unstable API surface, a full build in seconds, and
`cargo run -- zig rust/gcd /tmp/out` is the whole pipeline.

This was the right call and it has run much further than expected. Two backends,
sixteen fixtures, and features — `Drop` elaboration, `Cell` erasure, error-set
mapping, dependency-ordered OCaml output — that read like they need a type
checker behind them. They do not, quite, because rust-analyzer already ran one
and wrote its answers down.

The cost is real but it has been paid in the right currency: what SCIP cannot
answer has been worked around locally, one query at a time, instead of being
allowed to block the emission work that is the actual project.

## The ceiling

SCIP is an **index**, not a type API. It maps occurrence to symbol, and symbol
to a *rendered signature string*. Everything type-shaped in this codebase is
therefore recovered by parsing text:

* `Scip::type_at` splits a signature at `": "` and runs `syn::parse_str` on the
  remainder.
* `Scip::binary_type_at` parses the *symbol string itself* —
  ``ops/arith/impl#[i32][`AddAssign<&i32>`]add_assign().`` — finding `<` and
  `>` by index, with a backquote branch for names that are not plain
  identifiers.
* `Scip::self_type_at` scrapes `impl#[T]` out of a symbol because a foreign
  crate's symbol carries no `SymbolInformation` at all.
* `translate::ty::expr_type` is a hand-rolled partial type checker over the
  expression shapes SCIP answers directly plus the ones that pass a type
  through.
* `desugar::generic`'s `find_type_param` / `peel_type` reconstruct generic
  instantiation by shape-matching a parameter's type against an argument's.

Three consequences follow, and they compound:

**Types have no identity.** A type recovered from a signature is a `syn::Type`
with synthetic spans, so it cannot be resolved *back* through SCIP. `doc/ml.md`
records the workaround at `expr_module`: the name is matched against the
collected module names instead. That is name equality standing in for symbol
equality — sound on the current fixtures, and exactly the kind of thing that
breaks quietly when two types share a name.

**Only some questions are askable.** `type_at` answers for `Variable`,
`Parameter`, `SelfParameter`, and `Field`. There is no per-expression inference
result, no `is_copy`, no trait resolution, no method-resolution detail, no
adjustments. Features that need those are not hard — they are unreachable.

**Every gap is filled by re-deriving what rustc already computed.** This is the
one that matters. `expr_type` is a type checker. `find_type_param` is
unification. Each is small, each is justified locally, and together they are the
front half of a compiler being reimplemented by accident, in string-processing,
without a specification.

Two smaller frictions, worth naming because migration removes them: every
fixture needs a `rust-analyzer scip .` run before it can be translated
(`build_index.sh`, and `index.scip` is gitignored, so the pipeline needs the
binary on PATH), and lookups are keyed on exact `(line, column)` ranges built
from `proc_macro2` spans.

## The target: rust-analyzer as a library

**rust-analyzer, not rustc.** Three reasons, in order of weight:

1. **It is already the source of truth.** Every semantic fact the translator
   uses today is rust-analyzer's answer, serialized through SCIP and re-parsed.
   Calling the library directly is not switching engines; it is deleting a
   lossy serialization step. Answers should not change — only get richer — which
   is what makes the migration differentially testable against the existing
   goldens.
2. **rustc's useful IR is at the wrong altitude.** `design/drop.md` states the
   principle explicitly: "Keep high-level structure (no MIR)." The whole thesis
   is that generated code is for human consumption, which means emission tracks
   the source's structure. MIR has already thrown that away. rustc's HIR would
   serve, but reaching it means `rustc_private`, nightly, and a driver harness.
3. **`ra_ap_*` crates are published on crates.io** and build on stable.

What the library provides that the index cannot:

* `hir::Type` as a value with identity and methods — `is_copy`, `as_adt`,
  generic arguments — instead of a re-parsed string.
* Inference results for **every expression**, not just for bindings.
* Method resolution and **adjustments** — autoref, autoderef, deref coercion —
  which is precisely the information the `binary` desugar pass currently
  reconstructs from an operator's dispatched impl symbol.
* Real generic substitutions, which is `desugar::generic`'s entire job handed
  over for free.
* Trait solving, so `T: Copy` and `needs_drop` become askable.
* Workspace loading (`load_cargo`) with no index-building step, and proc-macro
  expansion.

## The architecture: `syn` stays, the oracle changes

The recommendation is to keep `syn` as the AST and use rust-analyzer purely as a
semantic oracle — the same shape as today, with `Scip` replaced by something
backed by `hir::Semantics`.

**Why not port to rust-analyzer's syntax tree.** The desugar architecture needs
a tree that is cheap to *construct and rewrite*: nine passes built on
`syn::visit_mut::VisitMut`, with `quote!` / `parse_quote!` synthesizing new
nodes. rowan is a lossless tree designed for incremental reparsing and IDE
edits; synthesizing a subtree in it is markedly clunkier, and there is no
`visit_mut` equivalent. Against that, the translator is thousands of lines of
`syn::Expr` matching. The port would be most of a rewrite, and it would buy
nothing the oracle does not already give.

**Why the two-tree bridge is safe here.** It is the same bridge as today: `syn`
spans map to source ranges, and semantic questions are asked at ranges. The
soundness discipline is already written down in `doc/desugar.md` — query SCIP
only at original spans; synthetic nodes carry `call_site` spans that never
resolve; a pass may duplicate a span for a *use* but never for a *definition*.
That invariant transfers unchanged, because it was never about SCIP. It is
about the fact that the semantic model describes the *original* program, and
desugar output is not that program.

This is the strongest continuity argument available: the hard-won rule that
makes the current design work is exactly the rule the next design needs.

**The one genuinely new piece of plumbing** is offset mapping. `proc_macro2`
reports line and column; rowan indexes by byte offset. The bridge needs a line
index, and it has to settle the character-encoding question that SCIP raises
today and no fixture has ever exercised, since every fixture is ASCII.

## Stages

**Stage 0 — narrow the interface.** `Scip` is queried from all over the
translator. Define a facade with the questions actually asked — symbol at,
kind at, type at, return type at, self type at, binary operand types,
definition range, moniker check — and route everything through it. Do this
whether or not the migration ever happens: it is the difference between a
mechanical swap and an archaeology project. Where possible the type answer
should stop being `syn::Type`, since that is the shape that cannot be resolved
back.

**Stage 1 — second implementation, differentially tested.** Implement the facade
over `hir::Semantics` and run both against the fixture suite, comparing answers
query by query. The goldens are the migration harness, and this is the moment
they pay off most: sixteen fixtures with byte-exact expected output, plus
`test_test.sh` proving input and output are behaviourally equivalent.

**Stage 2 — cut over.** Delete `src/scip.rs`, `proto/`, `build.rs`,
`build_index.sh`, and the per-fixture index step.

**Stage 3 — collect the winnings.** Delete `GenericArgRef` / `find_type_param` /
`peel_type` in favour of real substitutions; delete `expr_type`; replace
`is_closure_type`'s `impl Fn` text match with a real closure type; replace
`ml`'s name-matched `expr_module` with symbol identity. Then the features that
are currently unreachable: closure capture mode from `is_copy`, `needs_drop`
for generic types, trait-dependent lowering.

## What this means for work now

The useful test, applied to any piece of code or any proposed feature:

> Is this **reconstructing what rustc already knows**, or **deciding how the
> target should express it**?

The first is stopgap. Keep it minimal, accept known gaps, do not generalize it.
The second is the project, and it survives any frontend change.

| Stopgap — dies at migration | Durable — survives it |
|---|---|
| `translate::ty::expr_type` | the `Drop` lowering strategy (`design/drop.md`) |
| `find_type_param` / `peel_type` / `GenericArgRef` | the `Cell` erasure rule (`design/cell.md`) |
| `Scip::binary_type_at`'s symbol-string parsing | `Result` → error union, and the payload-free test (`design/result.md`) |
| `Scip::self_type_at` | SCC ordering and `let rec` grouping (`design/recursion.md`) |
| `is_closure_type`'s `impl Fn` text match | the closure struct shape (`design/closure.md`) |
| `ml`'s name-matched `expr_module` | OCaml namespacing, `ref` vs `mutable` (`doc/ml.md`) |
| per-fixture SCIP index building | every desugar pass's *rewrite* |

Two clarifications the table invites:

**Design documents are durable even where the implementation is stopgap.**
`design/generic.md`'s rule — a type parameter becomes `comptime T: type`, call
sites get explicit type arguments, a generic type is a container function — is a
statement about Zig and holds regardless of how the instantiation is discovered.
Only the reconstruction machinery in its "call site is a desugar pass" section
dies. Same for the desugar passes themselves: `binary` inserting an explicit
deref is durable; *how* it learns the operand is a reference is not.

**Fixture breadth is durable.** Every new example pins emission behaviour that
outlives the frontend, and each one is a differential test case for stage 1. A
fixture is never wasted work.

## Generics: freeze at minimum viable

Concretely, since this is the immediate case.

The emission rules are already right and tested. What is weak is inference —
which is exactly what migration replaces. So:

**Do:**
* `Option::map` via the explicit type-argument route already recommended in
  `TODO.md` step 3. It unblocks a real feature (higher-order functions over
  `Option`) and it deliberately sidesteps inference by making the user write the
  type.
* Fix the `register_generic` / `desugar::generic` disagreement documented in
  `design/generic.md`. That is a consistency bug, not an inference improvement,
  and it costs nothing.

**Do not:**
* Peel references in `find_type_param`. Needs a matching peel at the call site
  and generalizes the shape-matcher.
* Const generics.
* Widen `expr_type` to more expression shapes.
* Infer a closure's return type via Zig `@TypeOf(f.call(undefined))`.

Each of those buys a piece of type checking that rust-analyzer hands over for
free, and each makes the eventual deletion larger.

The same policy applies wherever the pattern recurs: prefer a feature that needs
*no new inference*, or one where the user can write the fact down, over one that
requires teaching `expr_type` a new shape.

## When to actually do it

Not yet, and not on a date. The triggers to watch for:

* A wanted feature genuinely needs trait resolution — closure capture-by-value
  vs by-reference needs `is_copy`; `needs_drop` for generic types needs the
  same.
* The name-for-symbol substitutions start being wrong rather than merely
  imprecise — two same-named types in one crate is the concrete case.
* Crate ingestion makes the per-crate index step the bottleneck.

Until one of those bites, syn + SCIP is still the fastest way to move, and the
work it produces is mostly in the durable column.

## Risks

* **`ra_ap_*` API churn.** Published from rust-analyzer's release train, with
  frequent breaking changes. Pin exact versions; the stage 0 facade is what
  keeps the churn from reaching the translator.
* **Dependency weight.** Going from five lean direct dependencies to a compiler
  frontend is a genuine loss — build times, and the ability to read the whole
  dependency set. Worth paying, worth naming.
* **rust-analyzer is best-effort.** It is an IDE engine and can answer "unknown"
  where rustc would not. This is not a regression, since it is already the
  source of truth, but it means unresolved answers stay a case to handle rather
  than becoming impossible.
* **Stage 1 could reveal that answers differ**, not merely improve — most likely
  where the current code compensates for a SCIP quirk. That is what the
  differential run is for, and finding it there is the cheap place to find it.

## Other threads

**Crate ingestion** (`research/`). The next milestone: hand the translator a
published `.crate` and get buildable Zig. Its stated constraints — leaf, no
macros, no std — are partly artifacts of the indexing pipeline rather than of
the translator, so migration relaxes them (proc-macro expansion in particular).
It should nonetheless proceed on syn + SCIP: it stresses *breadth* of language
coverage, which is durable emission work, and it produces exactly the fixtures
stage 1 will need. Migration is triggered by *depth* needs, not breadth.

**Further backends.** `doc/desugar.md` names Common Lisp and Haskell as
candidates after OCaml. The OCaml backend already proved the shared-desugar
architecture: passes that produce valid Rust are reusable, and each backend
picks its own pass list. A third backend is mostly emission work and is
orthogonal to the frontend question — and it is also the best available test of
whether a given piece of logic is target-specific or shared, which is the same
question this document's durability test asks.

Common Lisp is under way as target files only — `lisp/gcd.lisp`,
`lisp/iter.lisp`, `lisp/sum.lisp` with no translator behind them, documented in
`doc/lisp.md`. It has already paid the dividend claimed above twice. The
`compound_assignment` pass turns out to be wanted per *operator* rather than per
backend, which no existing backend revealed. And `design/bound.md`'s
permissiveness invariant — an emitted check may miss an error but must never
reject a valid program — recurred there under an unrelated mechanism, which is
evidence it belongs somewhere central rather than in one backend's design doc.

## Prior art: *Rust via Desugarings*

Nadrieril's [*Rust via Desugarings*](https://nadrieril.github.io/rust-via-desugarings/)
([repo](https://github.com/Nadrieril/rust-via-desugarings)) explains Rust by
progressively rewriting it through ~40 named steps until it reaches a formally
describable subset. It is the closest thing available to a specification of what
this project's desugar passes are approximating, and worth reading before
extending one.

**Read it as a catalogue, not a pass list.** Its pipeline runs *toward* MIR /
MiniRust, progressively destroying structure; ours runs the other way, since the
output is for human consumption. Several of its steps would be actively wrong to
adopt — `pipeline/loop-desugaring.md` turns `for` into `IntoIterator::into_iter`
plus `while let`, which is correct and would wreck Zig's native `for`. What it is
good for is telling us precisely what a piece of sugar hides, and therefore what
our shortcut is giving up.

Its scope boundary is also ours: it covers function-body semantics and explicitly
excludes type checking and trait solving. That is the same line this document
draws between the desugar passes and the semantic oracle, arrived at
independently.

Chapters that bear on existing designs, cited where they are relevant:

| Chapter | Bears on |
|---|---|
| `pipeline/closure-capture.md`, `pipeline/closure-adt.md` | `design/closure.md` — the struct-with-`call` shape, and capture mode |
| `pipeline/match-ergonomics.md` | `doc/desugar.md`'s match ergonomics pass — same job, opposite direction |
| `pipeline/try-desugaring.md` | `doc/desugar.md`'s try pass — confirms ours is *not* a `?` desugaring |
| `pipeline/explicit-drop.md`, `pipeline/drop-elaboration.md`, `pipeline/scope-end.md` | `design/drop.md` |
| `pipeline/desugaring-bindings.md` | `design/generic.md` — making types explicit as syntax |

**The technique worth stealing** is its "Extra Language Features" chapter:
eighteen invented constructs (`move` expressions, `let place` aliases,
`copy!`/`move!`, explicit scope-end markers, unique-immutable borrow) whose only
purpose is to keep each step's output valid and readable. We have the same
constraint — every pass must produce valid Rust — and no escape hatch, which is
why `doc/desugar.md` parks OCaml's reference erasure in the translator: "resulting
Rust is not valid, so it stays in the translator." A small, explicitly marked
language extension would let that become a shared pass. Worth considering when a
second backend wants a lowering that Rust cannot spell.
