# Coverage Report

Run via `./coverage.sh` against the 9 fixtures in `test.sh` (direction, div,
divmod, gcd, geometry, option, ratio, result, sum). Coverage is **93%
regions / 91% lines** across `src/`, with all 55 functions hit. The
prost-generated `target/.../out/scip.rs` is excluded via
`--ignore-filename-regex`.

Recent additions to the `geometry` fixture closed: multi-arg method call,
multi-arg unnamed enum variant, struct-payload variant (`Dot(Point)`,
`Line(Point, Point)`), named-field variant (`Circle { center, radius }`,
hitting `Pat::Struct`, struct-literal-as-enum-variant in `translate_struct_expr`,
and `Fields::Named` in `translate_enum`), non-generic enum-with-data, and
`ReturnType::Default`.

Below: cases the test fixtures still never exercise, grouped by file. The
catch-all `_ => "/* TODO: ... */"` arms in every `match` are also unhit;
only the specific gaps that look worth a fixture are called out.

## src/translate/pat.rs (80% lines)

`translate_pat`:
- `Pat::Wild` — `let _ = ...` / `_` patterns.

`translate_match_pat`:
- `Pat::Ident` — binding a whole match scrutinee to a name.
- `Pat::Wild` — `_ =>` arm. None of the fixtures use a wildcard arm; all
  matches are exhaustive over named variants.

## src/translate/ty.rs (81% lines)

- `Type::Array` — entire arm uncovered. `sum` test has an array local but it
  goes through the `stmt.rs` array-with-`[_; N]` special case, never reaching
  `translate_type` for an `Array`.
  - Sub-branch: non-literal length (falls back to `_`).
- `Path` arm, generic-arg branches:
  - `Option<T>` where `segment.arguments` is not `AngleBracketed` (the
    implicit None branch on `if let Some(...)`).

## src/translate/expr.rs (90% lines)

`translate_binop` — these operators are never used by any fixture:
- `DivAssign` `/=`, `MulAssign` `*=`, `RemAssign` `%=`, `SubAssign` `-=`
- comparisons `Ge >=`, `Gt >`, `Le <=`, `Lt <`
- `Sub -` (subtraction!) — surprising gap; `gcd`/`divmod` use `%` not `-`.

Other unhit branches:
- `translate_lit`: non-bool/int/str literal (e.g. `Lit::Float`, `Lit::Char`).
- `translate_field`: `Member::Unnamed` (tuple `.0` access). `divmod` fixture
  destructures via `let (a, b) = ...`, doesn't index.
- `translate_for_loop`: the non-array fallback `/* TODO: for */`. Also the
  `else { false }` branch when `efl.expr` isn't a `Path` — every for-loop in
  fixtures iterates a bare identifier.
- `translate_if`: the `Some` `if let` matched-but-`check_moniker`-fails path,
  and the `Pat` non-`TupleStruct` path inside `if let`.
- `translate_panic`: malformed panic (no closing quote) — defensive only.
- `translate_println`: the empty-format-string branch
  (`std.debug.print("\n", .{})`) is unhit; all `println!()` calls have
  content.
- `translate_struct_expr`: `Member::Unnamed` (tuple-struct literal) in the
  non-variant branch.

## src/translate/item.rs (93% lines)

- `translate_item`: `Item` other than Enum/Struct/Fn/Impl (catch-all).
- `translate_enum`:
  - generic enum with no data (`is_generic && !has_data`) — the `Option`-like
    pure-tag generic. `option` fixture uses `core::Option`, not a user-defined
    one.
  - non-`ImplItem::Fn` items inside an `impl` (consts, types, etc.).
- `translate_struct`: struct with no impls — `ratio`/`geometry` are the only
  struct fixtures and both have methods. A plain data struct is uncovered.
  - non-`ImplItem::Fn` items inside a struct `impl`.
- `translate_fn_arg`: `FnArg::Receiver` at the free-function level
  (`/* TODO: self */`) — defensive, shouldn't happen in valid Rust.

## src/translate/stmt.rs (91% lines)

- `Stmt::Local` with `Pat::Type` whose pat isn't `Pat::Ident` (the inner
  `else { false }` of the mutability check).
- `Stmt::Local` with neither `Pat::Ident` nor `Pat::Type` (catch-all
  `_ => false`).
- Array-with-`[_; N]`-type local where `ta.len` is not `Expr::Infer` — the
  `if matches!(...)` false branch. Only the `[_; N]` form is used.
- `translate_macro` returning false from a `Stmt::Macro` (unknown macro
  fallback).
- `Stmt` variants other than `Expr`/`Local`/`Macro` (e.g. `Stmt::Item`).

## src/translate/mod.rs (97%)

- `check_moniker` with no symbol at the span (returns `false`) — the
  `let Some(symbol) = ... else { return false }` `None` arm.
- `analyze`: `impl` block whose `self_ty` resolves to a name not in `enums` or
  `structs` (orphan impl, e.g. `impl Trait for ExternalType`).

## src/main.rs

- The `args.len() != 2` usage-error path.

## Suggested fixtures to close gaps

A handful of small fixtures would lift most of the above:

1. **arith** — exercise `-`, `<`, `<=`, `>`, `>=`, `-=`, `*=`, `/=`, `%=`.
2. **wildcard** — `match` with `_ =>` arm and a `let _ =` binding.
3. **tuple_index** — `t.0` field access and a tuple-struct literal.
4. **plain_struct** — a struct with no `impl` block.
5. **array_param** — function taking `&[T; N]` to hit `Type::Array`.
