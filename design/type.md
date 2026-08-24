# Types

Where the translator gets its type information and what it does with it. This
is not a feature; it is the substrate the other design documents stand on, and
it is the reason SCIP is a dependency at all.

Narrower type questions have their own documents: `design/type_alias.md` (the
alias-expanding desugar pass), `design/integer.md` (OCaml integer widths),
`design/string.md` (`&str` and `&[u8]`), `design/option.md`, and
`design/result.md`.

## Types do not come from the Rust source

`syn` gives the types the programmer *wrote*, and in Rust that is not the same
as the types the program *has*:

* they are optional -- `let x = f();` has none;
* they are incomplete -- `let xs: [T; _]` is legal Rust with a wildcard in it;
* they are elided -- `&self` names no type, and lifetimes are inferred.

So the translator does not read types off the AST. It asks rust-analyzer,
through the SCIP index:

| Question | Method | Answers for |
|---|---|---|
| what type is this binding? | `Scip::type_at(range)` | `Kind::Variable`, `Kind::Parameter`, `Kind::SelfParameter` -- parsed out of the suffix after `": "` in the symbol's signature documentation |
| what does this call return? | `Scip::return_type_at(range)` | `Kind::Function`, `Kind::Method`, `Kind::StaticMethod` |
| what types does this operator dispatch on? | `Scip::binary_type_at(range)` | the impl recorded at a binary operator, e.g. `AddAssign<&i32>` |
| what kind of thing is this name? | `Scip::kind_at(range)` | everything with a `SymbolInformation` |

The consequence is stated in the README as a rule and is worth repeating here as
a design decision: **a `let` binding always emits the SCIP type, ignoring any
annotation in the source.** The source annotation is not a second opinion to be
merged; it is strictly less informative.

### The oracle does not answer for expressions

SCIP indexes *symbols*, so it can type a name and a call, but not
`(a + b) >> 2`. That gap is filled by `expr_type` in `src/translate/ty.rs`, a
partial inference of about sixty lines, shared by all three backends:

* a path or method call -- ask the oracle;
* a cast -- the cast's own type;
* a paren -- the inner type;
* an index -- the element type of the indexed array or slice;
* an arithmetic or bitwise binary -- the left operand's type, falling back to
  the right's; a shift takes its left operand's type, since Rust's shift
  amount need not share it.

That is the whole type system the translator has. There is no checker and no
unification: **a type oracle plus sixty lines of inference**, and every
type-directed decision in the project is built on exactly that. When neither
answers, the decision degrades -- and how it degrades is the interesting part,
below.

## What each backend needs types for

| | emitted where | needed internally for |
|---|---|---|
| Zig | everywhere: params, returns, `let` annotations, struct fields | `@truncate` vs `@as`, `@intCast` on shift amounts, `@rem` on signed operands, receiver pointer constness, wrapping widths |
| Common Lisp | in `declaim ftype`, `declare`, and `defstruct` `:type` -- none of it required by the language | the equality predicate (`design/equality.md`), the `ldb` width for wrapping, `not` vs `lognot` |
| OCaml | almost nowhere -- `let gcd a b = …` carries no annotation at all | integer escalation (`design/integer.md`), `string` vs `bytes`, per-field mutability |

The spread is the point. **The amount of type information the three backends
emit ranges from everything to nothing, while the amount they need internally
is nearly the same.** OCaml's inference means the backend can stay silent;
Common Lisp's declarations are optional and were adopted anyway, as the
overflow check that the language does not otherwise have; only Zig is obliged.

## The mapping

| Rust | Zig | Common Lisp | OCaml |
|---|---|---|---|
| `i8`..`i128`, `u8`..`u128` | same name | `(signed-byte N)`, `(unsigned-byte N)` | `int`, or `int32`/`int64` when escalated |
| `isize`, `usize` | same name | `fixnum` | `int` |
| `bool` | `bool` | `boolean` | `bool` |
| `char` | **marker** | `character` | -- |
| `&str`, `String` | `[]u8` | `vector` | `string` |
| `&[T]` | `[]T` | `vector` | `bytes` for `&[u8]` |
| `[T; N]` | `[N]T` | `vector` | -- |
| `&T`, `&mut T` | `*const T`, `*T` | erased | erased |
| `()` | `void` | `null` | `unit` |
| `(A, B)` | `struct { A, B }` | `(values A B)` in a return | `A * B` |
| `Option<T>` | `?T` | `(or null T)` | `t option` |
| `Vec<T>` | **marker** | `vector` | -- |
| `Box<T>` | **marker** | erased to `T` | -- |
| `Cell<T>` | erased to `T` | erased to `T` | -- |
| a type parameter `T` | `comptime T: type` | `t` (erased) | `'t` |
| `impl Fn(..) -> R` | a struct with a `call` method | `function` | -- |
| a crate struct or enum | its name | `point`, `shape-dot` | `Point.t` |

Two mechanics behind that table are worth naming.

**References divide the backends the way nothing else does.** Zig keeps them --
`&T` is `*const T`, `&mut T` is `*T` -- with `&[T]` the exception, since a slice
is its own type. The other two erase them entirely, because a Common Lisp
structure and an OCaml record are already heap references. That single fact is
why `design/match.md`'s `by_ref` captures and the `match_ergonomics` pass matter
only to Zig, and why `doc/lisp.md` can say mutation "needs nothing".

**A type parameter is not syntactically distinguishable.** A bare `T` is just a
path, so the OCaml backend asks the oracle -- `kind_at(...) == Kind::TypeParameter`
-- and emits `'t` rather than a type constructor named `t`. This is the clearest
small case of the oracle answering something `syn` structurally cannot.

## Where the fallbacks differ

Every backend meets types it cannot map, and each shrugs differently:

| | an unmappable *type* | a binding whose type the oracle does not know |
|---|---|---|
| Zig | `// TODO: type` marker | omit the annotation -- `const x = e;`, and Zig infers |
| Common Lisp | `t`, the universal type | omit the `declare` |
| OCaml | `_`, the inference wildcard | nothing to omit; there was no annotation |

So on an unknown type **Zig marks, Common Lisp shrugs, OCaml delegates** -- and
in the binding case all three end up delegating to the target's own inference,
which is why an unresolved `let` type is usually harmless.

Two edges to that:

* **Common Lisp's `t` is correct but unchecked.** It cannot produce a wrong
  answer -- everything is of type `t` -- but it silently drops the declaration
  that SBCL would have enforced, so the overflow check quietly stops applying to
  that binding.
* **OCaml's `_` is not legal everywhere.** Verified: `let (x : _) = 3` and
  `let f (x : _) = x + 1` both compile, while `type t = { xs : _ }` is
  `Error: A type wildcard _ is not allowed in this type declaration`. So an
  unmappable *field* type is a compile error rather than a silent hole, which is
  the right failure -- and it means the fallback's safety depends on position,
  not just on the type.

Where the fallback does bite is a decision that *needs* the type rather than
merely printing it. `design/operator.md`'s narrowing cast is the example: an
operand `expr_type` cannot resolve stays `@as` instead of becoming `@truncate`,
which Zig then rejects for a narrowing conversion. Loud, deliberately -- the
alternative would be a silent truncation.

## Holes

### 1. `expr_type`'s coverage

It handles paths, calls, method calls, casts, parens, indexes, and arithmetic
binaries. It does **not** handle field access, `if`/`match`/block expressions,
unary operators, or references -- so `self.data`'s type is unknown to the shared
inference. The Common Lisp backend patches around this with its own `expr_ty`
wrapper that adds a field case (and `Cell::get`, and a couple of methods); the
Zig backend does not, so a decision keyed on a field's type falls back.

Adding a field case to the shared `expr_type` is cheap -- the struct's fields
are already collected in `analyze` for both backends -- and it is the single
highest-value extension, since field arithmetic is exactly what `rust/bitset`
and `rust/random` are made of.

### 2. `char` and `Vec` in Zig

Both leave markers. `char` is `design/string.md`'s open question (a Unicode
scalar value, not a byte, in a language whose string is a byte slice); `Vec`
needs an allocator story that nothing has designed. Common Lisp maps both --
`character` and `vector` -- which is why `lisp/` has fixtures that `zig/` does
not.

### 3. Slice and array are one type in Common Lisp

Both become `vector`, so the declaration cannot tell `[T; N]` from `&[T]`. This
is right for the target -- a Common Lisp vector covers both, and the length is a
property of the value -- but it means a length mismatch that Rust catches at
compile time and Zig catches in the type is unchecked in the Lisp output.

### 4. Synthetic spans have no types

The desugar passes' discipline (`doc/desugar.md`) is that SCIP is queried only
at original spans, because a rewritten node carries a `call_site` span the
occurrence map does not contain. That is what keeps the passes sound, and it
also means **a type question can only be asked about code the programmer
wrote**. Any future pass that needs to type its own output has to carry the type
along rather than re-deriving it.

## Test

Every fixture exercises this, since nothing translates without it. The ones
that isolate a specific mechanism:

| Path | Role |
|------|------|
| `rust/sum` | `let` bindings with no source annotation at all -- the plainest demonstration that types come from SCIP |
| `rust/hash` | `bytes[i] as u32` -- `expr_type`'s index case feeding the cast decision, and the narrowing/widening split |
| `rust/random` | Shift amounts, wrapping widths, and `rotate_right`'s explicit `T`: three decisions that all fail visibly if the type is unresolved |
| `rust/iter` | An erased `T`: the case where the type exists in Rust and is deliberately gone in two of the three targets |
| `rust/geometry` | `&self` and `&mut self` receivers, where the type is elided in the source and recovered as `Kind::SelfParameter` |

## Not implemented yet

1. Field access in the shared `expr_type` (hole 1).
2. `char` and `Vec` in the Zig backend (hole 2).
3. Anything that would need a real type checker: trait resolution, associated
   types, where-clause reasoning. The oracle answers or the decision degrades.

## Not planned

* Re-deriving types ourselves. rust-analyzer has done it, the input is
  known-good Rust, and a second inference engine would be a second source of
  truth to disagree with.
* Preserving lifetimes. They are erased everywhere, and no target has a
  counterpart.
* Type-checking the *output*. The three test suites compile and run it, which
  is the check that matters, and it catches exactly the loud failures the
  fallbacks above are designed to produce.
