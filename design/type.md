# Type aliases

Status and roadmap for `type Name<T> = ...`, a shared desugar pass that expands
aliases at their use sites. **Implemented** as `src/desugar/type_alias.rs`.
Driven by `rust/calc` and `rust/regex`, both of which declare
`type Result<T> = core::result::Result<T, Error>`.

## Why desugar and not the translator

The Zig backend first resolved aliases in the translator (`resolve_alias` in
`src/translate/zig/result.rs`, applied at the top of `translate_type`), because
`Result<T>` had to become `Result<T, Error>` before the level 1 mapping in
`RESULT.md` could see an error type at all. That placement is wrong on the test
`doc/desugar.md` sets: expanding an alias yields *valid Rust*, so it is
backend-neutral and belongs in desugar, where every backend gets it.

The OCaml backend is the evidence. It has no alias resolution, so `Result<u32>`
translates as the one-argument `int result`, while OCaml's own type is the
two-argument `('a, 'b) result`. The alias is not a `Result` feature; it is a
prerequisite that two backends now need for two unrelated reasons.

## The pass

`src/desugar/type_alias.rs`, first in the pass list for both backends. It
depends on nothing and `generic` benefits from seeing real types when deciding
where to insert turbofish.

```rust
pub fn run(scip: &Scip, file: &mut syn::File) {
    let aliases = collect(scip, file);          // SCIP symbol -> syn::ItemType
    if aliases.is_empty() {
        return;
    }
    Desugar { scip, aliases }.visit_file_mut(file);
    file.items.retain(|item| !matches!(item, syn::Item::Type(_)));
}

impl VisitMut for Desugar<'_> {
    fn visit_type_mut(&mut self, ty: &mut syn::Type) {
        visit_mut::visit_type_mut(self, ty);   // arguments first, spans still original
        let Some(expanded) = self.expand(ty) else { return };
        *ty = expanded;
        self.visit_type_mut(ty);               // an alias whose RHS is another alias
    }
}
```

`expand` resolves the path's last ident to a SCIP symbol, looks it up in the
alias table, zips the alias's type params against the use site's arguments, and
runs a substituting `VisitMut` over a clone of the right-hand side. Aliases are
keyed by symbol rather than by name, so a crate-local `Result` never collides
with `core`'s.

Substitution replaces a bare-ident `Type::Path` matching a param and does not
recurse into what it just inserted, so a use-site argument that happens to
mention a name the alias also uses as a param is not captured.

Expanding an alias of an alias terminates because Rust rejects cyclic aliases
(`cycle detected when expanding type alias`), so no depth guard is needed.

## Removing the item

Once every use is expanded the alias is dead code, so dropping the item leaves
valid Rust — which is what makes this a desugar rather than a translator hack.
Both backends then never see an `Item::Type`: the Zig `syn::Item::Type => None`
case goes away, and `item_kind`'s `"type alias"` string becomes unreachable.

## Span discipline

This pass synthesizes nothing. The right-hand side keeps the spans it had at
the alias definition — which is why `check_moniker` still resolves
`core::result::Result` and `Error` after expansion — and the substituted
arguments keep their use-site spans. Unlike the inserted `*` and `ref` of
`binary` and `match_ergonomics`, there is no node the translator must avoid
querying.

What is new is that the pass **duplicates** spans: the right-hand side's spans
appear once per use site. That is safe here because the occurrence map is keyed
span -> symbol, and every copy denotes the same type, so all copies resolve
identically. It would not be safe for a rewrite that duplicated a *binding*,
because `SymbolInfo::range` maps the other way — symbol -> its one definition
range — and `collect_captures` relies on that being unique. So the rule to
carry forward is narrower than "preserve original spans": **duplicating a span
is fine for uses, not for definitions.**

## Not handled

Each of these leaves the alias unexpanded rather than half-expanded, so the
failure is a `TODO` or an unresolved name, not a wrong type.

* **Alias in value position.** `Result::Ok(x)`, or `<Alias as Trait>::f()`, is
  an `Expr::Path`, which `visit_type_mut` never reaches. Handling it is path
  rewriting rather than type substitution, and wants the alias's *head* path
  rather than its whole right-hand side — `Alias::Ok` has to become
  `core::result::Result::Ok`, dropping the type arguments. No fixture needs it.
* **Type-param defaults.** `type Foo<T = u32> = ...` used as `Foo` supplies no
  argument for `T`. Substitute positionally over type params only, and leave
  the type unexpanded when the use site supplies fewer arguments than there are
  params, rather than emitting a right-hand side with `T` still in it. Applying
  the default instead is the better answer and is a small extension, but no
  fixture forces the question.
* **Const and lifetime params.** `type Buf<const N: usize> = [u8; N]` needs
  substitution into an array length, which is an `Expr`, not a `Type`.
  Lifetimes are erased by both backends anyway.
* **Macro token streams.** `visit_mut` treats a macro's tokens as opaque, so an
  alias inside `assert_eq!(...)` — in a turbofish, say — is missed. `generic`
  and `shadowing` each re-parse macro arguments to work around this;
  `doc/desugar.md` already notes that the duplication should be shared, and
  this pass would be the third caller.
* **Associated types.** `ImplItem::Type` inside an `impl` block is a different
  item kind, and resolving `Self::Item` needs the impl's context. Out of scope.

## Test

| Path | Role |
|------|------|
| `rust/calc`, `zig/calc.zig`, `ml/calc` | `type Result<T> = core::result::Result<T, Error>`, generic alias with one substituted param |
| `rust/regex` | The same alias, in the crate the feature came from |

Neither exercises a non-generic alias (`type Byte = u8;`), an alias of an
alias, or an alias to something other than `Result`.

The pass was output-neutral for the Zig backend, as expected: `zig/calc.zig`
did not move, since `resolve_alias` already did this work. What it changed is
the OCaml side, where `add` went from

```ocaml
let exception Return of int result in        (* wrong arity *)
```

to

```ocaml
let exception Return of (int, error) result in
```

so the fixture that shows the pass doing anything new is an OCaml one. `ml/calc`
now exists and asserts exactly that, the alias having been the first of the
gaps the OCaml `Result` mapping needed closed (`RESULT.md`).

## Not implemented yet

Nothing, beyond the cases under "Not handled" above, none of which a fixture
reaches.

### Not planned

* Alias in value position, until a fixture needs it.
* `type` aliases as a *target* feature: neither backend emits an alias, so a
  Rust alias never survives translation. Zig's `const Name = T;` and OCaml's
  `type name = t` would both express one, and keeping the name would read
  better than expanding it — but that is a different feature (preserving an
  abstraction) from this one (resolving it), and it conflicts with the
  `Result` mapping, which must see through the alias to work at all.
