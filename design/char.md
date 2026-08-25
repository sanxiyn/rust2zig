# Characters (OCaml)

How Rust's `char` is represented in the OCaml backend. **Implemented** across
`translate_lit`, `print/ml.rs`, and `ml/string.rs`, with `rust/string` /
`ml/string` as the golden pair; character *patterns* are the one part still
open, and they are a design question rather than a missing implementation.
Driven by `rust/regex`, the one fixture using `char`, `.chars()`, and
`len_utf8`, and by `rust/string`, which is that one pared down. The OCaml
snippets here are verified against OCaml 5.4.1.

This is the other half of `design/string.md`, which chose `string` for `&str`
and `&[u8]` and explicitly deferred `char` to its own design. Read that one
first: the representation it picked is what makes the rule below cost nothing at
the boundary.

## The mismatch

`char` is the one primitive where Rust and OCaml share a name and mean different
things.

| | Rust `char` | OCaml `char` | OCaml `Uchar.t` |
|---|---|---|---|
| domain | Unicode scalar value | a byte | Unicode scalar value |
| range | `0..=0x10FFFF` less surrogates | `0..=255` | `0..=0x10FFFF` less surrogates |
| width | 4 bytes | 1 byte | immediate `int` |
| literal syntax | `'a'` | `'a'` | **none** |
| pattern matching | yes | yes | **no** |

OCaml's `char` is not a narrower `char`; it is a different type that happens to
agree on ASCII. `Uchar.t` is Rust's `char` exactly — same domain, same
exclusions.

## The rule

> Rust's `char` is OCaml's `Uchar.t`.

Not an approximation, and the reason is the first row of the facts table below:
`Uchar.max` is `0x10FFFF` and `Uchar.is_valid 0xD800` is false. The set of
values is the same set, so no Rust `char` is unrepresentable and no `Uchar.t` is
un-Rustable.

The cost is entirely in *syntax* — literals and patterns — and is discussed
below. It is real, but it is noise rather than wrongness, which is the trade
this project takes everywhere else (`design/operator.md` leaves integer `!` a
compile error rather than a wrong answer; `design/bound.md`'s permissiveness
invariant).

## OCaml facts

Verified by running each, on OCaml 5.4.1.

| # | Expression | Result |
|---|---|---|
| 1 | `Uchar.to_int Uchar.max` | `0x10ffff` |
| 2 | `Uchar.is_valid 0xD800` | `false` — surrogates excluded, as in Rust |
| 3 | `Obj.is_int (Obj.repr (Uchar.of_char 'a'))` | `true` — immediate, not boxed |
| 4 | `Uchar.of_char 'a' = Uchar.of_int 0x61` | `true` — polymorphic `=` works |
| 5 | `Uchar.of_char 'a' < Uchar.of_char 'b'` | `true` — polymorphic `<` works |
| 6 | `Uchar.utf_8_byte_length (Uchar.of_int 0x1F600)` | `4` |
| 7 | `String.get_utf_8_uchar "a\xf0\x9f\x98\x80b" 1` | decodes U+1F600, length `4` |
| 8 | `match (u : Uchar.t) with 'a' -> …` | **type error**: pattern matches `char` |
| 9 | `Uchar.is_char (Uchar.of_int 0x1F600)` | `false` |
| 10 | `Uchar.to_char (Uchar.of_int 0x1F600)` | raises `Invalid_argument` |
| 11 | `String.get_utf_8_uchar "\x80" 0` | invalid: yields `Uchar.rep` (U+FFFD), length `1` |

Fact 3 is the one that keeps this cheap. `Uchar.t` is an `int` underneath, so it
costs nothing at runtime and, by facts 4 and 5, works with structural equality
and comparison — see [Interaction with equality](#interaction-with-equality).

Fact 11 is unreachable from translated code and is listed so that stays on the
record: a Rust `&str` is guaranteed UTF-8, so a decode at a character boundary
cannot fail. The `utf_decode_is_valid` flag can therefore be discarded rather
than checked, which is the same move `design/string.md` makes when it declines
to enforce the UTF-8 invariant — Rust already did.

## What `rust/regex` needs

Three operations, and OCaml's standard library has an exact counterpart for each.

| Rust | OCaml | |
|---|---|---|
| `c.len_utf8()` | `Uchar.utf_8_byte_length c` | implemented, by moniker |
| `s[i..].chars().next().unwrap()` | `Uchar.utf_decode_uchar (String.get_utf_8_uchar s i)` | implemented, as one idiom |
| `self.char() == '\n'` | `c = Uchar.of_char '\n'` | implemented, and needs no equality case |

**The second row is the interesting one, because it is not an iterator.**
`rust/regex` never walks a character sequence; `char_at` takes the *first*
character at a byte offset, and `bump` advances the offset by that character's
UTF-8 length. `String.get_utf_8_uchar` is indexed by byte offset and answers
exactly that, so this needs no iterator protocol and does not wait on
`design/flow.md`'s iterator-chain hole. The `.chars().next()` shape is a Rust
idiom for "decode one character here", and it is recognized as one rather than
lowered through `.chars()` — `translate_char_at` in `ml/string.rs` matches the
whole four-call shape, since the intermediate steps have nothing to map onto
individually.

Each step is checked by moniker, and the one that makes this safe is `next`:
rust-analyzer spells it ``str/iter/impl#[`Chars<'a>`][Iterator]next().``, naming
`Chars` rather than `Iterator`, so the match cannot fire on an unrelated
`Option`-yielding `next`. The range has to be open-ended; `s[i..j]` decodes the
same character but is a different thing to have written, and falls through.

## Worked example

`ml/string`'s `char_at` and the `bump` offset advance, emitted rather than
written by hand, and byte-for-byte what the design predicted:

```ocaml
let char_at pattern i =
    Uchar.utf_decode_uchar (String.get_utf_8_uchar pattern i)

let bump pattern offset =
    offset + Uchar.utf_8_byte_length (char_at pattern offset)

let count_lines pattern =
    let offset = ref 0 in
    let line = ref 1 in
    while not (is_eof pattern !offset) do
        if char_at pattern !offset = Uchar.of_char '\n' then
            line := !line + 1;
        offset := bump pattern !offset
    done;
    !line
```

Note what is *not* in it: no decoder, no conversion at the string boundary, no
helper module, and no type annotations — the signatures are inferred, and infer
correctly, because `String.get_utf_8_uchar` fixes `pattern` as a `string` and
`Uchar.utf_decode_uchar` fixes the result as a `Uchar.t`.
`design/string.md`'s `string` feeds `String.get_utf_8_uchar` directly.

`dune runtest` agrees with `cargo test` on all six assertions over `"a√\nz"`:
six bytes, four characters, two lines, and `len_utf8` of 1 and 3. That is Rust
and OCaml reaching the same answers through entirely different string
representations, which is the check that matters here.

## The cost: no literals, no patterns

`Uchar.t` is abstract. This is the whole of the objection to it, and it has two
halves of very different weight.

### Literals: noise, and mechanical

**Implemented.** There is no `Uchar.t` literal, so every Rust `char` literal
needs a constructor:

| Rust | OCaml | When |
|---|---|---|
| `'\n'`, `'a'`, `'('` | `Uchar.of_char '\n'` | the literal is Latin-1 (fact 9) |
| `'√'`, `'😀'` | `Uchar.of_int 0x221A` | anything above U+00FF |

Both are free at runtime — fact 3 — so this is textual noise only.
`Uchar.of_char` is total on OCaml's `char`, and the ASCII case is the one every
current fixture would hit. A literal appearing many times in one function is the
usual candidate for hoisting to a `let`, but that is a printer concern and not
this design's.

The split is at `U+00FF` rather than at ASCII because OCaml's `char` *is*
Latin-1, and Latin-1 is Unicode's first 256 code points, so `Uchar.of_char` is
exact over the whole range: `Uchar.of_char '\xe9'` is U+00E9, verified. That
made `Constant::Char` — which the ml AST carried unused until now — usable
as-is, with one printer change. Its escaping had never run: it printed `'{c}'`
raw, which spells `'\n'` as a literal newline inside quotes. `char_escape` now
emits OCaml's named escapes for the six that have them, passes `0x20..=0x7E`
through, and sends everything else out as `\xNN` — necessarily so for the
Latin-1 upper half, which a UTF-8 source file cannot write as one byte.

### Patterns: the question that decides how far this scales

Fact 8: a `Uchar.t` cannot be matched against a character literal at all.

`match c { '(' => …, ')' => …, _ => … }` is the natural shape of a regex parser,
and it has no direct translation. The workaround is a guard chain, verified to
work:

```ocaml
match u with
| u when u = Uchar.of_char 'a' -> 1
| u when u = Uchar.of_char 'b' -> 2
| _ -> 0
```

The guard is `=`, not `Uchar.equal`, by fact 4: `Uchar.t` is an immediate `int`,
so polymorphic equality answers correctly on it. That keeps the guard the same
spelling as the `== '\n'` row of the table above, and means `char` needs no
entry in `design/equality.md`'s dispatch — one comparison operator covers it,
here as everywhere else in the OCaml backend.

This is correct and it is ugly, and it gives up the exhaustiveness checking that
made `match` worth using. It is also, notably, the same shape the Common Lisp
backend would need and does not (`doc/lisp.md` matches characters directly),
which makes this an OCaml-specific cost rather than a `char` cost.

**How much this bites depends on where `rust/regex` is going, not where it is.**
The current extraction does not match on character literals at all: its two
`match self.char()` blocks are a `_ =>` and a bare `c =>` binding, and the only
literal use in the file is `== '\n'`, which is the third row of the table above
and needs no pattern. So `Uchar.t` costs this fixture nothing today. The real
`regex-syntax` parser dispatches on `'('`, `'*'`, `'['`, and friends, and that
is where guard chains would become the dominant shape of the output.

Deciding between guard chains and something better should therefore wait for a
fixture that actually matches on characters. Two options exist if it becomes
worth it, neither designed here: lower an all-Latin-1 match by converting the
scrutinee (`Uchar.is_char` guard plus `Uchar.unsafe_to_char`, recovering native
`char` patterns inside), or match on `Uchar.to_int` against integer literals,
which restores exhaustiveness and destroys readability.

## Interaction with `design/string.md`

None needed, which is the point. That document chose `string` for `&str`, and
`String.get_utf_8_uchar : string -> int -> Uchar.utf_decode` consumes exactly
that representation. There is no conversion at the boundary and no third type.

The two designs stay disjoint at indexing, too. `s.[i]` (`StringGet`) yields an
OCaml `char` and is how `&[u8]` indexing lowers, because Rust's `bytes[i]` is a
`u8` and not a `char`. Nothing in this design changes that path; `Uchar.t`
appears only where the Rust type is `char`.

## Interaction with equality

Also none, and this is the sharpest contrast with the other backends.

By facts 4 and 5, `Uchar.t` is an immediate `int`, so OCaml's polymorphic `=`,
`<`, and `compare` answer correctly on it. `design/equality.md`'s OCaml column —
"one polymorphic `=` covers every case Rust can present" — stays true with
`char` added to the table, and `==` on characters needs no special case at all.

Compare what `char` costs the other two. In Common Lisp it is one of the two
rows that rule out `equalp` (`doc/lisp.md`: `(equalp #\a #\A)` is true where Rust
is false), which is half the reason that backend needs three predicates. In Zig
it is not comparable because it has no type yet (`design/string.md`,
`design/equality.md`'s level 1). OCaml gets it for free.

## Zig is a separate question

`design/string.md` observes that neither backend has solved `char` and infers:
"that both backends are stuck at the same place suggests it is a problem about
Rust's `char`, not about either target."

**That inference does not hold.** OCaml has an exact answer with standard-library
support for both operations `rust/regex` needs, and the only cost is syntactic.
The shared stuckness was coincidence rather than a shared cause, so Zig's answer
has to be argued on Zig's own terms rather than falling out of a common finding.

Zig's likely shape — `u21`, with `std.unicode.utf8Decode` and
`std.unicode.utf8CodepointSequenceLength` — is *not* claimed here; nothing in
this document has been checked against Zig, and `zig/regex.zig` does not exist.
It is named only to record that the question is open and separate.

## Test

| Path | Role |
|------|------|
| `rust/string`, `ml/string` | golden pair |
| `rust/regex` | Where the shapes come from: `Literal { c: char }`, `char_at` via `.chars().next()`, `len_utf8`, and `== '\n'`. Unfixtured in every backend |

There cannot be a cheap fixture that is only about `char`: the operations that
matter are `.chars().next()` and `len_utf8`, both of which need a string to
index into, so the smallest honest one is a decoder loop over a mixed-width
string. `rust/string` is that loop — `rust/regex`'s `char_at`, `bump`, and
`is_eof` flattened to free functions over `(pattern, offset)`, with the `Cell`,
the `Position`, and the AST dropped — over `"a√\nz"`: four characters, six
bytes, two lines, and one three-byte character to make a byte offset and a
character offset disagree.

It emits marker-free and `test_test.sh string` passes on both Rust and OCaml.
There is no Zig or Common Lisp golden: neither backend has a `char` answer, and
`design/string.md`'s note that the Zig one has to be argued separately still
stands.

Running the ml backend on it produced four distinct holes, which was the work
this design implied and is now all done:

| Emitted | Wanted | |
|---|---|---|
| `Char.code pattern.[(* TODO: expr *)]` | `String.get_utf_8_uchar pattern i` | **done** |
| `Option.get (next (chars …))` | dropped — the decode is the whole idiom | **done** |
| `len_utf8 c` | `Uchar.utf_8_byte_length c` | **done** |
| `(* TODO: lit *)` for `'\n'` | `Uchar.of_char '\n'` | **done** |

The fixture arrived with two more holes that were not about `char`, both fixed.
`pattern.len()` emitted `len pattern`, because `str::len` is a different moniker
from the slice `len` that `design/string.md` gated on and only the latter was in
the table.

The fixture also exposed, and has now retired, a span bug unrelated to either
design. `proc_macro2` counts columns in characters and SCIP counts them in UTF-8
bytes; every previous fixture was ASCII, so the two agreed by accident. Here a
name occurring after `'√'` on the same line resolved at the wrong column and
printed unlowered (`char_at PATTERN 1`). `src/scip.rs` converts occurrence
columns at load; see the README's desugaring section.

`doc/lisp-regex.md` inventories what `rust/regex` needs before it can have a
golden at all; `char` is one item on that list and not the largest.

## Not implemented yet

1. Character patterns, which is the open design question above rather than a
   missing implementation. `rust/string` deliberately does not need them.
2. Any `char` at all in the Zig and Common Lisp backends. This is an OCaml
   design and only OCaml implements it.
3. `char`-classifying methods (`is_alphabetic`, `is_numeric`, `to_ascii_*`).
   OCaml's stdlib has no Unicode character database, so these have no
   counterpart and would need either an ASCII-only lowering or a dependency —
   the first honest, the second out of scope for a closed stdlib reference set
   (`design/string.md`'s "Alternatives considered" rejects a shipped runtime
   library for the same reason).
4. `char as u32` / `u32 as char`. `Uchar.to_int` and `Uchar.of_int` are the
   counterparts, with `of_int` raising on an invalid scalar value where Rust's
   `from_u32` returns `Option`.

## Not planned

* **OCaml `char` as the representation.** It buys literals and patterns by being
  silently wrong on any non-ASCII input: `len_utf8` would be 1 always, and
  `char_at` would return a fragment of a multi-byte character rather than the
  character. This is the failure mode the project refuses everywhere else, and
  it fails on exactly the inputs a regex engine exists to handle.
* **`int` as the representation.** Patterns work and nothing else does; a
  character literal becomes `0x28`. Rejected for the reason `design/string.md`
  rejected `int array` for `&str`: the backend exists to produce idiomatic
  OCaml, and no OCaml programmer spells a character as an integer.
* **A `Char` compatibility module shipped with the output.** Would give literals
  a shorter spelling and patterns a home. Rejected on the same ground as that
  document's `Slice` abstraction and `design/integer.md`'s `Uint32`: the
  backend's stdlib references are a closed set, and a runtime library shipped
  with generated code is a different product.
* **Grapheme clusters, normalization, or collation.** Rust's `char` is a scalar
  value and nothing more; matching its semantics means matching that, not
  improving on it.
