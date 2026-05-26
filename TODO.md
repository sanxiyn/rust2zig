# TODO

## Option::map

Goal: translate `Option::map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U>`
and a test like `Option::Some(3).map(i32, |x| x + 1)` (explicit `U`).

### Step 1: unify closure translation (DONE)

All closures now emit as `struct { ... fn call(self/_: @This(), ...) R
{ ... } }{ ... }`. Non-capturing closures use `_: @This()` and `{}`;
capturing ones use `self: @This()` and `{ .field = value, ... }`. Every
closure binding is recorded in `closures`, so `translate_call` appends
`.call` uniformly. Existing `closure` example regenerated.

### Step 2: closure parameters as callees (DONE)

`translate_call` no longer tracks a `closures` set; instead it queries
the callee ident's `Scip::type_at` and appends `.call` when
`is_closure_type` matches an `impl Fn`/`FnMut`/`FnOnce` bound. Because
this is type-driven, closure-typed *parameters* (like `f` in `map`) route
through `.call` for free. Open: a comptime `F: Fn(..)` param's SCIP type
may not present as `impl Fn(..)` — verify once Step 4 lands.

### Step 3: return-only generic params

`register_generic` skips a function if any type param is unfindable in
param types via `find_type_param`. `U` in `Option::map` only appears in
the return type, so the function is skipped today.

Options (pick one):

* **Explicit type argument at call site (recommended for first cut).**
  Allow `GenericArgRef` to carry "no source" entries; at call sites,
  expect the user-written call to include the type as an explicit
  argument (`opt.map(u32, f)` translates to
  `opt.map(u32, T_of_f, f.call_struct)` after step 2). Cheap to
  implement: relax the skip rule, and at the call site read the
  explicit `syn::GenericArgument::Type` from the method's turbofish
  or — if Rust source omits it — fall back to a type-arg slot in
  the call expression.
* **Inference from the closure's return type.** Use Zig
  `@TypeOf(f.call(undefined))` to derive `U`. Avoids the explicit arg
  but the Zig output is ugly and depends on `undefined` being legal
  there.

Start with the explicit-arg approach. Update the README's
`register_generic` paragraph to mention return-only slots.

### Step 4: `Fn`-trait bounds on generics

When emitting `comptime F: type` for a type param whose bound is
`F: FnOnce(T) -> U` (or `Fn`/`FnMut`), no extra Zig annotation is
needed — the body uses `f.call(...)` and Zig comptime checks the
shape at instantiation. Just don't emit a TODO for the bound; ignore
it during signature translation.

### Step 5: add `map` to option example

In `rust/option/src/lib.rs`:

```rust
pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Option<U> {
    match self {
        Option::Some(x) => Option::Some(f(x)),
        Option::None => Option::None,
    }
}

#[test]
fn test_map() {
    let x: Option<i32> = Option::Some(3);
    assert_eq!(4, x.map(|v| v + 1).unwrap());
}
```

Regenerate SCIP, run `test.sh` and `test_test.sh`.

## Slicing

Goal: translate `a[i..j]` (an `Expr::Index` whose index is a
`Expr::Range`) to a Zig slice expression.

`translate_index` currently handles only scalar indexing (`a[i]`).
Branch on the index being a range and emit Zig's slice syntax, with the
range rewrites:

* `a[i..j]` -> `a[i..j]`
* `a[i..]`  -> `a[i..]`
* `a[..j]`  -> `a[0..j]` (Zig has no open-start sugar)
* `a[..]`   -> `a[0..]`
* `a[i..=j]` -> `a[i..j + 1]` (reuse the closed-range `+1` logic from
  `translate_for_range`)

Add a natural example (the user is choosing one) covering at least the
half-open and open-start cases, then regenerate SCIP and run both suites.
