# Levenshtein distance (levenshtein)

**Level 1 implemented in OCaml.** `rust/levenshtein` is the byte-level
extraction described under [Suggested levels](#suggested-levels), and
`ml/levenshtein` is a passing golden. There is no Zig golden yet, so `test.sh`
reports `SKIP levenshtein`. The [gap analysis](#what-the-translator-does-today)
below comes from running the published crate, unedited, through both backends
from a scratch package, and is annotated with what has been fixed since.

The crate is `levenshtein` 1.0.5 (`wooorm/levenshtein-rs`, MIT, ~30M
downloads): one 67-line `src/lib.rs` holding one function. Of the candidates in
this directory it is the smallest that needs **heap allocation**, and the
allocation is as simple as it gets: one local `Vec`, sized up front, freed at
the end of the function. That is the case for it. [regex.md](regex.md) and
`design/box.md` both conclude that Zig's allocator decision "wants the smallest
possible surface", and this crate is that surface.

## Why this crate

* **Leaf.** No dependencies; the published `.crate` ships only `Cargo.toml` and
  `src/lib.rs`.
* **No macros, traits, generics, or types.** One free function,
  `fn(&str, &str) -> usize`. The only attribute is `#[must_use]`, which needs
  no translation.
* **Not `no_std`.** It uses `alloc` for `Vec<usize>`. Like
  [regex.md](regex.md), it breaks the README's no-alloc constraint on purpose.
  Here the alloc *is* the point, and it is far smaller than regex's
  recursive `Box` tree.
* **Real control flow in a small space**: early `return`, nested `for` over
  `enumerate`, `if` as an expression including an `else if` chain, indexed
  read *and* write on a `Vec`.

## Real source (essence)

```rust
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
    let mut result = 0;
    if a == b {
        return result;
    }
    let length_a = a.chars().count();
    let length_b = b.chars().count();
    if length_a == 0 { return length_b; }
    if length_b == 0 { return length_a; }

    let mut cache: Vec<usize> = (1..).take(length_a).collect();
    let mut distance_a;
    let mut distance_b;

    for (index_b, code_b) in b.chars().enumerate() {
        result = index_b;
        distance_a = index_b;
        for (index_a, code_a) in a.chars().enumerate() {
            distance_b = if code_a == code_b { distance_a } else { distance_a + 1 };
            distance_a = cache[index_a];
            result = if distance_a > result {
                if distance_b > result { result + 1 } else { distance_b }
            } else if distance_b > distance_a {
                distance_a + 1
            } else {
                distance_b
            };
            cache[index_a] = result;
        }
    }
    result
}
```

The above is the whole crate, minus the license comment. The published crate
has no tests (`include` drops them), so a fixture's tests would be written
fresh. The usual pair would do: `("kitten", "sitting") == 3`, and the
degenerate cases `("", "abc") == 3` and `("abc", "abc") == 0`.

## What the translator does today

### Unedited: both backends panic

`let mut distance_a;`, a `let` with no initializer, reaches
`local.init.as_ref().unwrap()` in both `translate_local`s
(`src/translate/zig/stmt.rs:84`, `src/translate/ml/stmt.rs:79`) and crashes
the translator. That is worse than the project's usual failure mode: an
unhandled construct is supposed to leave a `TODO`, not abort the whole file.

### With the two declarations initialized to `0`

Zig (abridged):

```zig
fn levenshtein(a: []const u8, b: []const u8) usize {
    var result: usize = 0;
    if (a.* == b.*) { return result; }
    const length_a: usize = a.chars().count();
    ...
    var cache: Vec(usize) = (/* TODO: expr */).take(length_a).collect();
    ...
    for (b.chars(), 0..) |code_b, index_b| {
        ...
        for (a.chars(), 0..) |code_a, index_a| {
            distance_b = /* TODO: expr */;
            distance_a = cache[index_a];
            result = /* TODO: expr */;
            cache[index_a] = result;
        }
    }
    return result;
}
```

OCaml (abridged):

```ocaml
let length_a = count (chars a) in
...
let cache = collect (take (* TODO: expr *) length_a) in
...
Array.iteri (fun index_b code_b ->
    ...
    Array.iteri (fun index_a code_a ->
        distance_b := (if code_a = code_b then !distance_a else !distance_a + 1);
        distance_a := Dynarray.get cache index_a;
        result := (if !distance_a > !result then (if !distance_b > !result then !result + 1 else !distance_b));
        Dynarray.get cache index_a := !result) (chars a)) (chars b);
```

The early `return` (OCaml's local `Return` exception), the `ref` cells, the
`Dynarray.get` read, and OCaml's `a = b` on strings all come out right.
Everything else is a gap, and several of the gaps are **silent**.

## Gaps

| # | Construct | Zig today | OCaml today | Kind |
|---|---|---|---|---|
| 1 | `let mut x;` (no initializer) | panic | panic | crash |
| 2 | `.chars().count()` / `.take().collect()` | emitted verbatim as method calls | emitted as bare `count (chars a)` | **silent** |
| 3 | `for (i, c) in s.chars().enumerate()` | `for (b.chars(), 0..)` | `Array.iteri … (chars b)` | **silent** |
| 4 | `else if` in an `if` expression | `/* TODO: expr */` | `else` branch **dropped**; **fixed** | **silent** (OCaml) |
| 5 | `&str == &str` | `a.* == b.*` | `a = b` (correct) | wrong (Zig) |
| 6 | `Vec<usize>` | `Vec(usize)`, no such type | `Dynarray.t` (correct) | missing (Zig) |
| 7 | `v[i] = x` | `cache[index_a] = result` | `Dynarray.get cache i := x`; **fixed** | wrong (OCaml) |
| 8 | `(1..)` open range | `TODO` | `TODO` | missing |
| 9 | `if` with block branches used as a value | `TODO` | correct | missing (Zig) |
| 10 | `for (i, b) in bytes.iter().enumerate()` (level 1 only) | correct | `Array.iteri` on a `string`; **fixed** | wrong (OCaml) |

Gap by gap:

1. **Uninitialized `let`.** The crash wants fixing whatever happens to this
   candidate. Zig has a direct spelling, `var distance_a: usize = undefined;`,
   with the type taken from `Scip::type_at` exactly as for an initialized
   binding. OCaml has none, since a `ref` needs a value. Short of a desugar that
   moves the declaration down to its first assignment (valid here, since every
   use comes after one, but a dataflow question in general), the OCaml backend
   should emit a `TODO`. A fixture would hand-edit both to `let mut x = …` at
   first use, which is what idiomatic Rust writes anyway.
2. **Unknown method calls pass through.** Both backends fall back to emitting
   a method they don't recognize as an ordinary call (`translate_method_call`'s
   last arm). For a crate-defined method that is correct; for a `core` method
   the result looks like a translation and is not one. A crate method resolves
   to a symbol in the crate, so the fallback could check that and emit a
   `TODO` for anything else. Of every gap here, this would catch the most
   silently-wrong output in future candidates.
3. **The `enumerate` shape doesn't check its receiver.** `translate_for_enumerate`
   (both backends) matches the `enumerate` moniker and then uses whatever
   `peel_iter` hands back. `peel_iter` strips only `.iter()`, so a `.chars()`
   receiver goes through untouched and is iterated as if it were an array. The
   shape should require an array-like receiver after peeling (`is_array_like`
   in `ml/flow.rs`), and otherwise fall through to `TODO`, as `design/flow.md`
   says anything outside the five shapes does. **Still open.** Gap 10 added a
   string-or-array dispatch to OCaml's `enumerate`, but a `Chars` receiver is
   neither, so it still falls to `Array.iteri`.
4. **`else if`.** **Fixed in OCaml:** `translate_if` and `translate_if_option`
   now translate the `else` branch with `translate_expr`, so a chain recurses.
   Zig's `translate_if` (`src/translate/zig/flow.rs:168`) still has the
   `if let syn::Expr::Block` shape and drops the branch. Here that shows up as a
   `TODO` only because of gap 9.
5. **String equality in Zig.** `design/equality.md` lists `&str` / `String`
   as a **compile error** for Zig and leaves it there. The `binary` desugar has
   additionally dereferenced both slices (`a.*`), which is not valid Zig for a
   slice either. The target spelling is `std.mem.eql(u8, a, b)`.
6. **`Vec` in Zig.** See the next section.
7. **Indexed write on a `Vec`.** **Fixed in OCaml:** `translate_assign` has
   an `Expr::Index` arm, `Dynarray.set` for a `Vec` base and `xs.(i) <- x`
   (`ArraySet`) otherwise (`design/vec.md`).
8. **`(1..).take(n).collect()`** is an iterator chain, out of scope by
   `design/flow.md`. A fixture would rewrite it as a push loop
   (`for i in 1..=length_a { cache.push(i); }`), which needs nothing new in
   OCaml.
9. **Zig `if` as a value.** Not only assignment: the level 1 fixture's
   `let distance_b = if … { … } else { … };` is a `TODO` too, so it is any
   `if` with block branches used as a value.
10. **Byte iteration in OCaml.** Found by the level 1 fixture, not the
    original crate. A loop over `&[u8]` emitted `Array.iteri` on what is an
    OCaml `string`. **Fixed:** the slice and `enumerate` shapes now choose
    `String.iter` / `String.iteri` by `is_string_expr`, and rebind the element
    with `Char.code` so it is an `int`, like a `u8` (`design/string.md`).

## The Zig allocator decision, at minimum size

This is the reason to pick the crate. Every allocation question `design/box.md`
lists comes up here in its smallest form:

```zig
pub fn levenshtein(allocator: std.mem.Allocator, a: []const u8, b: []const u8) !usize {
    ...
    var cache: std.ArrayList(usize) = .empty;
    defer cache.deinit(allocator);
    for (1..length_a + 1) |i| try cache.append(allocator, i);
    ...
}
```

* **Fallibility stays local.** Only `levenshtein` itself becomes `!usize`;
  there are no constructors whose callers would inherit
  `Allocator.Error`, which is the ripple that makes `Box` hard.
* **The allocator is one parameter.** A test passes `std.testing.allocator`,
  which also *checks for leaks*. That makes `test_test.sh` a free verification
  of the next point.
* **Freeing is the existing `Drop` machinery.** `cache` is dropped at the end
  of the function, which is the plainest case `design/drop.md` handles. The
  `defer cache.deinit(allocator)` is its Zig emission, with
  `std.ArrayList`'s `deinit` as the destructor. Nothing is moved out, so no
  drop flag.

The open choice is the same as regex's: thread `std.mem.Allocator` as a
parameter (honest, changes the signature) or reference a file-level `std.heap`
constant (cheap, keeps the signature). This crate is small enough to settle it
with the honest version and see what it costs.

## `chars()` versus bytes

The crate is character-level: `chars().count()` and `chars().enumerate()`.
Neither backend has a lowering for walking a string by characters.
`design/char.md` deliberately implemented only the "decode one character at a
byte offset" idiom, and `design/flow.md` lists `.chars()` among the shapes that
become a marker.

| Rust | Zig | OCaml |
|---|---|---|
| `s.chars().count()` | `std.unicode.utf8CountCodepoints(s)` (an error union; `catch unreachable` is sound since `&str` is valid UTF-8) | no stdlib function; a fold with `String.get_utf_8_uchar` |
| `for (i, c) in s.chars().enumerate()` | `Utf8View` iterator plus a counter | a `while` over byte offsets with a counter, or a decoded `Uchar.t array` built first |

Neither row is one call in either target, which is why this belongs in a later
level, not the first.

## Suggested levels

**Level 0: fix bugs, no fixture needed.** Gaps 1 (crash → `undefined` / `TODO`),
2 (unknown `core` methods → `TODO`), 3 (`enumerate` receiver check), and 4
(`else if`) are bugs on their own, and each is a small change. Only gap 4 is
done, and only in OCaml; 1, 2, and 3 are open in both backends, and 4 is
open in Zig.

**Level 1: byte-level, hand-edited. Done in OCaml.** `rust/levenshtein`
iterates `bytes_a.iter().enumerate()` instead of `a.chars().enumerate()`, uses
`.len()` instead of `.chars().count()`, builds `cache` with `vec![]` and a push
loop over `1..=length_a`, and declares the two `distance` variables at first
use. Its test covers equal strings, both empty cases, `kitten`/`sitting` (3),
`flaw`/`lawn` (2), and a one-letter substitution. The edits avoid gaps 1, 2,
3, and 8, which leaves:

* OCaml: gaps 4, 7, and 10 were fixed for it, and `ml/levenshtein` passes
  `dune test`.
* Zig: `std.mem.eql` (5), `std.ArrayList` with the allocator (6, the payload),
  `vec![]` (a `TODO: macro` today), `push`, `if` as a value (9), and `else if`
  (4). Iterating a `&[u8]` with `enumerate` is already a recognized shape and
  comes out right.

The byte-level version gives the same results on ASCII input and different
ones beyond it, the same way `bitset` is "not `fixedbitset`". The fixture
carries no comment saying so, so it is recorded here.

**Level 2: characters.** Restore `chars()`; needs the table above in both
backends. This is the first case of *walking* a string by characters, so it
probably wants its own section in `design/char.md` rather than a one-off idiom
match.

## Verdict

A good candidate, but for allocation rather than breadth. Its language
surface is small and mostly covered already. Its value is in being the
minimum case for Zig `Vec` plus allocator, sitting between `rust/nat` (`Box`,
OCaml only) and `rust/regex` (everything at once). As a bonus, running it
turned up four bugs (gaps 1–4) that affect existing code paths, and the level 1
fixture a fifth (gap 10). In OCaml the fixture paid off immediately: three
fixes (4, 7, 10), and a passing golden.
