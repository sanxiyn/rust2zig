# Coverage Report

Run via `./coverage.sh`, which runs `test.sh` (all 20 fixtures: bitset,
closure, direction, div, divmod, dot, drop, drop2, gcd, geometry, geometry2,
hash, inc, iter, min, option, random, ratio, result, sum) under
`cargo-llvm-cov`, with the prost-generated `target/.../out/scip.rs` excluded via
`--ignore-filename-regex`.

Totals: **91.0% regions (5276/5800), 90.4% lines (2565/2836), 94.9% functions
(260/274)**.

| File | Regions | Lines | Functions |
|---|---|---|---|
| desugar/binary.rs | 100.00% | 100.00% | 2/2 |
| desugar/compound_assignment.rs | **0.00%** | **0.00%** | 0/3 |
| desugar/generic.rs | 91.12% | 95.83% | 14/14 |
| desugar/integer_literal.rs | 77.33% | 90.24% | 3/3 |
| desugar/match_ergonomics.rs | 92.96% | 95.35% | 6/6 |
| desugar/mod.rs | 100.00% | 100.00% | 1/1 |
| main.rs | 95.83% | 90.00% | 1/1 |
| print/zig.rs | 93.91% | 92.34% | 38/38 |
| scip.rs | 91.26% | 97.48% | 14/14 |
| translate/name.rs | 98.15% | 97.14% | 4/4 |
| translate/zig/call.rs | 97.48% | 97.64% | 9/9 |
| translate/zig/closure.rs | 86.50% | 92.47% | 8/8 |
| translate/zig/drop.rs | 84.25% | 85.71% | 40/45 |
| translate/zig/expr.rs | 88.78% | 87.69% | 36/39 |
| translate/zig/flow.rs | 89.47% | 85.41% | 11/11 |
| translate/zig/generic.rs | 95.65% | 95.65% | 5/5 |
| translate/zig/item.rs | 92.39% | 86.96% | 26/27 |
| translate/zig/mac.rs | 89.25% | 87.23% | 7/8 |
| translate/zig/mod.rs | 96.38% | 96.58% | 10/10 |
| translate/zig/pat.rs | 85.54% | 84.78% | 2/2 |
| translate/zig/rename.rs | 96.51% | 98.37% | 13/13 |
| translate/zig/stmt.rs | 96.08% | 94.16% | 7/8 |
| translate/zig/ty.rs | 96.18% | 95.52% | 3/3 |

## Two structural findings first

**`src/desugar/compound_assignment.rs` is dead code (0%, 0/3 functions).**
It is declared `#[allow(unused)]` in `desugar/mod.rs` and `desugar()` never
calls `compound_assignment::run`. So `x /= y` is *not* desugared to
`x = x / y`; it stays a compound assignment and is handled directly in
`translate_binary`, which is why `Node::AssignDiv` / `AssignMul` / `AssignMod` /
`AssignSub` exist. Either wire the pass in and delete the compound-assign arms
in `expr.rs`/`print/zig.rs`, or delete the pass. Right now both paths exist and
neither is exercised (see the operator gap below).

**No fixture ever produces a `TODO` marker.** Every `Node::Todo` construction
site in the translator, plus the `Node::Todo` printing arms in `print/zig.rs`
(lines 105-106, 131-134, 449-450) and `item_kind` in `item.rs` (the entire
function, one of the missed functions), is unhit. All 20 fixtures translate
completely. That is the healthy reading of most of the "uncovered" list below:
a large share of it is fallback code that fires only on input the translator
cannot handle.

## Genuine gaps worth a fixture

### 1. Operators (biggest single cluster)

Not one of the 20 fixtures uses `-=`, `*=`, `/=`, `%=`, `&=`, `||`, `>=`, `<=`,
or `wrapping_sub`. This leaves matching holes in three files:

- `expr.rs` `translate_binary`: `BitAndAssign`, `DivAssign`, `MulAssign`,
  `RemAssign`, `SubAssign`, `Ge`, `Le`, `Or` arms (lines 115-136), and
  `Wrapping::Sub` (line 92) in `translate_wrapping_assign`.
- `call.rs` line 122: `Wrapping::Sub => Node::SubWrap`.
- `print/zig.rs`: the corresponding `Node::AssignBitAnd`, `AssignDiv`,
  `AssignMod`, `AssignMul`, `AssignSub`, `AssignSubWrap`, `BoolOr`,
  `GreaterOrEqual`, `LessOrEqual`, `SubWrap` cases, in both the expression
  printer (lines 346-373) and `binop` (lines 460-487).

This is ~40 uncovered lines across three files that one small fixture closes.
It also matters more than it looks: `-=` and `-%=` are the arms where a
copy-paste slip (`-` printed for `-%`) would be invisible.

### 2. Drop analysis: block and match-arm exits (`drop.rs`, 50 lines)

`drop.rs` is the least-covered translator file, and the gaps are in the
path-sensitive move analysis, exactly where correctness is subtle:

- `continue_after` (lines 340-360) is **entirely uncovered** — a nested
  `{ ... }` block or an `unsafe { ... }` block as a statement inside a scope
  that has a drop binding. `drop`/`drop2` do use `unsafe` blocks, but only in
  functions with no `Drop`-typed binding.
- `analyze_stmts_inner` lines 320-324: the `Expr::Block` / `Expr::Unsafe`
  statement arms that feed `continue_after`.
- `analyze_branch_expr` lines 418-426: a match arm or `else` branch whose body
  is directly `return x` or a bare `if` (not a block). Fixtures always write
  `if early { return t.id; }`, i.e. the return is a *statement inside a block*,
  which goes through the covered `analyze_stmts_inner` path instead.
- `analyze_match` lines 401-407: the fallthrough-with-tail case (a `match` that
  is not the tail expression and has statements after it).
- `MoveFinder`'s overrides (lines 440-464): `visit_expr_assign`
  (`y = t` — move by assignment), `visit_expr_method_call` (`t.m(u)` — move of
  `u` as an argument), `visit_expr_reference` (`&t` is not a move). Four of the
  five missed functions in the file are these. The non-move rules are asserted
  by no test, so a regression that treats `&t` as a move would go unnoticed.
- `type_needs_drop` line 171-172: the name-suffix fallback returning false.
- `scope_move_status` line 271: the `exits.is_empty()` early return.

Suggested fixture (`drop3`): a `Drop` type moved inside a nested block, moved
via assignment, moved as a method argument, and borrowed with `&` — plus a
`match` arm body that is a bare `return x`.

### 3. Option-match cold paths (`expr.rs` lines 316-382)

`translate_match_option` covers the plain `Some`/`None` shape well, but the
whole guard-and-wildcard machinery is unhit:

- `option_pat`: `Some(pat)` with more than one element (line 316), and the
  `None` arm (line 324) — no fixture matches `None` other than as the final
  arm, which returns early at line 342.
- Every guard path: `Some(x) if g` builds the nested `if` (covered), but the
  `(pat, guard)` catch-all at 356-372 — a guarded `None` arm, a guarded `_`
  arm, and the `(None, None) => Todo` bail — is entirely uncovered.
- Line 382: falling off the end of the arms (a match whose last arm has a
  guard) returns `Todo("match")`.

Suggested fixture: `match opt { None if flag => a, Some(x) => b, _ => c }`.

### 4. For-loop shapes (`flow.rs`, 27 lines)

Every `Node::Todo("for")` bail is uncovered (lines 36, 51, 54, 73, 76, 118,
121) — these are malformed/unsupported shapes and mostly defensive. Two are
not:

- Line 88-91 in `translate_for_range`: the `None` branch of `Scip::type_at` on
  the loop variable, i.e. a range loop where the variable's type is unresolved,
  emitting `const x = _x;` without the `@intCast`.
- Lines 99-101: a **closed range with a non-literal end** (`for i in a..=n`),
  which emits `a..(n + 1)`. Fixtures only use closed ranges with literal ends,
  where the `+1` is constant-folded at translation time. This is a real
  translation rule with no test.
- Lines 147-149, 158, 172, 189, 198-201: `peel_iter` falling through to a
  non-`.iter()` receiver, and `iter_by_ref` returning `None` for a
  non-path/non-array/non-slice iterable — e.g. `for (i, e) in xs.enumerate()`
  where `xs` is not a bare identifier.

### 5. Closures (`closure.rs`, 27 regions)

- Line 85: a closure parameter whose type SCIP cannot resolve
  (`Node::Todo("type")`).
- Line 91: `closure_return_type` returning `None` — a closure returning `()`,
  emitted as `void`. No fixture has a unit-returning closure.
- Line 101: a closure with a **braced body** (`|x| { ... }`). Every fixture
  closure is a bare expression, so only the implicit-return branch is tested.
- Lines 25, 35, 59, 61: `collect_captures` rejecting an ident (non-variable
  symbol, definition inside the closure span, no resolvable type).

Suggested fixture: a closure with a braced body that returns nothing and
captures a local.

### 6. Patterns (`pat.rs`, 84.78% — lowest line coverage of the translator)

- `pat_name` line 19: a non-`Ident` pattern (`_` as a loop/closure variable)
  falling back to `"_"`.
- `translate_match_pat` lines 62-64: **literal patterns in a match**
  (`Pat::Lit`) — integer, string, and the `Todo` fallback. No fixture matches
  on a literal at all.
- Lines 69-70: the `Todo("match pat")` catch-all.

Suggested fixture: `match n { 0 => .., 1 => .., _ => .. }`.

### 7. Types (`ty.rs`)

- Lines 28/31: `Option<...>` whose path arguments are absent or not a type —
  the two `Todo("type")` bails in the `Option` arm.
- Line 79: the type catch-all (e.g. `fn`, trait object, `impl Trait` in a
  position that reaches `translate_type`).

### 8. Statements and items (small)

- `stmt.rs` line 39: `Stmt` other than `Expr`/`Local`/`Macro` (e.g.
  `Stmt::Item` — a nested `fn`).
- `stmt.rs` line 47: a tuple `let` whose element is not an ident
  (`let (a, _) = ...`).
- `stmt.rs` line 70: a `let` whose pattern is neither tuple, wild, nor ident.
- `stmt.rs` lines 110/141: minor branch pairs in the drop-flag and block-expr
  paths.
- `item.rs` lines 92-95: a non-`Fn` item inside an `impl` (an associated type,
  say) becoming `Todo("impl item")`.
- `item.rs` lines 186-194: the **alive-flag preamble for a `&mut`/by-value
  parameter that needs a conditional drop** — i.e. a function taking an owned
  `Drop` value that it moves on some but not all paths. `drop`/`drop2` exercise
  the flag machinery for *locals* (`stmt.rs`) but not for *parameters*.
- `item.rs` line 231: a function parameter whose pattern is not an ident.

### 9. Macros (`mac.rs`)

`translate_println` (lines 43-45) is a stub returning `Todo("println")` and is
uncovered — no fixture calls `println!`. Note the README still documents
`println!` translation and `test.sh`'s sed hack for format specifiers; that
support is gone from the translator. Worth reconciling.

## Defensive / not worth a fixture

- `main.rs` lines 16-17 (usage error) and line 41 (`_ => unreachable!()` in the
  backend dispatch — unreachable by the `BACKENDS` check above it).
- `print/zig.rs` lines 11 and 250: `panic!` on a wrong root/block node.
- `scip.rs` lines 87, 140, 163: missing-symbol and malformed-signature bails.
- `name.rs` line 6: the `result.push('_')` in `camel_to_snake`. Its callers all
  pass enum variant names, and every fixture variant is a single word (`Some`,
  `Ok`, `North`, ...), so the separator is never inserted. A two-word variant
  (`NotFound` -> `not_found`) would cover it; cheap to fold into any new enum
  fixture.
- `translate/zig/mod.rs` lines 89-90: the `println` moniker entry and the
  unknown-moniker `return false`; lines 131/137/182: `symbol_at` returning
  `None` during `analyze`, and a capture-stack miss in `translate_path`.
- `rename.rs` line 57: `Pat::Reference` in `bind_pat` (`ref x` bindings reach
  it only through the ergonomics desugar, which rewrites to `Pat::Ident` with
  `by_ref` instead).
- `desugar/generic.rs` lines 27/29/61/126/142 and `desugar/integer_literal.rs`
  lines 29/30/52/55: the "not applicable" sides of each rewrite guard
  (no type param found, non-primitive operand type, already-suffixed literal).
- `desugar/match_ergonomics.rs` line 60: `ref mut` binding generation — a
  `&mut` match scrutinee, which the README already lists as a known bug
  (`translate_match_arm` ignores the mutability). Covering it means fixing it.

## Suggested fixtures, ranked

1. **arith** — `-=`, `*=`, `/=`, `%=`, `&=`, `>=`, `<=`, `||`, `wrapping_sub`.
   Closes ~40 lines across `expr.rs`, `call.rs`, `print/zig.rs`, and forces a
   decision on the dead `compound_assignment` pass.
2. **drop3** — moves in nested/`unsafe` blocks, move by assignment, move as a
   method argument, `&t` non-move, `match` arm returning directly, and an
   owned-`Drop` *parameter* conditionally moved. Closes most of `drop.rs` and
   `item.rs` 186-194, in the file where untested branches are riskiest.
3. **match_lit** — matching on integer/string literals plus a guarded `None`
   arm on an `Option`. Closes `pat.rs` 62-70 and `expr.rs` 356-382.
4. **range** — `for i in a..=n` with a non-literal `n`. One real translation
   rule, currently untested.
5. **closure2** — braced-body closure returning `()` with a capture.
