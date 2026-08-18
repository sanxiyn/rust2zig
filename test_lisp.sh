#!/bin/sh
set -e

cargo build --quiet

pass=0
fail=0

if [ "$#" -gt 0 ]; then
    names="$@"
else
    names=$(ls rust)
fi

for name in $names; do
    source="rust/${name}"
    expected="lisp/${name}.lisp"
    if [ ! -f "$expected" ]; then
        echo "SKIP $name (no expected output)"
        continue
    fi
    target="/tmp/rust2_lisp_${name}"
    cargo run --quiet -- lisp "$source" "$target"
    actual="${target}/${name}.lisp"
    if diff -q "$expected" "$actual" > /dev/null 2>&1; then
        echo "PASS $name"
        pass=$((pass + 1))
    else
        echo "FAIL $name"
        diff -u "$expected" "$actual" || true
        fail=$((fail + 1))
    fi
    rm -rf "$target"
done

echo "$pass passed, $fail failed"
[ "$fail" -eq 0 ]
