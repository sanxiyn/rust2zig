# Result and `?`

Status and roadmap for translating Rust's `Result<T, E>` and the `?` operator.
**Zig: level 1 implemented**, levels 2 and 3 are not. **OCaml: implemented**,
except for a `?` outside statement position. The emitted output is verified
against Zig 0.16.0 and OCaml 5.4.1. Driven by `rust/regex`, whose parse methods return
`Result<T> = core::result::Result<T, Error>` and propagate with `?`.

The two backends share a fixture and almost nothing else. Everything from here
to the OCaml section is about Zig, where the whole design is forced by one
constraint; the OCaml section is short because that constraint does not exist
there.

## The constraint that shapes everything

**Zig errors carry no payload.** An error set is a set of bare names:

```zig
const E = error{ Parse: Span };   // error: expected '}', found ':'
```

There is no other way to attach data to an error value, so the natural mapping
`Result<T, E>` -> `E!T` is available **only when `E` is payload-free** — a unit
struct, or an enum whose variants have no fields. Everything below follows from
that split.

This is not a corner case. `rust/regex`'s `Error` is a unit struct and so
qualifies, but the real `regex-syntax` it was extracted from has
`ast::Error { kind: ErrorKind, pattern: String, span: Span }`, which does not.
The fixture was hand-stripped into exactly the case that works — another
instance of "bitset is not fixedbitset". Level 1 satisfies the fixture and does
**not** generalize to the crate it came from.

## Level 1: payload-free `E` -> error union

### The rule

> `Result<T, E>` translates to Zig `E!T` when `E` is payload-free: a unit
> struct, or an enum with no data-carrying variant.

The gate is nearly free to compute. `Enum.has_data` is already collected in
`analyze` for exactly this shape of question, and for a struct error it is
`fields.is_empty()`.

### The mapping

| Rust | Zig |
|------|-----|
| `struct Error;` | `const Error = error{Error};` |
| `enum Error { Eof, Overflow }` | `const Error = error{ Eof, Overflow };` |
| `Result<T, E>` | `E!T` |
| `Ok(x)` | `x` (coerces) |
| `Err(Error::Eof)` | `error.Eof` |
| `e?` | `try e` |
| `r.unwrap()` | `try r` |
| `match r { Ok(v) => a, Err(e) => b }` | `if (r) \|v\| a else \|e\| b` |
| `assert_eq!(Ok(x), r)` | `expectEqual(x, r)` |

A fieldless Rust error enum becoming a Zig error set is the sweet spot: the
variant names map straight across and the result reads like hand-written Zig.
The unit-struct case is the awkward one — `error{Error}` is a one-member set
whose member repeats the type name.

An inferred error set (`fn parse(...) !Ast`) is also legal and sidesteps naming
the set at all, but it drops the declared error type from the signature. Prefer
the explicit set; keep inference in reserve for cases where the set is hard to
name.

### Prerequisites (both done)

* **`Item::Type` (type alias).** `type Result<T> = core::result::Result<T, Error>`
  must be resolved to discover `E = Error` before any of this can fire. The
  alias is a dependency of the feature, not a co-benefit of it. It is now the
  `type_alias` desugar pass, shared by both backends, so `Result<T>` reaches
  the translator already expanded to `Result<T, Error>`. See `TYPE.md`.
* **`Expr::Try` is two translations, not one.** Rust's `?` also applies to
  `Option`, where the Zig is `orelse return null` rather than `try`. Which one
  it is turned out not to need the operand's type at all: rust-analyzer records
  the dispatched `Try` impl at the `?` token itself
  (`option/impl#[`Option<T>`][Try]branch().` and its `Result` twin), so the two
  cases separate by moniker, the same shape as every other intrinsic here.

### Known gap even within level 1

Rust's `?` performs `Err(From::from(e))`, so it converts between error types.
Zig error sets coerce only by subset — there is no implicit conversion — so a
`?` that crosses error types needs an explicit `catch |e| return convert(e)`.
Level 1 is gated on the operand's error type matching the function's, and
leaves a `TODO` when they differ. No fixture exercises the gate: writing one
needs a second error type and a `From` impl between the two, and that impl
would itself take the error type out of the error-set mapping, since the
mapping requires the type to have no impls.

## Level 2: tagged union fallback

When `E` carries a payload, `Result<T, E>` is translated as an ordinary
data-carrying generic enum:

```zig
union(enum) { ok: T, err: E }
```

The appeal is that this needs **no new type machinery** — it is exactly what the
backend's existing generic enum translation already emits, so only `?`, `Ok`,
and `Err` need a second code path. `Ok(x)` and `Err(e)` become variant
constructions and the `match` becomes an ordinary switch.

The cost is `?`, which stops being a keyword and becomes a switch that early-
returns the error variant — a labeled block in expression position. That is not
Zig anyone would write by hand, which is why this is a fallback rather than the
target.

## Level 3: out-of-band diagnostics

The idiomatic Zig answer for a payload-carrying error is to return a
payload-free error and carry the detail beside it. This is what `std.json` and
`std.zig.Ast` do.

```rust
pub struct Error { kind: ErrorKind, pattern: String, span: Span }

impl Parser {
    fn parse(&self) -> Result<Ast, Error> { ... }
}
```

becomes

```zig
const Diagnostic = struct { kind: ErrorKind, pattern: []const u8, span: Span };

const Parser = struct {
    diagnostic: ?Diagnostic = null,

    fn parse(self: *Parser) error{Parse}!Ast {
        // on failure:
        self.diagnostic = .{ .kind = .unexpected, .pattern = ..., .span = ... };
        return error.Parse;
    }
};
```

and callers read `p.parse() catch { const d = p.diagnostic.?; ... }`.

Two things make this more than a rename:

* **It needs somewhere to put the diagnostic.** The transformation applies only
  when the fallible function is a method on a stateful receiver that can hold
  the field. A free function returning `Result<T, RichError>` has no such place,
  and would need the diagnostic threaded through as an out-parameter.
* **It requires a mutable receiver**, so it interacts with the receiver rules in
  `CELL.md`: a `&self` method that records a diagnostic has to be promoted to
  `*Self` on the same grounds a `Cell` write is.

It also changes semantics in a way the other levels do not: error detail becomes
state on the parser rather than a value, so it is overwritten by the next
failure and is only meaningful immediately after one.

## OCaml: the standard library `result`

OCaml has no levels. `('a, 'b) result` is an ordinary variant that carries its
payload, so the mapping is unconditional:

> `Result<T, E>` translates to `(t, e) result`.

There is no payload-free gate, no error set, no error-scope tracking, and no
fallback representation, because the constraint that produced all of those is a
Zig fact and not a functional-language one. `regex-syntax`'s real
`ast::Error { kind, pattern, span }` — the case that forces levels 2 and 3 on
the Zig side — needs nothing special here at all.

### The mapping

| Rust | OCaml |
|------|-------|
| `enum Error { Eof, Overflow }` | `type error = Eof \| Overflow` (already emitted) |
| `Result<T, E>` | `(t, e) result` |
| `Ok(x)` | `Ok x` |
| `Err(e)` | `Error e` |
| `let x = e?;` | `let* x = e in` |
| `r.unwrap()` | `Result.get_ok r` |
| `match r { Ok(v) => a, Err(e) => b }` | `match r with Ok v -> a \| Error e -> b` |
| `assert_eq!(Ok(x), r)` | `assert (Ok x = r)` |

The type mapping and both constructors are nearly free: `translate_type`
already lowers a path type with its arguments in order, so `(int, error)
result` falls out once the alias is expanded (`TYPE.md`), and `Ok` already
crosses over unchanged. Only `Err` needs renaming, to OCaml's `Error`.

### `?` is a binding operator

Rust's statement-position `?` is exactly OCaml's `let*`:

```rust
let sum = add(a, b)?;
div(sum, c)
```

```ocaml
let ( let* ) = Result.bind in
let* sum = add a b in
div sum c
```

The operator is defined **locally, in each function that needs it**, rather
than once in a prelude. That is what keeps the `Result` and `Option` flavors
from colliding: both want the name `let*`, and scoping each to its own function
means neither has to be renamed, nor wrapped in a `Result_syntax` module the
reader then has to open (fact 2 below).

Which flavor to define is decided the same way the Zig backend picks `try` from
`orelse`: rust-analyzer records the dispatched `Try` impl at the `?` token, so
the moniker says `Result.bind` or `Option.bind` without typing the operand.

This shape also makes the `From`-conversion gate free. Zig needs an explicit
check that the operand's error type matches the function's; in OCaml
`Result.bind` fixes both error types to be the same, so a `?` that crosses
error types is a type error the compiler reports (fact 3), and there is nothing
for the translator to verify.

### Mid-expression `?`

`let*` binds a whole statement, so it cannot express a `?` in the middle of an
expression — `self.parse_primitive()?.into_ast()`, from `rust/regex`. The
rewrite that fixes this is hoisting:

```rust
let tmp = self.parse_primitive()?;
tmp.into_ast()
```

which is valid Rust, so it belongs in desugar rather than the translator, as an
OCaml-only pass — the same shape as `compound_assignment`, which is already
OCaml-only. The Zig backend does not want it: `try` works mid-expression.

Until that pass exists, a mid-expression `?` should stay a `TODO`. Only
`rust/regex` has one, and it is unbuildable for other reasons.

### Prerequisites

* **Type aliases**, done — see `TYPE.md`. This was the change that turned
  OCaml's wrong-arity `int result` into `(int, error) result`.
* **Core enum members in patterns.** `Translator::is_variant` in the OCaml
  backend asks `Scip::kind_at`, but an external symbol like
  `result/Result#Ok#` carries no `SymbolInformation` in the crate's own index,
  so the kind is `None` and `Ok(v)` falls through to the catch-all
  `Pattern::Var("_")`. Today `match eval(a, b, c) { Ok(value) => value, ... }`
  therefore emits two `| _ ->` arms with `value` unbound. Ok/Err (and Some/None)
  patterns have to route through `check_moniker` the way the Zig backend does.
  This is a silent-miscompile bug independent of `Result` and worth fixing on
  its own.

### Steps

1. Core enum members in patterns via monikers (the prerequisite above). **Done.**
2. `Err` -> `Error`, in expression and pattern position. **Done.**
3. `r.unwrap()` -> `Result.get_ok`, and `Option::unwrap` -> `Option.get`. **Done.**
4. `?` in statement position as a locally defined `let*`, flavor chosen by the
   `Try` moniker. **Done.**
5. `ml/calc`, regenerated and checked by `test_ml.sh` and `test_test.sh`. **Done.**
6. The hoisting desugar pass for mid-expression `?`, when a buildable fixture
   needs it. Not done — a `?` elsewhere is a `TODO`.

The emitted OCaml, for the `?` and `match` rows:

```ocaml
let eval a b c =
    let ( let* ) = Result.bind in
    let* sum = add a b in
    div sum c

let eval_or a b c default =
    match eval a b c with
    | Ok value -> value
    | Error _ -> default
```

## OCaml facts

Verified against OCaml 5.4.1, by running the snippets.

| # | Case | Result |
|---|------|--------|
| 1 | `(int, error) result` with `Ok` / `Error`, matched and compared with `=` | compiles and runs |
| 2 | `let ( let* ) = Result.bind in` defined inside a function, and `Option.bind` under the same name in another | compiles — binding operators are ordinary local bindings, so the two flavors never collide |
| 3 | `let*` over a `(int, e1) result` inside a function returning `(int, e2) result` | type error, `Type e1 is not compatible with type e2` |

Fact 2 is what makes the per-function definition preferable to a prelude or a
syntax module. Fact 3 is the `From`-conversion gate, enforced by the type
checker rather than by us.

## Zig facts

Verified against Zig 0.16.0. Zig analyzes lazily, so each snippet was
referenced from a test — an unused function is not checked at all, and a
verification that forgets this reports success without compiling anything.

| # | Case | Result |
|---|------|--------|
| 1 | `error{ Parse: Span }` | syntax error — errors take no payload |
| 2 | Payload-free `E!T` with `try`, `if (r) \|v\| … else \|e\|` | compiles and runs |
| 3 | `expectEqual(value, error_union_expr)` | compiles — compares through the union, no unwrap |
| 4 | Inferred error set `fn f() !T` | compiles |

Fact 1 is the whole reason this document has three levels instead of one.
Fact 3 matters because it is the fixture's assertion shape: `assert_eq!(Ok(x), r)`
needs no unwrapping on the Zig side.

## Test

| Path | Role |
|------|------|
| `rust/calc`, `zig/calc.zig` | Zig level 1: fieldless enum error, alias, `?`, `Ok`, `Err`, `unwrap`, `match`, and `?` on `Option` |
| `rust/calc`, `ml/calc` | The same fixture for OCaml: `(int, error) result`, `Error`, `let*` over both `Result.bind` and `Option.bind`, `Result.get_ok`, and an `Ok`/`Error` match |
| `rust/regex`, `zig/regex.zig` | Level 1: unit-struct error, `?`, `Ok` |

`rust/calc` is a bounded calculator: `add` and `div` return
`Result<u32> = core::result::Result<u32, Error>` over a fieldless
`enum Error { Overflow, DivideByZero }`, `eval` propagates with `?`, `eval_or`
matches on the result, and `half` / `quarter` carry the `Option` half of `?`.
It exists because `rust/regex` cannot be translated yet — the unit-struct error
row above is still unexercised.

The same fixture covers the OCaml mapping: every row of its table appears in
it, and `dune runtest` on the generated tree is checked by `test_test.sh`
alongside `cargo test` and `zig test`, so the three translations are held to
the same assertions.

No fixture exercises levels 2 or 3. A payload-carrying error would need one —
the obvious candidate is restoring `regex-syntax`'s real `ast::Error`, which is
also what would make the fixture stop being a hand-stripped special case.

`zig/regex.zig` does not exist yet: the fixture still hits unrelated gaps
(`Box`, `Vec`, `loop`, `char`, and the field/method collisions), so both suites
report `SKIP regex`.

## Not implemented yet

1. Step 6 above: the hoisting desugar pass, so a `?` outside statement position
   translates for OCaml.
2. Level 2 as the fallback when the payload-free gate fails. A `Result` whose
   `E` carries a payload currently emits a `TODO` type. OCaml needs no
   equivalent.
3. Level 3, once a fixture justifies it. OCaml needs no equivalent.

An error type with impl blocks is also excluded from level 1, since a Zig error
set has no namespace for methods. The natural answer is to emit them as free
functions taking the error, but no fixture asks for it yet.

### Not planned

* `?` with `From` conversion between distinct error types.
* `Box<dyn Error>`, trait-object errors, and the `anyhow` / `thiserror` idioms.
* `Result` combinators (`map_err`, `and_then`, `ok_or`, ...).
