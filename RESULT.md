# Result and `?`

Status and roadmap for translating Rust's `Result<T, E>` and the `?` operator
into Zig. **Designed, not implemented.** The level 1 mapping is verified against
Zig 0.16.0. Driven by `rust/regex`, whose parse methods return
`Result<T> = core::result::Result<T, Error>` and propagate with `?`.

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

### Prerequisites

* **`Item::Type` (type alias).** `type Result<T> = core::result::Result<T, Error>`
  must be resolved to discover `E = Error` before any of this can fire. The
  alias is a dependency of the feature, not a co-benefit of it.
* **`Expr::Try` is two translations, not one.** Rust's `?` also applies to
  `Option`, where the Zig is `orelse return null` rather than `try`. The
  operand's type decides which.

### Known gap even within level 1

Rust's `?` performs `Err(From::from(e))`, so it converts between error types.
Zig error sets coerce only by subset — there is no implicit conversion — so a
`?` that crosses error types needs an explicit `catch |e| return convert(e)`.
Level 1 should be gated on the operand's error type matching the function's.

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
| `rust/regex`, `zig/regex.zig` | Level 1: unit-struct error, `?`, `Ok` |

No fixture exercises levels 2 or 3. A payload-carrying error would need one —
the obvious candidate is restoring `regex-syntax`'s real `ast::Error`, which is
also what would make the fixture stop being a hand-stripped special case.

`zig/regex.zig` does not exist yet: the fixture still hits unrelated gaps
(`Box`, `Vec`, `loop`, `char`, and the field/method collisions), so both suites
report `SKIP regex`.

## Not implemented yet

Everything. In rough order:

1. `Item::Type` alias resolution (prerequisite).
2. Level 1 for a payload-free `E`, gated on the error type matching across `?`.
3. `?` on `Option` -> `orelse return null`.
4. Level 2 as the fallback when the gate fails.
5. Level 3, once a fixture justifies it.

### Not planned

* `?` with `From` conversion between distinct error types.
* `Box<dyn Error>`, trait-object errors, and the `anyhow` / `thiserror` idioms.
* `Result` combinators (`map_err`, `and_then`, `ok_or`, ...).
