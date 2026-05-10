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
        continue
    fi
    if zig test "$zig_file" > /dev/null 2>&1; then
        echo "PASS $name (zig)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (zig)"
        fail=$((fail + 1))
    fi
done

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
