# Common Lisp: what `rust/regex` needs

An inventory of the gap between the Common Lisp backend and `rust/regex`, the
largest unfixtured example. `doc/lisp.md` states the rules the backend follows;
this is the list of what `regex` asks for that they do not cover, ordered so
that the parts worth doing first come first.

`regex` is a cut-down `regex-syntax` parser: `Span`/`Position` arithmetic, an
`Ast` enum of boxed variants, a `Parser` holding a `Cell<Position>`, and a
`ParserI<'s>` borrowing both the parser and the pattern. Its one test parses
`"a"` and compares the result against a hand-built `Primitive::Literal`.

It is worth having for a reason none of the twelve current fixtures supplies:
it is the only example that is a **program** rather than a function or two.
Every other Lisp fixture exercises one feature against a small assertion.
`regex` exercises `Result`, `Vec`, `Box`, `&str`, `char`, `Range`, interior
mutability, and struct equality *at once*, in code that was written to be read
rather than to be translated.

## Current state

29 `defun`s, 16 inline `todo` markers, 3 top-level `;; TODO` comments.

| marker | count |
|---|---|
| `(todo "method call")` | 9 |
| `(todo "field")` | 4 |
| `(todo "match")` | 1 |
| `(todo "expr")` | 1 |
| `(todo "local")` | 1 |

The count started at 21, rose to 22, and is now 16 -- which is why it is a
poor measure of progress here. Much of the work so far has been converting
silent breakage into visible markers, and unblocking constructs that were
hiding others behind them. Marker count tracks how much of `regex` the
translator can *see*, not how close it is to translating it.

`(todo "method call")` is now most of what is left, and nearly all of it is
[strings and chars](#strings-and-chars).

A fair amount already works and is worth stating, because it narrows what is
left. `Span`, `Position`, `Literal`, `Concat`, `Parser`, and `ParserI` all
become correct `defstruct`s; `Ast` and `Primitive` become variant structs under
a `deftype` union; `LiteralKind` becomes `(member :verbatim)`; the unit struct
`ParserBuilder` becomes a fieldless `defstruct` and `ParserBuilder` in value
position becomes `(make-parser-builder)`. Associated functions
(`Span::new`, `Position::new`) and methods (`ParserI::pos`, `ParserI::span`)
translate and compose:

```lisp
(defun parser-i-span (self)
  (span-splat (parser-i-pos self)))
```

The `Cell` is handled end to end, and it is the one part of `regex` that is
finished. All four positions erase, so the cell leaves no trace in the output:

| Rust | Common Lisp | where |
|---|---|---|
| `pos: Cell<Position>` | `(pos nil :type position)` | `translate_type` |
| `Cell::new(p)` | `p` | `translate_call` |
| `self.parser().pos.get()` | `(parser-pos (parser-i-parser self))` | `translate_method_call` |
| `...pos.set(p)` | `(setf (parser-pos ...) p)` | `translate_method_call` |
| `...pos.get().offset` | `(position-offset (parser-pos ...))` | `expr_ty` |

`design/cell.md`'s question -- which Zig pointers lose their `const` -- does
not arise, because a Common Lisp structure is a reference already.

## Defects that produce no marker

These come first because they are worse than the gaps. The backend's stated
discipline is that an unhandled construct is *loud* -- `doc/lisp.md` opens its
"Not implemented yet" section with it, and `doc/lisp-random.md` sharpens it
into "I know what this is and declined" being a different report from "I do not
know what this is." `regex` is the first example where that discipline fails,
in six distinct ways, and it fails silently in all six.

### The call fallthrough invented a function (half done)

This was the most serious one. `translate_call` ended by taking the *last path
segment* of any unrecognized callee and emitting it as a plain function call:

```rust
let name = snake_to_kebab(&ep.path.segments.last().unwrap().ident.to_string());
call(&name, ...)
```

So `Box::new(span)` became `(new span)`, `Cell::new(...)` became `(new ...)`,
and `Ok(concat.into_ast())` became `(Ok (concat-into-ast concat))` -- calls to
functions the file never defines, with no marker anywhere:

```lisp
(defun ast-empty (span)
  (make-ast-empty :v0 (new span)))
```

The failure mode is an undefined-function error at load or call time, blaming a
name that appears nowhere in the Rust source -- strictly worse than a marker,
and not a `regex`-specific accident: it fired for every foreign associated
function in any crate, and had not bitten only because the twelve fixtures call
nothing foreign.

**A *qualified* path that is not ours now leaves `(todo "call")`.** The branch
sits directly after the one that recognizes an associated function on a crate
type (`BitSet::with_capacity`), so the rule reads: qualified and ours, emit the
prefixed call; qualified and not ours, marker.

The four sites `regex` had were all `Box::new` and `Cell::new`, and both are
[erased](#box-done) now, so the crate has no qualified-foreign call left to
mark. That is the intended shape of the thing: the marker is the floor, and a
recognition is added above it whenever one is worth having. The marker still
earns its place -- it is what the next crate's foreign call will hit.

**The unqualified half is still open**, and `Ok` is what it costs. A
single-segment path stays on the fallthrough, because that is also how a
crate-defined *free* function is called -- `gcd`, `min`, `position` -- and
those are the common case. `Ok(...)` and `Err(...)` are single-segment, so they
still emit `(Ok ...)`. Telling them apart from a crate free function wants
SCIP's symbol rather than the syntax, and is better done as part of
[`Result` and `?`](#result-and-), which has to give `Ok` a translation anyway.
Recorded so the remaining hole is not mistaken for a closed one.

Whether some qualified paths should get a pass-through rather than a marker --
`Box::new(x)` really is `x` once boxing is erased, the same shape as `Cell` --
is a separate decision, taken below.

### A marker in binding position binds a variable named `todo`

`src/translate/lisp/stmt.rs:122` pushes `todo("local")` into a `let`'s
*binding list*, which produces this:

```lisp
(let ((todo "local"))
  ...
  (setf line ...)
  (incf offset ...))
```

`(todo "local")` in that position is not a marker at all -- it is a binding of
the variable `TODO` to the string `"local"`. The body then references `offset`,
`line`, and `column`, which are unbound. The marker is invisible to a reader
skimming for `todo` calls and invisible to the compiler as a marker.

The construct behind it is the struct destructuring
`let Position { mut offset, mut line, mut column } = self.pos();`. `stmt.rs`
handles `Pat::Tuple` (as `multiple-value-bind`) and nothing else.

### A method may clobber a slot accessor

`ParserI` has a field `parser` and a method `parser()`. Both translate to the
name `parser-i-parser`, so the `defun` redefines the `defstruct` accessor:

```lisp
(defstruct parser-i
  (parser nil :type parser)
  (pattern nil :type vector))

(defun parser-i-parser (self)
  (parser-i-parser self))
```

The same happens for `pattern`. This is the Common Lisp form of the union
field/method collision `README.md` records as a Zig bug, and it is the reason
Rust's per-type namespaces keep costing this backend something.

What actually happens is worth being precise about, because it is not what it
looks like. SBCL prints `WARNING: redefinition of FOO-BAR clobbers structure
accessor` and proceeds; ECL accepts it silently. The body's inner call is
inlined against the *accessor* before the redefinition takes effect, so
`parser-i-parser` ends up behaving exactly like the accessor it replaced -- the
translation is accidentally correct, because the method body happens to be
precisely that field read. Verified on SBCL 2.6.7: the redefined function
returns the slot value rather than recursing, both when called directly and
from a caller compiled afterwards.

The luck runs out as soon as a method named after a slot does anything else,
and it runs out quietly: every emitted slot read of `pattern` would silently
route through the method instead. A renaming strategy is needed, the same one
the Zig bug wants.

### `equal` on structs makes the test answer wrongly (blocked on `Result`)

`regex`'s only test is an `assert_eq!` between two `Primitive::Literal` values.
Confirmed on SBCL: `equal` of two separately constructed structures with equal
slots is `NIL`, `equalp` is `T`. The assertion as emitted cannot pass, so
`equalp` is not optional for this fixture.

The third equality is **implemented** now -- a `Sort::Aggregate` covering crate
structs, data-carrying enums, arrays, slices, and `Vec` selects `equalp`. It
does not yet reach this line, and the reason is instructive: the comparison is
between two `Result<Primitive>` values, and `Result` sorts as `Other` because
it has no encoding. So this assertion still emits `equal`:

```lisp
(assert (equal (parser-i-parse-primitive (parser-i-new p "a")) (Ok ...)))
```

It should fall out for free once [`Result`](#result-and-) lands, whichever
encoding wins -- a variant-struct pair sorts as an aggregate directly, and any
other encoding has to answer the question anyway. Nothing further is owed to
`equalp` here.

### Shadowing `Error` disarms `panic!`

The crate defines `pub struct Error`, so the package emits `(:shadow #:error)`.
That makes `ERROR` a fresh symbol in the `regex` package and `CL:ERROR`
unreachable by that name -- confirmed on SBCL, where `(fboundp 'error)` is
`NIL` after the shadow. But `(error "msg")` is exactly what `doc/lisp.md`'s
Macros rule emits for `panic!`.

`regex` contains no `panic!`, so nothing breaks today. The general rule is
missing though: the shadow list is computed from the names the crate defines,
with no account of the names the *translator* emits. `error` is the one that
matters; `assert` would be another.

### A newline character literal was emitted as a line break (done)

`'\n'` became `#\` followed by a literal newline in the output:

```lisp
    (if (equal (parser-i-char self) #\
)
```

That reads correctly -- `#\<newline>` is `#\Newline`, verified -- but it broke
the line the printer was laying out, and it was one trailing space away from
being a different character. `'\n'` is now special-cased to `#\Newline` and the
`if` fits on one line again.

Only that one character is handled. The same question applies to every other
character Common Lisp *names* rather than shows -- `Space`, `Tab`, `Return`,
`Nul`, `Page`, `Backspace`, `Rubout`, all eight of which were checked to read
back to the right code on SBCL 2.6.7 and ECL 26.5.5 -- and beyond them to
control characters CL names not at all, whose only portable spelling is
`(code-char n)`, an expression rather than a literal. `regex` needs none of
them, and space is the one most likely to arrive next: `#\ ` reads correctly
only because a delimiter follows it, which is the same fragility as the
newline.

### Duplicate `:export` entries

`ast-empty`, `ast-literal`, and `ast-concat` each appear twice in the export
list, once as the variant struct and once as the associated constructor
function `Ast::empty` / `Ast::literal` / `Ast::concat`. SBCL tolerates the
duplication, so this is cosmetic rather than broken -- but it is a visible
symptom of the two namespaces colliding again, this time harmlessly, since a
`defstruct` type and a `defun` of the same name coexist.

## What is missing, by feature

### loop (done)

`fn parse` uses a bare `loop { if ... { break; } ... }`, which was the whole of
one `(todo "expr")`. It is now `(loop do ...)` -- the `while` shape with the
test dropped, establishing the same implicit `nil` block the existing `break`
translation already targeted, so nothing had to be added for `break` or
`continue`.

### match

Three separate gaps, of which two are now done:

* **Literal patterns (done).** `match self.asts.len() { 0 => ..., 1 => ..., _ => ... }`
  is now a `case`.
* **A wildcard arm (done).** Plain `case`/`typecase` with a final `t` clause.
  This came with the literals rather than after them: rustc requires a
  catch-all over integers and characters, so a literal match nearly always has
  a `_` and the two are one feature in practice.
* **A bare binding pattern.** `match self.char() { c => ... }` binds the
  scrutinee and always matches. It is a `let` in disguise, and is probably
  best recognized as one rather than routed through a case form. This is the
  one remaining `(todo "match")`.

These are matches over an *integer* and a *character*, not over an enum, which
was the interesting part: both existing encodings dispatch on a crate enum, and
`case` over integers is a third shape that happens to share the keyword side's
machinery, since both dispatch by value. `value_pat` is what they share.

### Box (done)

Pure erasure -- a Common Lisp value is already a reference, so `Box<T>` is `T`
and `Box::new(x)` is `x`, exactly as `Cell` erases, and with even less to
argue: a box has no representation of its own to collapse, so unlike the
`Option`-as-`nil` erasure this one carries no soundness condition.

### Vec (done)

A `Vec` is an **adjustable vector with a fill pointer**, which is what makes
`push` and `pop` exist at all -- a `#(...)` literal is fixed-size and has
neither -- and it leaves `length` reading the fill pointer, so the count is
Rust's rather than the allocation's. The declared slot type stays the wide
`vector`, which this is one of, so nothing changes there:

```lisp
(make-concat :span (parser-i-span self)
             :asts (make-array 0 :adjustable t :fill-pointer t))
```

`vec![a, b]` fills the same shape through `:initial-contents`, though `regex`
only ever writes `vec![]`.

Two things worth recording:

* **`push` reverses its operands.** `vector-push-extend` takes the element
  first and the vector second, which is the opposite of the receiver-first
  shape every other translated method has. It is the only method so far where
  the receiver is not the first argument.
* **`pop` diverges on an empty vector.** Rust answers `None`; CL's
  `vector-pop` signals, on both implementations.

### Result and `?`

The largest item, and the one with no design at all -- `doc/lisp.md` says so
outright. `regex` needs the whole of it: `type Result<T> = core::result::Result<T, Error>`
(a type alias, itself a `;; TODO: type alias`), `Ok(...)` in return position,
and `self.parse_primitive()?` in expression position. The `try_expression`
desugar pass exists for the other backends and is not run here.

The `nil` erasure that serves `Option` does not extend: a `Result` carries a
payload on both sides. The two obvious candidates are Common Lisp's own
two-value convention (`(values result error)`, which is what CL idiom would
reach for and which composes badly with `?`) and a variant-struct pair reusing
the data-carrying enum encoding already implemented. The second is closer to
what the backend already does, and `?` then lowers to an `etypecase` with an
early `return-from`, which is available for free -- `doc/lisp.md`'s "What the
target gives" notes that Common Lisp needs neither of the exception shims
`doc/ml.md` builds. Recorded as an option, not a decision.

### Strings and chars

`design/string.md`'s question, which `doc/lisp.md` already flags as a design
problem rather than a gap: CL strings are character vectors, not byte vectors.
`regex` wants `str::len` (which the `core::slice::len` moniker does not match,
so it markers), `chars()`, `next()`, and `len_utf8()`, plus `self.pattern()[i..]`
slicing. It is the one place where CL's representation is *closer* to Rust than
Zig's is -- a CL string is a sequence of characters, so `chars()` is nearly
`identity` and `len_utf8` is about the encoding rather than the string -- but
`str::len` counting bytes while `length` counts characters is a real divergence
that has to be decided rather than papered over. `regex`'s positions are byte
offsets.

### Range

`#[cfg(test)] fn span(range: Range<usize>)` reads `range.start` and
`range.end`, and the test calls `span(0..1)`. Four of the seven `(todo "field")`
markers and one `(todo "expr")` are this. A `Range` in *value* position is new;
ranges are handled today only as `for` clauses, where `loop`'s `from`/`below`
consume them without ever building one. A cons or a two-slot struct would do.

### `expr_ty` sees through `Cell::get` (done)

Three `(todo "field")` markers -- `offset()`, `line()`, `column()`, all of the
form `self.parser().pos.get().offset` -- came from a single narrow hole.
`translate_method_call` erased `Cell::get` to its receiver, but `expr_ty` had
no arm for it, so the field access could not find the struct to name an
accessor from.

The fix took the shape of the `wrapping_op` arm directly above it in the same
`match`, which takes a method call's type from its receiver: `expr_ty` of
`x.get()` where `x: Cell<T>` is `T`, peeled by a local `cell_inner` that
identifies the cell by moniker rather than by name. The three functions now
translate:

```lisp
(defun parser-i-offset (self)
  (position-offset (parser-pos (parser-i-parser self))))
```

The remaining four `(todo "field")` markers are all `Range`, below.

### Items

Two `;; TODO: use` (one of them `#[cfg(test)]`) and one `;; TODO: type alias`.
Also unexamined: `#[cfg(test)]` on an item generally, which here gates both a
`use` and the helper `fn span`, and which the backend currently ignores -- the
helper is emitted unconditionally, which happens to be what a self-contained
test file wants.

## Suggested order

The list divides cleanly into work that is cheap and work that is a design.

**Cheap, independent, and clears markers now:** all done -- the `Cell::get` arm
in `expr_ty`, bare `loop`, the `#\Newline` spelling, and `Box` erasure.

**Cheap and fixes a discipline violation rather than a marker:** ~~replacing
the call fallthrough with a marker~~ done for qualified paths, `Ok`/`Err` left
to the `Result` design; and moving the `let` marker out of binding position.
Neither adds a feature; both stop the backend from lying. Doing these *first*
is worth arguing for, because every later measurement of "what does `regex`
still need" is wrong until the silent failures become visible ones.

**Real features, roughly by size:** ~~`equalp`~~, ~~`Vec`~~, and two of the
three ~~`match` shapes~~ done; left are the bare binding pattern, `Range`, and
the accessor/method renaming strategy.

**Designs, not tasks:** `Result` and `?`; strings and chars. Either could be
taken independently of the other, and `regex` needs both before it can have a
fixture.

A fixture is all-or-nothing -- `test_lisp.sh` diffs a whole file -- so `regex`
stays unfixtured until the last of these lands. That argues for keeping it as
the marker-count benchmark it is today, and for adding a smaller example for
each feature as it arrives, the way `direction` was added for payload-free
enums.

## Verification

Claims about Common Lisp behavior in this document were checked on SBCL 2.6.7,
and on ECL 26.5.5 where the two might differ:

| claim | checked |
|---|---|
| `equal` NIL / `equalp` T on two equal-slotted structs | SBCL |
| accessor redefinition warns and returns the slot value | SBCL (warns), ECL (silent) |
| `#\` + literal newline reads as `#\Newline` | SBCL |
| `(fboundp 'error)` is NIL after `(:shadow #:error)` | SBCL |
| duplicate `:export` entry accepted | SBCL |

The translator output quoted throughout is from
`cargo run -- lisp rust/regex <dir>` at the current tree. Nothing in this
document has been run as a whole file: `regex.lisp` does not load, which is the
point of it.
