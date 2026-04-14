# rust2zig: Rust to Zig transpiler

A compiler from Rust to Zig where high level structures are idiomatically
translated. Generated Zig should be suitable for human consumption.

## Implementation

* Written in Rust, using `syn` for parsing
* Use LSIF for semantic information
* Rust generics are translated to Zig comptime
* Rust references are translated to Zig pointers, lifetimes are erased

### Code structure

* `src/main.rs`: CLI entry point, takes Cargo package directory as argument
* `src/lsif.rs`: LSIF loading and indexing (MonikerMap, span->range conversion)
* `src/translate/mod.rs`: `Rust2Zig` struct, analysis pass, shared helpers
  (path, type, block, case conversion, check_moniker)
* `src/translate/expr.rs`: statement and expression translation
* `src/translate/item.rs`: item translation (enum, fn, method)
* `src/translate/pat.rs`: pattern translation

### Analysis pass

Pre-translation pass (`analyze`) collects metadata from the AST into
`HashMap<String, Enum>` where `Enum` has:
* `has_data: bool`: whether any variant has fields
* `is_generic: bool`: whether the enum has type parameters
* `impls: Vec<syn::ItemImpl>`: collected impl blocks

The analysis pass runs in two phases: first collects enums, then attaches
impl blocks to their corresponding enum.

### LSIF integration

rust-analyzer LSIF dumps provide semantic information. Used for detecting
standard library items.

* LSIF files: `rust/<name>/<name>.lsif`, generated via `rust-analyzer lsif .`
* `MonikerMap`: maps `Range` (line/column) to moniker
* `check_moniker(path, expected)`: resolves a `syn::Path`'s last ident span
  to an LSIF moniker and compares against expected (handles 0-based/1-based
  line/column conversion)
* Currently used for: `core::option::Option`, `core::option::Option::Some`,
  `core::option::Option::None`, `std::macros::panic`, `std::macros::println`

### Testing

`test.sh` runs each example under `rust` through the translator and compares
output byte-for-byte against the corresponding files under `zig`. Expected
files are regenerated from translator output after each change.

`test_test.sh` compiles and runs original Rust examples and translated Zig
examples and compares output against expected output under `out`. This ensures
input/output pairs used to test the translator is in fact equivalent.

## Bugs

* Union field/method name collision: In Zig, union(enum) fields and methods
  share the same namespace. Rust enum variants like `Ok`/`Err` become fields
  `ok`/`err` which collide with methods of the same name. Needs a renaming
  strategy.
* Format specifiers: Without type info, `println!("{}", x)` translates
  to `std.debug.print("{}\n", .{x})`. This works for integers but not for
  strings. Currently hacked with sed, see `test.sh`.
