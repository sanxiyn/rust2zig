# Closures (Zig)

How a Rust closure becomes an anonymous Zig struct with a `call` method.
**Level 1 implemented** in `src/translate/zig/closure.rs`, with the call-site
half in `call.rs` and the printer half in `print/zig.rs`. Driven by
`rust/closure`. The emitted output is exercised by `zig test` through
`test_test.sh`.

This is a Zig-only concern. OCaml has closures natively and translates them
verbatim — see [Other backends](#other-backends).

## The mismatch

Zig has no closures. Not "no sugar for closures": a Zig function literal cannot
refer to a local of its enclosing function at all, and there is no function type
that carries captured state. The one composite that can is a struct — a value
holding the captured state, with a method to run the body — which is exactly the
Zig idiom, written out by hand.

Rust's closure is the same object with the construction automated: rustc
generates an anonymous struct of the captures and an anonymous `Fn*` impl, and
the closure's type is that struct's. So the translation is not an approximation
of Rust's closures — it is the *same lowering rustc performs*, made visible
because the target cannot hide it.

That reframing decides most of the design. The questions worth asking are
therefore rustc's questions — what is captured, and how — not "how do we fake a
lambda".

*Rust via Desugarings* states the same lowering as two separate steps, and the
split is worth knowing (see `PLAN.md`, [Prior art](../PLAN.md#prior-art-rust-via-desugarings)):
[Closure Capture](https://nadrieril.github.io/rust-via-desugarings/pipeline/closure-capture.html)
makes every capture explicit *while the closure is still a closure*, and
[Closure To Struct](https://nadrieril.github.io/rust-via-desugarings/pipeline/closure-adt.html)
then turns it into "a struct, with one field per `move($expr)` expression, and
that field is initialized with `$expr`", implementing the `Fn*` traits. That is
this document's rule, arrived at from the opposite direction. Where the two
differ is staging: we do both at once, in the Zig translator.

## The rule

> A closure becomes an anonymous struct value: one field per capture, and a
> `call` method holding the body.

```rust
let a = 3;
let add = |x| x + a;
```

```zig
const a: i32 = 3;
const add = struct {
    a: i32,
    fn call(self: @This(), x: i32) i32 {
        return x + self.a;
    }
}{ .a = a };
```

Two spellings, one shape:

| | receiver | instantiation |
|---|---|---|
| no captures | `_: @This()` | `}{}` |
| captures | `self: @This()` | `}{ .a = a, ... }` |

The non-capturing case still gets the struct and the `call` method even though a
plain function would do. **Uniformity is load-bearing**, not tidiness: the call
site rewrite is type-driven (below), and a closure-typed *parameter* offers no
syntax to inspect — the caller cannot know whether the closure it was handed
captured anything. One shape means `f(x)` becomes `f.call(x)` unconditionally.
The `_` receiver is forced by Zig's unused-parameter rule; the parameter itself
cannot be dropped, since dot-call syntax passes the receiver either way.

## Capture analysis

`collect_captures` walks the closure body with `syn::visit` and keeps an ident
when all of the following hold:

* it resolves to a SCIP symbol whose kind is `Variable` or `Parameter` — so
  types, functions, and fields are not captures;
* that symbol's **definition range lies outside the closure's span** — the
  definition is in an enclosing scope, which is what "capture" means;
* it is the first occurrence of that symbol (`seen`), so a variable used three
  times yields one field;
* `Scip::type_at` answers, since the field needs a type.

**Definition ranges instead of a scope stack.** The obvious implementation
maintains a stack of bound names and asks whether an ident is in it. SCIP has
already computed that: `SymbolInfo::range` is the occurrence carrying
`SymbolRole::Definition`, so "defined outside this span" is a containment test
against one number pair. It is also *correct under shadowing* for free — the
comparison is on symbols, not names, so an inner `x` and an outer `x` are
distinguishable, which a name-keyed scope stack only manages by being careful.

This is the one analysis in the project that depends on definition ranges being
**unique**, and it is why `doc/desugar.md` states the narrower rule that a
desugar pass may duplicate a span for a *use* but never for a *definition*. A
pass that cloned a `let` would give one symbol two definition sites, and
`collect_captures` would then be reading whichever one the index recorded last.

**Captures are by value.** The field holds a copy, made once when the struct
literal is evaluated. For the `Copy` scalars the fixtures capture this is
exactly Rust's behaviour. It is *not* Rust's behaviour in general — a Rust
closure borrows by default, so a non-`move` closure observing a later write to
the captured variable, or capturing a non-`Copy` value it must not move, is
mistranslated rather than rejected. See
[Not implemented yet](#not-implemented-yet); by-reference capture is a field of
pointer type and the analysis to choose it is the gap, not the emission.

## The body

Inside the body, `a` has to become `self.a`. A `capture_stack`
(`RefCell<Vec<HashMap<symbol, field>>>` on the translator) is pushed with the
symbol-to-field map before the body is translated and popped after;
`translate_path` consults it for any `Variable`/`Parameter` ident and emits a
field access on a hit.

The map is keyed by SCIP symbol, so it does not misfire on an unrelated
same-named binding inside the body. The stack is consulted at `last()` only,
which is where nested closures come apart — see the gap below.

The `RefCell` is the same device as `error_scope` and OCaml's `current_module`,
for the same reason: the translator's methods take `&self` and this is context,
not a parameter every one of them should carry.

## The call site

`translate_callee` asks `Scip::type_at` for the callee ident's type and appends
`.call` when `is_closure_type` recognizes it — an `impl Trait` whose bounds
mention `Fn`, `FnMut`, or `FnOnce`.

**Type-driven, not binding-tracking.** The first implementation kept a set of
idents bound to closure literals and appended `.call` for members of that set.
Asking the type instead is both simpler and strictly more capable: a *parameter*
of closure type, which has no literal anywhere in the function, routes through
`.call` identically. That is what makes higher-order functions reachable at all,
and it is the piece `design/generic.md`'s `Option::map` work builds on
(`TODO.md`, steps 1 and 2).

The return type comes from the same signature: `closure_return_type` parses the
binding's `impl Fn(i32) -> i32` and takes the arrow's right-hand side, falling
back to `void`. Parameter types come from `Scip::type_at` on each parameter
ident. So every type in the emitted struct is rustc's own inference, read back
out of the index — the translator infers nothing.

## Interaction with the shadowing pass

`rust/closure`'s first case is `let x = 3; let double = |x| x * 2;`, and the Zig
is `fn call(_: @This(), x2: i32)`. The rename is not closure code: the
`shadowing` desugar pass renamed the parameter because Zig rejects shadowing
across nested scopes, and by the time the closure is translated the ident simply
*is* `x2`. Worth noting because it is easy to mistake for a capture bug — and
because it is the reason the emitted struct field names are safe to use verbatim
as Zig identifiers.

## Implementation

1. `stmt.rs` — `translate_local` recognizes a `let` whose initializer is an
   `Expr::Closure` and routes to `translate_closure_local`. This is the *only*
   entry point (see the gap below).
2. `closure.rs` — `collect_captures`, `closure_return_type`, `is_closure_type`,
   `translate_closure_local`, which assembles `Node::Closure` and pushes/pops
   the capture map around the body.
3. `call.rs` — `translate_callee` appends the `.call` field access.
4. `print/zig.rs` — `closure()` writes the struct, its fields, the `call`
   method, and the instantiation.

## Test

| Path | Role |
|------|------|
| `rust/closure`, `zig/closure.zig` | both level 1 shapes: non-capturing (`_: @This()`, `}{}`) and capturing (`self`, `}{ .a = a }`) |

Nothing else in the suite contains a closure, so every gap below is uncovered by
construction.

## Implemented

### Level 1: closures bound by `let`, captured by value

The struct-with-`call` lowering, capture collection by definition range,
`self.<field>` rewriting in the body, and type-driven `.call` at call sites,
for closures that appear as the initializer of a `let` and capture `Copy`
values they do not mutate.

## Not implemented yet

### Closures outside `let` position

`translate_closure_local` is reached only from `translate_local`.
`syn::Expr::Closure` has no arm in `translate_expr`, so a closure passed
directly as an argument — `xs.map(|x| x + 1)`, the single most common way to
write one — falls to the catch-all and emits `/* TODO: expr */`. The lowering
itself does not depend on the `let`: what the `let` supplies is the binding
ident, which is where `closure_return_type` reads the `impl Fn(..) -> R`
signature. An argument-position closure needs that type from its context
instead (the callee's parameter type), which is the actual work.

### `FnMut` and mutation of captures

A capture field is written as a `const` copy inside a `@This()` value receiver,
so a body that assigns to a capture does not compile as Zig. Rust's `FnMut`
needs the receiver to be `*@This()` and the closure binding to be a `var`, and
the choice between the two receivers is a mutation analysis over the body.

### `move`, and capture by reference

Both directions are missing, and they are the same gap seen from two sides:
today every capture is by value, which is `move` semantics for `Copy` types and
nothing else. A non-`move` closure over a non-`Copy` value, or one that must
observe later writes, needs a pointer field (`a: *const i32`, initialized `&a`)
and `self.a.*` at uses.

The book's
[Closure Capture](https://nadrieril.github.io/rust-via-desugarings/pipeline/closure-capture.html)
step is the specification of what is missing: it decides a mode *per captured
place* — by value (`move(x)`), by mutable reference (`*move(&mut x)`), or by
unique-immutable borrow (`*move(&uniq x)`) — and it does so before the struct
exists at all. Two things follow for us. The mode decision belongs with
`collect_captures`, feeding the existing struct builder, rather than being a
change to the struct shape; and it is the same analysis this section and the
`FnMut` one above both need, so they are one piece of work, not two. Note also
that the book calls the analysis "straightforward" only because its earlier
steps made every place use explicit — a precondition we do not have, and the
honest reason ours will be fiddlier than its presentation suggests.

### Nested closures

`translate_path` reads `capture_stack.last()` only, and the printer initializes
each capture field with an identifier of the same name (`.a = a`). An inner
closure capturing an outer closure's capture would therefore emit `.a = a` in a
scope where the name is `self.a`. Fixing this is two changes that belong
together: consult the whole stack, and carry an initializer *expression* on the
capture rather than reconstructing it from the field name.

### Returning a closure

Each closure has a distinct anonymous type, and a Zig function's return type has
to be spellable. Returning one requires hoisting the struct to a named
container-level declaration — which is also what a closure stored in a struct
field would need.

### `Fn`-bound generic parameters

`fn map<U, F: FnOnce(T) -> U>(self, f: F)` — the bound is dropped like any
other (`design/generic.md`), and the body's `f(x)` becomes `f.call(x)` only if
`Scip::type_at` on `f` presents as `impl Fn(..)`; a comptime `F: type`
parameter may not. `TODO.md` step 4 records the intent — emit `comptime F: type`
and let Zig check the shape at instantiation — and step 2 flags the open
question.

## Other backends

OCaml needs none of this. Closures are native, capture is lexical, and
`ml/closure` is `let double x = x * 2 in ...` — the translator emits a `let`
binding with the closure's parameters as the function's, and a use is an ordinary
application. There is no capture analysis, no struct, and no `.call`.

The contrast is a good check on what is Zig's and what is Rust's:
`collect_captures` computes a fact about the *Rust* program (which bindings cross
the closure boundary) that only a target without closures has to ask.

## Alternatives considered

* **Zig function pointers** (`*const fn (i32) i32`). The direct analogue for the
  non-capturing case, and it needs no struct, no `call`, and no rewriting at the
  call site. Rejected because it cannot carry state, so capturing closures would
  need the struct anyway — and then the two kinds have different types and
  different call syntax, and every call site must know which it holds. That
  knowledge is exactly what a closure-typed parameter does not have.
* **Hoist each closure to a named top-level struct.** More conventional Zig, and
  it is what returning a closure will require. Rejected for now: it moves the
  body away from where the source wrote it, and it manufactures a name for
  something the source left anonymous. It is a change to make when a fixture
  forces it, not before.
* **Inline the closure at its call sites.** Most fixtures call a closure once,
  so substitution would erase the problem entirely. Rejected: it deletes a
  binding the source wrote, duplicates the body when called more than once, and
  does nothing at all for a closure passed as an argument — the case that
  matters most.
* **Emit the captures as a `comptime` struct type plus a separate value.**
  Separating the type from the instantiation would give the closure a nameable
  type without a top-level hoist. Rejected as two declarations where Zig's
  anonymous-struct-literal syntax already expresses one thing.
