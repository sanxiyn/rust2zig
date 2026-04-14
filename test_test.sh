#!/bin/sh
set -e

pass=0
fail=0

for expected in out/*; do
    name=$(basename "$expected")
    rust_dir="rust/${name}"
    zig_file="zig/${name}.zig"

    if [ ! -d "$rust_dir" ]; then
        echo "SKIP $name (no rust package)"
        continue
    fi

    # Test Rust
    rust_out=$(cd "$rust_dir" && cargo run --quiet 2>&1)
    if echo "$rust_out" | diff -q "$expected" - > /dev/null 2>&1; then
        echo "PASS $name (rust)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (rust)"
        echo "$rust_out" | diff -u "$expected" - || true
        fail=$((fail + 1))
    fi

    # Test Zig
    if [ ! -f "$zig_file" ]; then
        echo "SKIP $name (no zig output)"
        continue
    fi
    zig_out=$(zig run "$zig_file" 2>&1)
    if echo "$zig_out" | diff -q "$expected" - > /dev/null 2>&1; then
        echo "PASS $name (zig)"
        pass=$((pass + 1))
    else
        echo "FAIL $name (zig)"
        echo "$zig_out" | diff -u "$expected" - || true
        fail=$((fail + 1))
    fi
done

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
