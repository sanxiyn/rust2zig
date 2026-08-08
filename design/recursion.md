# Definition order and recursion (OCaml)

How the OCaml backend orders the definitions it emits, and how it spells
recursion. **Level 1 implemented** in `src/translate/ml/order.rs`, with the
components from `petgraph` ([a decided addition](#the-algorithm-petgraph) to
`[dependencies]`). Driven by `rust/random`, whose `impl` block defines its
methods in the order they are used rather than the order OCaml needs. The OCaml
snippets here are verified against OCaml 5.4.1.

This is an OCaml-only concern. Zig's top-level declarations are order
independent, so the Zig backend emits items in source order and never has to
think about it.

## The mismatch

A Rust `impl` block is an unordered namespace: a method may call one defined
below it, and two methods may call each other. OCaml's `let` is a sequential
binding — a name must be bound before it is used, and recursion has to be asked
for with `rec`.

`rust/random` is the minimal case. Source order is `new`, `new_inc`,
`rand_u32`, and each one calls the next:

```
lib/lib.ml, line 8:
        new_inc seed default_inc
        ^^^^^^^
Error: Unbound value new_inc
```

Nothing about this is specific to integers or to `random`; it is the first
fixture whose Rust happens to be written in the order OCaml rejects.

## The rule

> Emit definitions in dependency order, grouping mutually recursive ones.

Concretely, over the graph whose nodes are the definitions a scope emits and
whose edges are "uses":

| Component | Emitted as |
|-----------|------------|
| singleton, no self-edge | `let f = ...` |
| singleton with self-edge | `let rec f = ...` |
| two or more members | `let rec f = ... and g = ...` |

which is the condensation of the graph into strongly connected components,
emitted in topological order.

## Granularity: strongly connected components, not items

A plain topological sort is the obvious reading of the problem and it is
insufficient, because a cycle has no valid order. Rust permits mutual
recursion, so the cycles are reachable input, and the only lowering that
expresses one is `let rec ... and ...`.

No current fixture needs it. `rust/regex` is the likeliest candidate and its
parser is a DAG: `parse` -> `parse_primitive` -> `char` / `span_char` / `bump`,
with no edge back. So a plain topological sort would pass every fixture today.
It is still the wrong target: `rust/regex` is a recursive-descent parser in
embryo, and the first `parse_group` that calls back into `parse` turns a working
backend into output that does not compile. SCC is not more work than the sort —
Tarjan produces the components *and* their topological order in one pass — so
there is no reason to build the version that has to be replaced.

Computing the components also subsumes `body_is_recursive` in `item.rs`, which
today decides `RecFlag` on its own. A self-recursive function is a singleton
component with a self-edge, and it falls out of the same pass.

## Building the graph

**Edges come from SCIP symbols, not from names.** `rust/regex` defines five
distinct `new`s — `Span::new`, `Position::new`, `ParserBuilder::new`,
`Parser::new`, `ParserI::new` — and matching on the identifier fuses all five
into one node, manufacturing cycles that do not exist and forcing unrelated
functions into a `let rec` group. Symbols separate them exactly:

```
random 0.1.0 impl#[Rand32]new().
random 0.1.0 impl#[Rand32]new_inc().
```

Note that `body_is_recursive` *is* name matching (it compares a path's last
segment against the enclosing function's name), so it already over-approximates.
That has been invisible because no golden contains a `let rec` at all.

**Over-approximate on purpose.** The two error directions are not symmetric:

* A **false edge** merges two functions into a larger `let rec` group. The
  output still compiles; the cost is idiom.
* A **missing edge** emits a use before its definition. The output does not
  compile.

So where a reference cannot be resolved to a symbol, fall back to name matching
and accept the spurious edges. The case that matters is `doc/desugar.md`'s
discipline: a node synthesized by a desugar pass carries a `call_site` span and
has no occurrence in the index, so SCIP cannot answer for it. `drop` elaboration
is the concrete instance, since it inserts calls to a user's `drop` method.

## Values in a recursive group

OCaml restricts the right-hand side of a `let rec` to a statically constructive
expression. A literal or a function is allowed, a computation is not:

```ocaml
let rec x = 1 and f () = x        (* fine *)
let rec x = f () and f () = 1     (* Error: This kind of expression is not
                                     allowed as right-hand side of let rec *)
```

This bounds what a component may contain, but it is not a case to handle: a
component with two or more members requires a cycle, a cycle through a `const`
requires that `const` to be cyclic, and Rust rejects that already. A computed
`const` is therefore always a singleton, and its edges still matter for
ordering — `const FOOBAR_HASH_32: u32 = fnv1a_hash_str_32(FOOBAR);` is legal
Rust against a `const fn`, and it has to be emitted after the function.

An SCC containing a non-function is thus a bug in the analysis or unsupported
input, and should fail loudly rather than emit a `let rec` OCaml will reject.

## The algorithm: `petgraph`

Tarjan's is `petgraph::algo::tarjan_scc`, and the crate is added to
`[dependencies]` rather than the algorithm being written out. Note that
`petgraph` is *already* in `Cargo.lock` — but only as a build-dependency of
`prost-build`, so this is a real addition to the shipped dependency set, which
otherwise holds five direct entries.

The case for building it was that the algorithm is about forty lines and the
adapter code (symbols to node indices) has to be written either way. The case
for buying, which wins: more graph work is expected, and one hand-rolled
Tarjan amortizes badly across a second and third algorithm. The library also
documents the guarantee this design depends on:

> The order of node ids within each scc is arbitrary, but the order of the sccs
> is their postorder (reverse topological sort).

The recursive-descent implementation it uses is irrelevant at these sizes; a
scope holds tens of definitions.

## Determinism and churn

The topological order is not unique, and the goldens need one answer. Three
separate things fix it, and they are worth keeping distinct because only two of
them are guaranteed.

**Edge direction, so no reversal.** Edges point *use to definition*: `new` ->
`new_inc`. Postorder emits a node after everything it points at, so a definition
comes out before its users, and `tarjan_scc`'s output is directly the emission
order. Getting this backwards produces a perfectly reversed file, which is worth
recognising on sight.

**Within a component: sort by source index.** The order inside an scc is
documented as arbitrary, and it is semantically irrelevant — the members are
mutually recursive — so sorting them by source position is free and makes the
output stable. Nodes are added to the graph in source order, so the `NodeIndex`
*is* the source index and no side table is needed.

**Between components: from Tarjan, never from a re-sort.** Sorting components by
their minimum source index would break the ordering outright: a component whose
lowest index is 0 may depend on one whose lowest is 5, and the sort would put
the user before the definition. The postorder is the answer and must be taken as
given.

That leaves the churn property this design wants — that items with no dependency
between them keep source order, so the existing ml goldens do not move and only
`rust/random` reorders. It is **observed, not guaranteed**: "reverse topological"
constrains only components that are actually related, and for two independent
components either relative order is a valid postorder. Source order falls out of
`tarjan_scc` visiting node identifiers in index order, which is real behaviour
but not part of its contract.

That is an acceptable bet because the failure mode is loud and harmless: if a
future `petgraph` changed its traversal, `test_ml.sh` fails with a reordering
diff across unrelated fixtures. It is a diff, not a miscompile. Making it a
guarantee instead means a stable topological pass over the condensation — Kahn's,
taking the ready component with the smallest source index — which is worth doing
only if that diff ever actually appears.

The property is worth having either way: the backend's output is meant to be
read next to its input, and gratuitous reordering costs the reader.

## Scope: two levels, and the one it does not reach

The rule applies twice, to two scopes that are ordered independently:

* the structure items inside a module — a type's methods and associated consts;
* the top-level items — modules, free functions, and consts.

It does **not** reach mutual recursion *across* modules, where type `A`'s method
calls type `B`'s and back. OCaml spells that `module rec`, which requires
writing out explicit module signatures — a different and much larger piece of
work. No fixture needs it. It should be detected and reported, not attempted.

## Not a desugar pass

Reordering items is a Rust-to-Rust rewrite that produces valid Rust, so
`doc/desugar.md`'s test would place it in `desugar/`. It belongs in the
translator anyway, for the reason that kept the integer-literal suffixing out of
`desugar/` (see the Literals section of `design/integer.md`): the passes are
shared, and reordering would churn all 14 Zig goldens to satisfy a constraint
only OCaml has.

## Worked example: `rust/random`

The impl defines `new`, `new_inc`, `rand_u32`; the uses run the same direction,
so the emitted order is the reverse:

```ocaml
module Rand32 = struct
    type t = { mutable state : int64; inc : int64 }

    let default_inc = 1442695040888963407L
    let multiplier = 6364136223846793005L

    let rand_u32 self = ...
    let new_inc seed increment = ...
    let new_ seed = new_inc seed default_inc
end
```

No component has more than one member, so no `let rec` appears. The two consts
are ordered before their users by the same edges.

## Levels

### Level 1: ordering within a scope

`tarjan_scc` over both scopes, with nodes added in source order and edges from
use to definition. `RecFlag::Recursive` when a component has more than one
member or a singleton has a self-edge, which is where `body_is_recursive` goes
away. This is what `rust/random` needs.

### Level 2: cross-module detection

Report a cycle that spans two modules rather than emitting unbuildable output.

### Level 3: `module rec`

Only if a fixture ever justifies the module signatures it requires.

## Test

| Path | Role |
|------|------|
| `rust/recursion`, `zig/recursion.zig`, `ml/recursion` | golden triple |
| `rust/random`, `ml/random` | golden pair (not yet landed) |

`rust/recursion` is the fixture for the `rec` half: `fib` is a self-edge and
`is_even` / `is_odd` are a two-member component, so it is the only thing in the
suite that exercises `let rec` and `let rec ... and ...` at all.

```ocaml
let rec is_even n = ... is_odd (n - 1) ...
and is_odd n = ... is_even (n - 1) ...
```

It is worth having in the Zig suite too even though Zig needs no ordering: it
pins that the shared desugar passes leave a recursive call alone.

`rust/random` now emits in dependency order — `rand_u32`, `new_inc`, `new_`,
the reverse of source order, as in the [worked
example](#worked-example-rustrandom). Ordering was one of its blockers rather
than the only one, so the golden pair is still not landed; see the Test section
of `design/integer.md` for the rest (`rotate_right`, `wrapping_shl`, the
struct-literal `0L`, and a `wrapping_*` whose receiver is a call into `core`).
Associated consts are also dropped entirely today, which is a separate gap in
`translate_impl`.

All eleven existing ml goldens are byte-identical after the change, which is the
[churn property](#determinism-and-churn) holding: every one of them is already
written in dependency order, and independent definitions kept their source
positions.

Before `rust/recursion` was added, nothing in the suite was recursive at all —
`rust/gcd` is iterative and no golden contained a `let rec` — so deleting
`body_is_recursive` would have removed the only mechanism producing `rec` with
no test covering the replacement.

## Alternatives considered

* **Emit every definition in one `let rec ... and ...` group.** Needs no graph
  and no sort, and is correct for any input without a value in the group.
  Rejected as anti-idiomatic: it declares that everything is mutually recursive,
  which is false and reads as noise, and it changes shadowing semantics for
  every binding at once (`doc/ml.md` notes `let rec` restricts the right-hand
  side and changes shadowing).
* **Reorder in a desugar pass.** See [above](#not-a-desugar-pass).
* **Write Tarjan's by hand** rather than taking the dependency. See
  [above](#the-algorithm-petgraph).
* **Plain topological sort, no components.** See
  [above](#granularity-strongly-connected-components-not-items).
* **Require the input to be written in dependency order.** Effectively what
  happens today. Rejected because it is a constraint on the *Rust*, invisible
  until the OCaml fails to build, and no Rust programmer would think to obey it.
* **Sort alphabetically, or emit consts first then functions.** A cheap
  approximation that happens to fix `rust/random`. Rejected because it is not a
  property of the dependencies at all — it fixes this fixture by luck and fails
  on the next one.
