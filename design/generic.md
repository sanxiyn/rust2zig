# Generics (Zig)

How Rust type parameters become Zig `comptime` parameters. **Level 1
implemented**, split across two places: `src/desugar/generic.rs` decides the
call sites and `src/translate/zig/generic.rs` + `item.rs` emit the
declarations. Driven by `rust/iter` (a generic free function) and `rust/option`
(a generic enum with a generic method). The emitted output is exercised by
`zig test` through `test_test.sh`.

This is a Zig-only concern. OCaml infers parametric polymorphism, so its backend
erases type parameters entirely and skips the desugar pass — see
[Other backends](#other-backends).

## The mismatch

Rust and Zig both monomorphize, and that is where the similarity stops.

In Rust a type parameter is a separate binder with its own syntax, and at a call
site it is *inferred*: `position(l, v)` never mentions `i32`, and nothing in the
source records that this is the `i32` instantiation. In Zig there is no generic
system at all — a type is an ordinary value of type `type`, a type parameter is
an ordinary parameter marked `comptime`, and a generic type is a *function from
types to a type*:

```zig
fn position(comptime T: type, l: []const T, v: T) ?usize { ... }

fn Option(comptime T: type) type {
    return union(enum) { some: T, none };
}
```

Nothing about this is inferred. `position(l, v)` is an arity error; the caller
writes `position(i32, l, v)`.

So the translation has two obligations, and only the first is syntactic:

1. Turn each declared type parameter into a parameter Zig can see.
2. **Reconstruct what rustc inferred** at every call site, because Zig will not
   infer it and the Rust source does not say it.

Obligation 2 is the whole difficulty. It is the reason this feature needs an
analysis at all, rather than being a shape-for-shape rewrite like most of the
translator.

## The rule

> A Rust type parameter becomes a Zig `comptime T: type` parameter, and every
> call site is given its type argument explicitly.

Three declaration positions, three spellings:

| Rust | Zig |
|------|-----|
| generic struct / enum `Option<T>` | `fn Option(comptime T: type) type` returning the container |
| generic free fn `fn position<T>(l, v)` | `comptime T: type` first, before the value params |
| generic method `fn and<U>(self, optb)` | `comptime U: type` after `self`, before the value params |

**Why methods put the comptime params after `self`.** Zig's `a.m(x)` is sugar
for `m(a, x)`: the receiver is always the first argument. A comptime parameter
declared ahead of `self` could not be reached by dot-call syntax at all, so the
only placement that keeps method calls spellable is immediately after it —
`y.@"and"(i32, z)`.

**A type's parameters are bound by the container function, not by its methods.**
`Option<T>`'s `T` is bound once by `fn Option(comptime T: type) type` and is in
scope for every method inside the returned container, so the `impl<T>`'s own
`T` is erased rather than re-emitted. This is why `register_generic` reads
`signature.generics` only — the method's *own* parameters (`U` in `and`) — and
never the impl's. `Self` is `@This()` inside the container, so a method can name
the instantiated type without respelling `Option(T)`.

**Bounds are dropped.** `T: PartialEq` and `T: Copy` have no Zig counterpart:
comptime duck-types at instantiation, and a body that uses an operation the
argument type lacks fails there. Dropping a bound therefore cannot produce
silently wrong output — it can only move an error from the definition to the
instantiation, which is Zig's normal behaviour anyway.

That is the current behaviour and it stays correct, but it is not the end of the
story: `design/bound.md` designs a `comptime` check that puts a bound back, for
bounds naming a *user-declared* trait. Two of its findings bear on this document.
A bound the body never exercises is dropped with no error anywhere, which is the
one case where dropping is a real loss rather than a relocation. And an
`impl<T: Bound>` whose bounds are stronger than its type's cannot have them
checked in the container function, even though the type parameter itself is
bound there — bounds do not erase with the parameter.

## The call site is a desugar pass

Obligation 2 could be discharged during emission, and originally was. It now
happens earlier: `src/desugar/generic.rs` rewrites `position(l, v)` into
`position::<i32>(l, v)` and `y.and(z)` into `y.and::<i32>(z)` before the
translator runs.

**Turbofish is the interchange format.** It satisfies `doc/desugar.md`'s test —
the output is still valid Rust, and it means the same thing the original did —
and it collapses inference into something the translator can read structurally.
The move is a standard one: *Rust via Desugarings* plans the same thing in its
[Desugaring Bindings](https://nadrieril.github.io/rust-via-desugarings/pipeline/desugaring-bindings.html)
step, whose `SUMMARY.md` carries the notes "explicit types on all bindings" and
"explicit types on generic calls" (unwritten as of this citation). Writing
inference down as syntax, so that later stages read rather than solve, is what
that pipeline does throughout.

Emission then has no analysis left in it: `translate_call` reads the path
segment's angle-bracketed arguments, `translate_method_call` reads
`ExprMethodCall::turbofish`, and both just translate the types and put them in
front of the value arguments. The two syntactic forms Rust uses for explicit
type arguments are also the two the translator already had to walk.

**How the inference is reconstructed.** Rust's inference is a solver; this is
not, and does not need to be. For each generic function, `analyze_signature`
records one `GenericArgRef { arg, path }` per type parameter: *which* value
parameter mentions it, and *where inside that parameter's type* it sits, as a
sequence of generic-argument positions. Bare `T` is `path: []`, `Option<T>` is
`[0]`, `HashMap<K, T>` is `[1]`. At a call site, `Scip::type_at` on the
corresponding argument gives its concrete type and `peel_type` walks the
recorded path to pull the substitution out.

That is unification restricted to the one shape that matters — a parameter type
is a path with generic arguments, and the argument's concrete type has the same
shape — with rustc's answer available for cross-checking, since the fixture has
to compile as Rust first.

**Worked example — `rust/iter`.** `fn position<T: PartialEq>(l: &[T], v: T)`.
`find_type_param` inspects `l: &[T]` first and returns `None`: it only descends
`Type::Path`, and a reference is not one. It then reaches `v: T` and records
`GenericArgRef { arg: 1, path: [] }`. At the call, `Scip::type_at` on `v` is
`i32`, the empty path peels nothing, and the turbofish is `::<i32>`, giving
`position(i32, l, v)`. Note what this shows about the reference gap: it costs
nothing as long as *some* parameter mentions `T` outside a reference.

**Why not OCaml too.** The pass produces valid Rust, so it is shareable in
principle, but `desugar_ml` does not run it. OCaml needs no explicit type
arguments, so the turbofish would be pure noise there — and the pass would fail
to insert one exactly where OCaml is still perfectly happy.

## The two sides must agree

The declaration side and the call side are computed by different code from
different data, and Zig checks them against each other by arity. They currently
diverge:

* `desugar/generic.rs` handles a function only when **every** type parameter is
  locatable in some value parameter (`refs.push(found?)`); otherwise it inserts
  no turbofish.
* `translate/zig/generic.rs`'s `register_generic` applies **no** such condition:
  it registers every type parameter it finds, and `comptime_params` emits one
  `comptime T: type` for each.

So a function with a type parameter that cannot be located — the return-only
`U` of `Option::map` is the motivating case (see `TODO.md`) — gets a comptime
parameter on the definition and no argument at the call, which Zig rejects as an
arity error. No fixture hits this today, since a function that fails the
locatability test is one no current example calls.

The failure mode is loud, so this is a gap rather than a hazard. But the
invariant is worth stating: **whatever decides the call site must be the same
predicate that decides the declaration.** The cleanest fix is to make the
translator read the desugar pass's verdict instead of recomputing a weaker one
— the turbofish is already in the AST by then, so a registered function could be
required to carry one at every call.

## Implementation

1. Desugar (`src/desugar/generic.rs`)
   * `Collect` walks every `syn::Signature` and records
     `HashMap<symbol, Vec<GenericArgRef>>`, keyed by SCIP symbol so two
     same-named methods on different types stay distinct.
   * `Desugar` rewrites `visit_expr_call_mut` and `visit_expr_method_call_mut`,
     and descends into macro token streams (`visit_macro_mut`) because generic
     calls habitually sit inside `assert_eq!`.
2. Analyze (`src/translate/zig/mod.rs`, `generic.rs`)
   * `register_generic` records `GenericFn { type_params }` for each free fn and
     each impl method, keyed by SCIP symbol.
3. Declarations (`item.rs`, `print/zig.rs`)
   * `comptime_params` turns `type_params` into `Param { comptime: true, ty: type }`,
     placed first for a fn and after the receiver for a method.
   * `Node::EnumDecl` carries `type_params`; a non-empty list makes the printer
     emit the `fn Name(comptime T: type) type { return ...; }` wrapper instead of
     a plain `const Name = ...`.
4. Types (`ty.rs`)
   * A path type with angle-bracketed arguments that is not one of the
     moniker-recognized types becomes `Node::Call` — `Option<U>` is `Option(U)`,
     which is exactly a call of the container function.
5. Call sites (`call.rs`)
   * Turbofish types are translated and pushed ahead of the value arguments.

## Test

| Path | Role |
|------|------|
| `rust/iter`, `zig/iter.zig` | generic free function; comptime param first; `T` located past an unusable `&[T]` |
| `rust/option`, `zig/option.zig` | generic enum (container function) and generic method (comptime param after `self`) |

`rust/option` is the only fixture covering the container-function form and the
method-position form; `rust/iter` is the only one covering the free-function
form. Both cover bound-dropping (`T: PartialEq`, and `Option`'s `impl<T>`).

## Implemented

### Level 1: parameters locatable in argument types

Everything above: `comptime T: type` on definitions, the container function for
generic types, turbofish insertion at call sites, and type arguments emitted
ahead of value arguments.

## Not implemented yet

### Return-only type parameters

`Option::map<U, F: FnOnce(T) -> U>` — `U` appears only in the return type, so
`find_type_param` cannot locate it and no turbofish is inserted (while the
definition still grows a `comptime U: type`; see
[the two sides](#the-two-sides-must-agree)). `TODO.md` sketches the two ways
out: require the type at the call site as an explicit argument, or derive it in
Zig with `@TypeOf(f.call(undefined))`. The explicit argument is the cheaper
first cut and the one to take, since it also gives the `Fn`-bound work in
`design/closure.md` something to call.

### Generic structs

`Node::StructDecl` has no `type_params` field, so `struct Pair<T>` emits as a
plain `const Pair = struct { ... }` with `T` dangling. The enum path already has
the whole mechanism — the container function and its printer branch — so this is
a matter of carrying the same field through `translate_struct`. No fixture
declares one.

### Type parameters behind references

`find_type_param` descends `Type::Path` only, so `T` in `&T`, `&[T]`, or `[T; N]`
is invisible. It costs nothing when another parameter mentions `T` directly (as
in `rust/iter`), and skips the function entirely when none does — the natural
`fn first<T>(l: &[T]) -> &T` is exactly that case. Peeling a reference and a
slice before recursing is the fix, but it needs a matching peel at the call site,
where `Scip::type_at` on the argument returns the *reference* type.

### Const generics and lifetimes

Both are filtered out by the `GenericParam::Type` match. Dropping lifetimes is
correct — they are erased throughout. Dropping a const parameter is not: `N` in
`fn zeros<const N: usize>()` would become an unresolved identifier. Zig's
`comptime N: usize` is the natural target, so this is mostly a matter of widening
the filter and carrying the parameter's type instead of assuming `type`.

### Non-ident call arguments

`resolve` requires the argument carrying the instantiation to be an
`Expr::Path`, since it asks `Scip::type_at` at that ident. `position(l, 3)` or
`position(l, f(x))` therefore resolves nothing and the call gets no turbofish.
Widening this to the shapes `expr_type` already answers (calls, field accesses,
casts) would cover most of the rest.

## Other backends

OCaml needs none of this. Its `let` is inferred polymorphic, so a generic
function's type parameters are simply erased and the inferred type is at least
as general as the Rust one; a generic enum becomes `type 't t`, with the
parameter written on the type declaration. Both the desugar pass and the
comptime machinery are skipped. `doc/ml.md` states the consequence for methods:
"generic methods need no per-instantiation machinery".

The split is a useful check on where the work belongs. Reconstructing the
instantiation is what a *monomorphizing* target needs, not what Rust semantics
require, which is why the pass is Zig's alone even though its output is
backend-neutral Rust.

## Alternatives considered

* **`anytype` parameters.** Zig's own idiom for the simple cases, and it needs
  no call-site rewriting whatsoever — `fn position(l: anytype, v: anytype)`
  compiles and the caller writes what it always wrote. Rejected because it
  cannot name the type: `Option(U)` as a return type, `[]const T` as a parameter
  type, and any body that says `T` all need a binder, and `anytype` provides
  none. It also silently discards the relationship Rust states — that `l` and
  `v` share one `T` — turning a definition-site contract into a body-site
  accident. `@TypeOf(v)` can recover a name at the cost of writing it at every
  mention, which is worse than the parameter.
* **Resolve instantiations during emission** (where this started). The
  translator queried `Scip::type_at` at the call and peeled the recorded path
  itself. Rejected in favour of the desugar pass: the reconstruction is not a
  Zig fact, it is a Rust fact, and expressing it as turbofish makes it visible in
  a form that is checkable by eye and reusable by any other monomorphizing
  backend.
* **Hand-monomorphize: emit one specialized copy per instantiation.** Needs no
  comptime at all and would work for any target. Rejected because Zig already
  has the feature — emitting `position_i32` and `position_u8` throws away the
  generic the source wrote, multiplies the output, and produces Zig no human
  would write, which is the project's whole criterion.
* **Infer at the call site from the argument's Zig type** rather than its Rust
  type, e.g. emitting `@TypeOf(v)` as the type argument. Always available and
  needs no `GenericArgRef` machinery. Rejected as noise at every call site for a
  type that is nearly always spelled out concretely one line above.
