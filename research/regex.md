# Regex parser (regex-syntax)

Status and roadmap for the first **structural** target: the `Ast` types and
character-level parser extracted from `regex-syntax`. Every prior doc in this
directory picks an arithmetic kernel and asks which *operators and intrinsics*
are missing ([hash.md](hash.md), [random.md](random.md), [isqrt.md](isqrt.md),
[float.md](float.md)); [hasher.md](hasher.md) adds traits. This one is a
different axis entirely: a small program with a **data structure and control
flow** — recursive types, heap ownership, interior mutability, a `loop`, and
`Result` + `?`. It is where "buildable Zig from a real crate" stops being about
expressions and starts being about types.

The fixture `rust/regex` is already written (262 lines, `index.scip` built).
Of the gaps below, gap 4 (`Cell` erasure) and gap 8 (irrefutable match arms) are
implemented and gap 5 was removed from the fixture; the rest of the output is
still mostly `TODO`.

## Why this crate

`regex-syntax` fails the MVP scope in its real form more comprehensively than
any previous candidate — it is a large multi-module crate with dependencies,
macros, and `alloc` throughout. The fixture is the usual hand extraction: the
`Position` / `Span` / `Ast` / `Parser` spine of `ast::parse`, with the parser
reduced to the one path that matters (`parse_primitive` on a verbatim literal).

* **It keeps `alloc`.** `Box<Ast>` and `Vec<Ast>` are not incidental — the AST
  is recursive and the concat node is variadic, so removing them removes the
  data structure. This is the first fixture that deliberately breaks the
  "no std / no alloc" constraint, and that is the point: recursion-through-a-
  pointer is unavoidable in any real parser, in Zig as much as in Rust.
* **It keeps `#[derive(Debug, PartialEq)]`.** First fixture that does. The
  derives are load-bearing for the test's `assert_eq!` but need no translation:
  Zig's `expectEqual` compares structurally. Worth confirming they can simply be
  ignored rather than stripped, which would retire one edit class from every
  future extraction.
* **It is real control flow.** `loop` + `break`, `?` on a `Result`, a `match`
  with a binding arm and a wildcard arm — none of which any curated example has.
* **The naming collisions are genuine, not contrived.** `ParserI` has a field
  `parser` *and* a method `parser()`; `Ast` has a variant `Empty` *and* a
  constructor `empty()`. Rust distinguishes both by case or namespace; the Zig
  and OCaml backends currently collapse both into one identifier. This is the
  first fixture where the collision bug the README flags as latent actually
  fires.

## Real source (essence)

```rust
pub struct Position { pub offset: usize, pub line: usize, pub column: usize }
pub struct Span { pub start: Position, pub end: Position }

pub enum Ast {
    Empty(Box<Span>),
    Literal(Box<Literal>),
    Concat(Box<Concat>),
}

pub struct Concat { pub span: Span, pub asts: Vec<Ast> }

impl Concat {
    pub fn into_ast(mut self) -> Ast {
        match self.asts.len() {
            0 => Ast::empty(self.span),
            1 => self.asts.pop().unwrap(),
            _ => Ast::concat(self),
        }
    }
}

type Result<T> = core::result::Result<T, Error>;

pub struct Parser { pos: Cell<Position> }

struct ParserI<'s> { parser: &'s mut Parser, pattern: &'s str }

impl<'s> ParserI<'s> {
    fn bump(&self) -> bool {
        if self.is_eof() { return false; }
        let Position { mut offset, mut line, mut column } = self.pos();
        if self.char() == '\n' {
            line = line.checked_add(1).unwrap();
            column = 1;
        } else {
            column = column.checked_add(1).unwrap();
        }
        offset += self.char().len_utf8();
        self.parser().pos.set(Position { offset, line, column });
        self.pattern()[self.offset()..].chars().next().is_some()
    }

    fn parse(&self) -> Result<Ast> {
        let mut concat = Concat { span: self.span(), asts: vec![] };
        loop {
            if self.is_eof() { break; }
            match self.char() {
                _ => concat.asts.push(self.parse_primitive()?.into_ast()),
            }
        }
        Ok(concat.into_ast())
    }
}
```

## Gaps

Grounded by running `cargo run -- zig rust/regex` today. Grouped by what each
gap costs, since this fixture's gap list is far longer than any previous one and
the ordering matters more than the enumeration.

### Tier 1: type-system gaps (the real content)

1. **`Box<T>` — recursion through a pointer.** `Box<Span>` renders as the
   nonexistent `Box(Span)`, and `Box::new(span)` loses its qualifier entirely,
   emitting a bare `new(span)`. Zig needs `*Span` plus an allocator, so this is
   not a type rename: it forces the first allocator decision in the project.
   The cheap first cut is `*T` with a single `std.heap` allocator threaded as a
   struct field or a file-level constant; the honest version passes
   `std.mem.Allocator` and makes every constructor fallible.
2. **`Vec<T>` -> `std.ArrayList(T)`,** with `vec![]`, `.push`, `.pop`, `.len`.
   Same allocator question. `vec![]` currently hits the macro fallback.
3. **`Result<T, E>` and `?` -> Zig error unions and `try`.** `Result<Ast>`
   renders as `Result(Ast)`; the `type Result<T> = ...` alias is
   `// TODO: type alias`; `Ok(x)` renders as an undefined `Ok(x)`;
   `syn::Expr::Try` is unhandled outright. Designed in `RESULT.md`, not
   implemented. The mapping is clean *only because this fixture's `Error` is a
   unit struct*: **Zig errors carry no payload**, so `E!T` is available just
   when `E` is payload-free. Real `regex-syntax` carries
   `Error { kind, pattern, span }` and would need the tagged-union or
   out-of-band fallback instead. Note also that `Item::Type` (type alias) is a
   *prerequisite* here rather than a co-benefit — `E` cannot be discovered
   without resolving the alias.
4. **`Cell<T>` — interior mutability — done.** `Cell<Position>` used to render
   as `Cell(Position)` with `.get()` / `.set()` passing through untouched. Zig
   has no interior mutability, so the `Cell` is erased and the mutability it
   granted is re-expressed in the pointer types: a shared reference to a
   Cell-bearing type becomes `*T` rather than `*const T`. Designed and recorded
   in `CELL.md`. Changing `ParserI` to hold `&'s mut Parser` (gap 5) put this
   fixture in the cheap case — the `Cell` is reached through a pointer, so the
   accessor return promotes and the receiver stays const, with no receiver
   promotion and no dataflow pass.
5. **Generic structs with a trait bound — removed from the fixture.**
   `ParserI` was originally `ParserI<'s, P: Borrow<Parser>>`, which got no
   `comptime P: type` on the type and emitted a bare undefined `P` for its
   field. It now holds `&'s mut Parser` outright, which both removes the gap and
   turns `Cell` into its cheap case (see gap 4 and `CELL.md`). The cost is that
   this fixture no longer exercises generic *structs* at all — existing generic
   support covers functions, methods, and enums, and that hole now needs a
   fixture of its own. The trait-bound half of the question stays with
   [hasher.md](hasher.md).
6. **`char` and `&str` internals.** `char` renders as the undefined `char`
   (Zig: `u21`), char literals `'\n'` hit `// TODO: lit`, and `.len_utf8()` /
   `.chars().next()` are unmapped. `&str` -> `[]const u8` already works, but
   string *slicing* (`self.pattern()[i..]`) is `// TODO: expr` — the slicing
   work already scoped in `TODO.md`, now with a fixture demanding it.

### Tier 2: control flow and patterns (small, mechanical)

7. **`syn::Expr::Loop`.** Unhandled; the whole `parse` body is one
   `/* TODO: expr */`. `loop { ... }` -> `while (true) { ... }`. `TODO.md`
   defers labelled `break` and break-with-value until "`loop` lands" — this is
   that fixture, though it needs only the plain form.
8. **`Pat::Wild` and `Pat::Ident` in match position** — **done.** Both used to
   emit `/* TODO: match pat */`: the `_ =>` arm of `into_ast` and the `c =>`
   binding arm of `parse_primitive`. Now `else =>` and `else => |c|`, via an
   `Option<Node>` pattern on `SwitchArm` and an `Accessor::Whole` capture. The
   wrinkle worth recording: syn parses an unqualified unit variant as
   `Pat::Ident`, so the binding case has to be gated on SCIP not reporting
   `EnumMember`, in both backends. See `doc/zig.md`.
9. **Struct-destructuring `let`.** `let Position { mut offset, .. } = self.pos()`
   is `/* TODO: local */`. Zig has no destructuring binding, so this lowers to
   one `var` per field.
10. **`Expr::Range` outside a `for`.** `span(0..1)` in the test is
    `/* TODO: expr */`; the range type `Range<usize>` renders as `Range(usize)`.
    Shared with `random`'s level 4.
11. **`.checked_add(1).unwrap()`.** Another moniker-dispatched intrinsic, but the
    target is nothing at all: Zig's `+` already traps on overflow in safe modes,
    so `a.checked_add(b).unwrap()` is just `a + b`. Cheap, and it removes four
    call sites from this fixture.
12. **`mut self` by value.** `into_ast(mut self)` emits `self: Self` and then
    mutates it (`self.asts.pop()`); Zig parameters are immutable, so this needs a
    `var` copy in a preamble.

### Tier 3: emission hygiene

13. **`use` items** emit `// TODO: use` three times. Zig has no equivalent for
    `use core::cell::Cell` and nothing is lost by dropping them — but silently
    dropping items contradicts the "print a TODO rather than drop" rule adopted
    on 2026-07-24. The resolution is probably a small allowlist of items that are
    *correctly* nothing in the target, rather than a blanket exemption.
14. **Field/method and variant/method collisions** — tier 3 by size, but
    blocking in practice. `ParserI.parser` vs `fn parser()`, `ParserI.pattern`
    vs `fn pattern()`, `Ast.empty` (variant) vs `fn empty()`. All three are Zig
    compile errors in today's output, so no amount of other work makes this
    fixture build until they are resolved — checking the `Cell` output meant
    hand-renaming `fn parser` first. The
    shadowing renamer handles *locals*; this is the same problem one level up, at
    container scope, and it needs a disambiguation convention. Inlining the
    trivial accessors away would remove these particular instances, but it was
    rejected for `Cell` (see `CELL.md`) and is no better here: it deletes methods
    the source declares and does not generalize past one-line bodies.
15. **Unit structs.** `struct ParserBuilder;` becomes `const ParserBuilder =
    struct {};`, which is right, but the *expression* `ParserBuilder` becomes a
    bare type reference instead of `ParserBuilder{}`.
16. **`#[cfg(test)]` helper functions.** `parser()` and `span()` are test-only
    helpers rather than `#[test]` functions — the first fixture with any. They
    emit as ordinary top-level functions, which is harmless in Zig (one file) but
    is exactly the split the OCaml backend has to get right.

### Already solved

Structs and records, enums with data, inherent methods, `&self` / `&mut self`
receivers, struct literals and field access, nested field paths, method chains,
`assert_eq!`, `if`/`else`, early `return`, and `snake_to_camel` naming all come
out correct — visible in the `Span` / `Position` / `Primitive` portions of the
output, which are already usable Zig.

## OCaml backend

The same fixture through `-- ml` is a useful contrast, because the tiers land
differently: `Box` and `Cell` mostly *vanish* (OCaml is boxed and has `ref`),
while the naming problems get worse.

* **Keyword collisions are unhandled and fatal.** `Span::new` becomes `let new`
  and the field `end` becomes `end :` — both reserved words. `name.rs` has
  `escape_zig` but no OCaml counterpart, and OCaml has no `@"..."` escape
  hatch, so this must be a rename, not an escape.
* **Method collisions are worse than in Zig**, as `doc/ml.md` predicts: three
  distinct `new` functions and two `into_ast` functions all become top-level
  `let`s, silently shadowing each other.
* **Type declaration order.** `type ast` references `literal` and `concat`
  before they are declared. OCaml needs `type a = ... and b = ...` for mutual
  recursion; `print/ml.rs` already emits `and` for grouped declarations, so the
  gap is in the translator — it never groups. This is the OCaml analogue of the
  `Box` problem: Rust's order-independent items versus a language that cares.
* `Box<T>` and `Vec<T>` want `t` and `t array`/`t list`; `Cell<Position>` wants
  a `position ref`, which reuses the existing `ref_vars` machinery.

## Levels

### Level 1: types only

`Position`, `Span`, `Ast`, `Concat`, `Literal`, `LiteralKind`, `Primitive` and
their constructors — no parser. Forces `Box` (gap 1) and the variant/method
collision (gap 14), and nothing else. This is the honest first cut: it is the
smallest slice that still contains the recursive type.

### Level 2: the parser, no `Vec`

`ParserI` with `bump` / `is_eof` / `span_char` / `parse_primitive`, dropping
`parse` and therefore `Concat.asts`. Adds `Cell` erasure and receiver promotion
(gap 4), `Result` + `?` (gap 3), generic struct (gap 5), `char` (gap 6), the
pattern and destructuring gaps (8, 9), and `checked_add` (11). Most of the
fixture's value is here, and it needs no allocator.

### Level 3: `parse` with `Vec`

Adds `loop` (gap 7), `vec![]`, and `ArrayList` — i.e. the allocator decision.
Deliberately last, since it is the one thing that changes every constructor
signature.

## Test

| Path | Role |
|------|------|
| `rust/regex` | fixture, written; `index.scip` built |
| `zig/regex.zig` | golden output — not yet generated |
| `ml/regex` | golden output — not yet generated |

* `#[test] parse_primitive_non_escape` asserts `parser("a").parse_primitive()`
  equals a `Primitive::Literal` with span `0..1`. It exercises `Result`
  equality, a struct literal, a char literal, and the test-only helpers in one
  assertion.
* Regenerate SCIP with `./build_index.sh regex`; golden compare with
  `./test.sh regex` / `./test_ml.sh regex`; behavioral parity with
  `./test_test.sh regex`.
* Both suites currently report `SKIP regex (no expected output)`.

## Next steps

Gaps 4 and 8 are done. The rest, in order of value per unit of work:

1. **Gap 3** (`Result` + `?`) — designed in `RESULT.md`; needs `Item::Type`
   first, and closes `Expr::Try`, a long-standing README "not yet targeted"
   entry. Level 1 covers this fixture's payload-free `Error` and no more.
2. **Level 1** — `Box` as `*T`, deciding the allocator story on the smallest
   possible surface, plus the variant/method collision convention.
3. **Gap 14** (field/method collisions) — promoted from hygiene to blocking:
   verifying the `Cell` output meant hand-renaming `fn parser` before it would
   compile, so `ParserI` cannot produce buildable Zig until this is settled, no
   matter what else lands.

Two cross-cutting observations worth recording independent of this fixture. The
naming collisions (gap 14, and its worse OCaml form) are not regex-specific —
any crate with a getter named after its field will hit them, so they are a
backend-wide debt this fixture merely exposes first. And the `alloc` constraint
in the README's scope section is now doing harm rather than good: it was chosen
to keep SCIP indexing tractable, but it excludes every crate that builds a data
structure, which is most of them. This fixture is the argument for dropping it.
