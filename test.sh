#!/bin/sh
set -e

cargo build --quiet

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
    expected="zig/${name}.zig"
    if [ ! -f "$expected" ]; then
        echo "SKIP $name (no expected output)"
        continue
    fi
    cargo run --quiet -- "$dir" > "/tmp/rust2zig_${name}.zig"
    if diff -q "$expected" "/tmp/rust2zig_${name}.zig" > /dev/null 2>&1; then
        echo "PASS $name"
        pass=$((pass + 1))
    else
        echo "FAIL $name"
        diff -u "$expected" "/tmp/rust2zig_${name}.zig" || true
        fail=$((fail + 1))
    fi
    rm -f "/tmp/rust2zig_${name}.zig"
done

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
