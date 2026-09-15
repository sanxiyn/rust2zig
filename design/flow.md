# Control flow

How `if`, `while`, `loop`, `for`, and the non-local exits (`break`, `continue`,
`return`) are translated. **`if`, `while`, `loop`, `for`, and all three exits
are implemented in every backend.** Labels and `break` with a value are
implemented nowhere.

`match` is `design/match.md`, `if let` is `design/option.md`, `?` is
`design/result.md`, and what an early exit does to a live droppable value is
`design/drop.md`.

## The axis is imperativeness, and it runs the other way

Every other document in this series ends up saying the same thing: the
constructs Rust inherited from ML -- `Option`, `Result`, enums, structural
equality -- cost OCaml nothing and cost Zig a design document.

Control flow inverts it exactly. **Rust's loops and jumps are Zig's loops and
jumps, almost token for token**, so the Zig backend mostly passes them through.
Common Lisp has a loop facility that is *more* expressive than Rust's. And
OCaml has no `break`, no `continue`, and no early `return` at all -- it buys all
three with exceptions, and its `for` over a sequence is not a loop but a
higher-order function.

So this is the one feature where the functional target pays the most.

## `if`

An `if` is an expression in all three targets, and mostly translates directly.
One decision is worth recording:

**Zig excludes a tail-position `if` from the implicit return.** `translate_stmt`
turns a block's final expression into `return expr`, except when it is an `if`
(`stmt.rs:19`), and instead each branch gets its own `return`:

```rust
fn min(a: i32, b: i32) -> i32 { if a < b { a } else { b } }
```

```zig
fn min(a: i32, b: i32) i32 {
    if (a < b) { return a; } else { return b; }
}
```

Zig's `if` *is* an expression, so `return if (a < b) a else b;` would also work
-- but only while each branch is a single expression. A branch with statements
needs a labeled block (`blk: { … break :blk v; }`) to produce a value, and
pushing the `return` inward avoids ever needing one. The cost is that the
emitted shape does not match the source's; the benefit is that it never has to
change shape as a branch grows.

Common Lisp emits `(if c a b)`, and an `if` with no `else` is `when`, which
takes its body directly rather than needing a `progn`. OCaml emits
`if … then … else`, where a missing `else` obliges the then-branch to be `unit`
-- which is exactly the shape a translated `continue` exploits below.

## `while` and `loop`

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `while c { ... }` | `while (c) { ... }` | `(loop while c do ...)` | `while c do ... done` |
| `loop { ... }` | `while (true) { ... }` | `(loop do ...)` | `while true do ... done` |
| `while let ...` | marker | marker | marker |

Common Lisp's `loop` is emitted in the *extended* form (`(loop do …)`) rather
than the simple one, so that the implicit `nil` block exists and `(return)` --
which is how `break` translates -- has something to return from. Verified: a
clause-free `(loop do …)` with a `(return i)` inside works on both
implementations.

Zig's `loop` reuses `Node::While` with `true` as the condition, so it needs no
node of its own and no printer change. A labeled `loop` still emits a marker,
like a labeled `break`.

What made it more than the one-line addition it looked like is **tail
position**. Rust's `loop` types as `!`, so a function whose body ends in one --
exiting only by `return` from inside, which is how `rust/regex`'s parser is
written -- puts a `loop` where `translate_stmt` applies its implicit return.
Wrapping it would emit `return /* TODO: expr */;` and drop the entire loop body,
since `Node::While` has no expression form in `print::zig`. So `loop` joins `if`
in `is_statement_like`, the set of tail expressions the implicit return skips
-- `if` because the `return` is pushed into its branches, the loops because
there is no value to return at all. `while` and `for` were in the same latent
hole and are now excluded too, though no fixture ever reached it: every existing
fixture ends in a value (`total`, `a`, `None`) rather than in a loop.

**OCaml pays for the same `!` in a different currency.** `while true do ... done`
is the loop, but OCaml's `while` is `unit` where Rust's `loop` is `!`, and a
`loop` is precisely the construct that leaves by an exit rather than by falling
off the end. So `translate_loop` splits on whether the body breaks:

| Rust | OCaml | type |
|---|---|---|
| `loop { ... break ... }` | `try while true do ... done with Exit -> ()` | `unit` |
| `loop { ... return v ... }` | `while true do ... done; assert false` | `'a` |

The second is the interesting one. With no `break`, the loop is left only by a
`Return` exception or never, so the `while` is genuinely unreachable-past --
and `assert false` is OCaml's spelling of `!`, typing as `'a` so a
tail-position `loop` unifies with whatever the function returns. Without it,
`try (while ... done) with Return r -> r` is a type error: `unit` body against an
`int` handler. Verified, parentheses included -- the printer emits
`assert (false)` and the polymorphic typing survives, which the `digits` case
proves by returning `int`.

This also closed a **latent bug in `while`**. `translate_while` never called
`wrap_break`, so a `while` containing a `break` emitted `raise Exit` with no
handler and would have escaped the function at runtime. No fixture had that
combination -- `rust/gcd`'s `while` has no `break` -- so nothing caught it. Both
loops now route through the same wrapper.

`while let` is unhandled everywhere for one reason: `syn::Expr::Let` in
condition position reaches the expression dispatcher and lands on its catch-all.
Zig is the backend that would get it cheapest -- `while (e) |x|` is native
syntax for exactly this -- and Common Lisp would reuse the `if let` bind-and-test
unchanged.

## `for`: five shapes, and three philosophies

Rust's `for` is sugar over `IntoIterator`, and **no backend implements the
protocol**. Five shapes are recognized syntactically instead, with monikers
where a method has to be identified (`std::iter::zip`,
`core::iter::Iterator::enumerate`, `core::slice::iter`):

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `for x in arr` | `for (xs) \|x\|` | `for x across xs` | `Array.iter (fun x -> …) xs` |
| `for x in &slice` | `for (xs) \|*x\|` | `for x across xs` | -- |
| `for i in a..b` | `for (a..b) \|_i\|` + preamble | `for i from a below b` | `for i = a to b - 1 do` |
| `for i in a..=b` | `for (a..b+1) \|_i\|` + preamble | `for i from a to b` | `for i = a to b` |
| `for (x, y) in zip(a, b)` | `for (a, b) \|x, y\|` | two parallel `for … across` clauses | `Array.iter2 (fun x y -> …) a b` |
| `for (i, e) in l.iter().enumerate()` | `for (l, 0..) \|e, i\|` | `for i from 0 for e across l` | -- |

The three columns are three different relationships to the problem:

* **Common Lisp maps.** `loop` already has every one of these as a clause --
  `across` for sequences, `from`/`below`/`to` for counters, and parallel `for`
  clauses that step together, which is what `zip` and `enumerate` need. The
  backend translates shapes into clauses and has no lowering to do. `to` even
  gives the inclusive range for free.
* **Zig builds.** Its `for` takes arrays, slices, and ranges, and everything
  else is constructed: `a..=b` becomes `a..(b+1)` (folded when the end is a
  literal); a range capture is `usize`, so the body gets a preamble
  `const i: T = @intCast(_i);` with the real type from SCIP; and `enumerate`
  flips operand order, since Rust yields `(index, element)` and Zig's index
  operand comes last.
* **OCaml calls a function.** There is no `for … in` over a sequence, so the
  loop becomes `Array.iter` with the body as a closure -- and *that* is what
  makes its non-local exits hard, because a `break` out of a closure is not a
  jump out of a loop. Its one native loop, `for i = a to b`, is inclusive at
  both ends (verified), which is why the half-open case emits `- 1`.

Anything outside the five shapes -- a real iterator chain, `.map().filter()`,
`.chars()` -- is a marker in all three.

## Non-local exits

| | `break` | `continue` | `return` from inside a loop |
|---|---|---|---|
| Rust | `break` | `continue` | `return` |
| Zig | `break` | `continue` | `return` |
| Common Lisp | `(return)` -- `loop`'s implicit `nil` block | `(return-from continue)`, with `(block continue …)` wrapped around the body **only when the body contains one** | `(return-from <fn> v)` -- the `defun`'s implicit block |
| OCaml | `raise Exit`, caught by `try … with Exit -> ()` | invert the guard: `if c then () else <rest of body>` | a locally declared `exception Return of τ`, caught immediately |

Three things follow.

**Common Lisp's mechanism is more general than Rust's.** A named block escape
covers everything Rust's exits do and more; `(return)` already carries a value,
and a labeled break is `(loop named outer …)` with `(return-from outer v)`. The
`(block continue …)` wrapper is emitted only when the body needs it, so a loop
without a `continue` pays nothing. Verified interaction, on both
implementations: in one loop carrying both exits, `(return)` leaves the loop and
`(return-from continue)` ends the iteration.

**OCaml's exceptions generalize for free.** An exception carries a payload, so
`break`-with-value is already expressible -- `exception Return of int option`
in `ml/iter` *is* that mechanism, used for an early `return`. What OCaml pays is
elsewhere: an exception is set up and caught per exit site, and the code reads
nothing like the Rust it came from.

**OCaml's `continue` is the fragile one.** `ml/sum`'s

```rust
for x in xs { if x % 2 == 0 { continue; } total += x; }
```

becomes

```ocaml
Array.iter (fun x -> if x mod 2 = 0 then () else total := !total + x) xs
```

-- the guard's `continue` became `then ()` and the *rest of the body* moved into
the `else`. That rewrite works when the `continue` is a guard at the top of the
body. A `continue` in the middle of a body, inside a nested `if`, or in one arm
of a `match` has no such inversion, and needs the same exception treatment
`break` already gets.

## `break` with a value, and labels

All three backends emit a marker for a labeled `break`/`continue` or a `break`
with a value. What each would take, in increasing order of work:

* **Common Lisp: nearly free.** `(return v)` is already the value form, and
  `(loop named outer …)` plus `(return-from outer v)` is the labeled form.
* **OCaml: reuse the payload exception.** One exception per labeled loop,
  which is the mechanism already in use.
* **Zig: needs unique labels.** `break :label value` is native, but every
  labeled block the backend emits today uses the single name `blk`
  (`BLOCK_LABEL`), so a nested one shadows its parent. That is the README's
  standing bug, and it is the same problem the `Option`-match lowering has
  (`design/option.md`) -- one label generator fixes both.

Note the ordering is the reverse of the usual: the feature is cheapest in the
Lisp backend and most work in the Zig one.

## What this costs the drop analysis

`design/drop.md`'s levels are ordered by control flow, not by types: level 1 is
unconditional destruction, level 2 is conditional, and **level 3 is loops** --
precisely because `break`, `continue`, and an early `return` leave a scope with
live values in it. Every exit above is a place where a `defer` has to fire, and
the reason the drop analysis stops where it does.

## The holes, ranked

1. **OCaml `continue` only as a top-of-body guard.** Anything else needs an
   exception. Unexercised -- `ml/sum`'s is a guard -- so it is a latent
   restriction rather than a visible failure, and it should become a marker
   before it becomes a wrong answer.
2. **Labels and `break` with a value**, everywhere; cheapest in Common Lisp,
   blocked in Zig on unique block labels.
3. **`while let`**, everywhere; native in Zig, free in Common Lisp.
4. **Iterator chains.** The five recognized shapes cover the fixtures and
   nothing beyond them. A general answer needs an iterator protocol per target
   -- `doc/scheme.md` reaches the same conclusion for a fourth backend and
   proposes SRFI-158 generators, which is the only place in the tree where the
   general problem has been thought about at all.

## Test

| Path | Role |
|------|------|
| `rust/sum` | Three loop shapes in one fixture: `for x in &slice`, `for i in 0..n`, `for x in [T; N]`, plus a `continue` -- and it is the fixture that shows OCaml's guard inversion |
| `rust/gcd` | `while` with a mutated condition, and `mut` parameters rebound (`var a = _a;` in Zig, `let a = ref _a` in OCaml) |
| `rust/iter` | Both `break` and an early `return` from inside a loop, in the same file: `(return)` versus `(return-from position2 i)` in Lisp, `raise Exit` versus `exception Return` in OCaml |
| `rust/dot` | `std::iter::zip`: multi-object `for` in Zig, parallel `for` clauses in Lisp, `Array.iter2` in OCaml |
| `rust/geometry`, `rust/div` | Tail-position `if`, with the return pushed into the branches |
| `rust/regex` | Unfixtured; the `loop`, and the only iterator chain in the tree |

**`loop` itself is unfixtured, in both backends that implement it.** It is
verified against a two-function scratch crate -- a `loop` with a `break` in
non-tail position, and a tail-position `loop` exiting by `return` -- which is
what forced Zig's `is_statement_like` change and OCaml's `assert false`, and
which passes under `cargo test`, `zig test`, and `dune runtest` alike. But
nothing in `rust/` pins it, so no golden file would catch a regression.
Fixturing it needs a name: `loop` is a Rust keyword and Cargo rejects it as a
package name. The same scratch crate covers `while` + `break`, the latent bug
above, which is equally unpinned.

No fixture has a labeled loop, a `break` with a value, a `while let`, or a
`continue` anywhere but the top of a body. The first three are markers today;
the fourth is the one that would be silently wrong, and only in OCaml.

## Not implemented yet

1. Labels and `break` with a value.
2. `while let`.
3. `continue` from a non-guard position in OCaml.
4. Iterator chains beyond the five shapes.

## Not planned

* Implementing `IntoIterator` faithfully. Rust's `for` desugars to
  `loop { match it.next() { … } }`, which is correct and reads terribly in every
  target. The shape-recognition approach exists precisely to emit the loop a
  human would write, and it degrades to a marker rather than to bad code.
* `goto`-style flow, or reconstructing Rust's MIR-level control flow graph. The
  input is structured, and every target's structured forms are enough.
* `async` and generators.
