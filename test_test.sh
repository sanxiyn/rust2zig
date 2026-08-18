#!/bin/sh
set -e

pass=0
fail=0

if [ "$#" -gt 0 ]; then
    dirs=""
    for name in "$@"; do
        dirs="$dirs rust/$name/"
    done
else
    dirs=rust/*/
fi

for dir in $dirs; do
    name=$(basename "$dir")
    rust_dir="rust/${name}"
    zig_file="zig/${name}.zig"
    ml_dir="ml/${name}"
    lisp_file="lisp/${name}.lisp"

    # Test Rust
    if (cd "$rust_dir" && cargo test --quiet) > /dev/null 2>&1; then
        echo "PASS $name (rust)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (rust)"
        fail=$((fail + 1))
    fi

    # Test Zig
    if [ ! -f "$zig_file" ]; then
        echo "SKIP $name (no zig output)"
    elif zig test "$zig_file" > /dev/null 2>&1; then
        echo "PASS $name (zig)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (zig)"
        fail=$((fail + 1))
    fi

    # Test OCaml
    if [ ! -d "$ml_dir" ]; then
        echo "SKIP $name (no ml output)"
    elif (cd "$ml_dir" && dune runtest) > /dev/null 2>&1; then
        echo "PASS $name (ml)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (ml)"
        fail=$((fail + 1))
    fi

    # Test Common Lisp
    if [ ! -f "$lisp_file" ]; then
        echo "SKIP $name (no lisp output)"
    elif sbcl --script "$lisp_file" > /dev/null 2>&1; then
        echo "PASS $name (lisp SBCL)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (lisp SBCL)"
        fail=$((fail + 1))
    fi

    if [ ! -f "$lisp_file" ]; then
        :
    elif ecl --shell "$lisp_file" > /dev/null 2>&1; then
        echo "PASS $name (lisp ECL)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (lisp ECL)"
        fail=$((fail + 1))
    fi
done

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
