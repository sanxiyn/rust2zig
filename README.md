# rust2zig: Rust to Zig transpiler

A compiler from Rust to Zig where high level structures are idiomatically
translated. Generated Zig should be suitable for human consumption.

## Implementation

* Written in Rust, using `syn` for parsing
* Use SCIP for semantic information
* Rust generics are translated to Zig comptime
* Rust references are translated to Zig pointers, lifetimes are erased

### Code structure

* `src/main.rs`: CLI entry point, takes Cargo package directory as argument
* `src/scip.rs`: SCIP loading (prost-generated bindings from `proto/scip.proto`),
  exposes occurrence -> symbol and symbol -> (kind, signature) maps
* `src/lsif.rs`: LSIF loader, currently unused (kept for reference)
* `src/translate/mod.rs`: `Rust2Zig` struct, analysis pass, shared helpers
  (path, case conversion, check_moniker, path_mode)
* `src/translate/expr.rs`: expression translation
* `src/translate/item.rs`: item translation (enum, struct, method, fn)
* `src/translate/mac.rs`: macro translation (`assert_eq!`, `panic!`, `println!`)
* `src/translate/pat.rs`: pattern translation
* `src/translate/stmt.rs`: statement and block translation
* `src/translate/ty.rs`: type translation
* `build.rs`: compiles `proto/scip.proto` via `prost-build`
* `build_index.sh`: regenerates `<name>.lsif` and `index.scip` for every example
* `coverage.sh`: runs `test.sh` under `cargo-llvm-cov`, excluding the
  prost-generated `target/.../out/scip.rs` from the report. Output goes to
  `coverage/text/`; current findings are summarized in `coverage.md`.

### Analysis pass

Pre-translation pass (`analyze`) collects metadata from the AST into two
maps: `HashMap<String, Enum>` and `HashMap<String, Struct>`.

`Enum` has:
* `has_data: bool`: whether any variant has fields
* `is_generic: bool`: whether the enum has type parameters
* `impls: Vec<syn::ItemImpl>`: collected impl blocks

`Struct` has:
* `impls: Vec<syn::ItemImpl>`: collected impl blocks

The analysis pass runs in two phases: first collects enum and struct decls,
then attaches each impl block to its corresponding enum or struct.

### SCIP integration

rust-analyzer SCIP dumps provide semantic information.

* SCIP files: `rust/<name>/index.scip`, generated via `rust-analyzer scip .`
* `Scip::symbol_at(range)`: resolves a source position to a SCIP symbol string
* `Scip::kind_at(range)`: resolves to `SymbolInformation.Kind`
* `Scip::type_at(range)`: for `Kind::Variable` and `Kind::Parameter`
  symbols, parses the suffix after `: ` in `signature_documentation.text`
  (e.g. `let xs: [i32; 5]`, `xs: &[i32]`) as `syn::Type`
* `SymbolInfo::range`: the definition occurrence's range (set from the
  occurrence carrying `SymbolRole::Definition`). Used by `has_capture`
  to tell outer locals from closure-introduced bindings.
* `check_moniker(path, expected)`: maps logical Rust paths
  (`core::option::Option::Some`, `std::macros::println`, ...) to SCIP
  descriptor suffixes and suffix-matches against the occurrence's symbol
* `path_mode(path)`: returns `EnumVariant` iff the path's last segment
  resolves to a symbol of kind `EnumMember`

### Testing

`test.sh` runs each example under `rust` through the translator and compares
output byte-for-byte against the corresponding files under `zig`. Expected
files are regenerated from translator output after each change.

`test_test.sh` runs `cargo test` on each Rust example and `zig test` on the
corresponding translated Zig file. This ensures the input/output pairs used
to test the translator are in fact equivalent.

Both suites accept optional name arguments (e.g. `./test.sh gcd divmod`) to
run a subset; with no arguments, all examples run.

Examples currently passing both suites: gcd, direction, div, option, result,
ratio (struct), divmod (tuple), sum (for loop), geometry, closure.

## Notes

* `let` bindings always emit a Zig type annotation resolved via
  `Scip::type_at` on the binding ident, ignoring any source-level
  Rust annotation (which may contain wildcards like `[T; _]`).
* For loops translate when the iterable is an array, an `&[T]` slice
  (treated identically — Zig captures the element directly), or a
  range. Closed Rust ranges (`a..=b`) become `a..(b+1)` in Zig; the
  capture is `usize`, so the body is wrapped with a preamble
  `const x: T = @intCast(_x);` using `Scip::type_at` on the loop var.
* Block emission goes through `translate_block_with_preamble`, which
  takes pre-built lines inserted before the body — used by mutable-arg
  shadowing (`var a = _a;`) and range-loop intCast.
* Non-capturing closures (`let f = |x| x * 2;`) translate to a local
  struct-wrapped fn: `const f = struct { fn call(x: T) R { ... } }.call;`.
  Param types come from `Scip::type_at` on each param ident; the return
  type is parsed out of the binding's `impl Fn(..) -> R` signature via
  `closure_return_type`. `has_capture` (a `syn::visit` walk that compares
  each ident's SCIP definition range against the closure span) gates the
  translation — capturing closures fall back to a TODO.

## Bugs

* Union field/method name collision: In Zig, union(enum) fields and methods
  share the same namespace. Rust enum variants like `Ok`/`Err` become fields
  `ok`/`err` which collide with methods of the same name. Needs a renaming
  strategy.
* Format specifiers: Without type info, `println!("{}", x)` translates
  to `std.debug.print("{}\n", .{x})`. This works for integers but not for
  strings. Currently hacked with sed, see `test.sh`.
* For loops over iterators (other than ranges and `&[T]` slices) are
  TODO.
* Closure param shadowing: a closure param with the same name as an
  outer local (e.g. `let x = 3; let f = |x| x * 2;`) compiles in Rust
  but the emitted Zig fails with "function parameter 'x' shadows local
  constant from outer scope", since Zig doesn't allow shadowing across
  the struct-wrapped fn boundary. Needs a renaming strategy.
