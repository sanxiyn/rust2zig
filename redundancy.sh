#!/bin/bash
set -e

OUT=coverage/pertest
IGNORE='--ignore-filename-regex /build/[^/]+/out/'

if [ "$#" -gt 0 ]; then
    names="$@"
else
    names=$(ls rust)
fi

source <(cargo llvm-cov show-env --sh --remap-path-prefix)
cargo llvm-cov clean
cargo build --quiet

rm -rf "$OUT"
mkdir -p "$OUT"
for name in $names; do
    if [ ! -f "zig/${name}.zig" ]; then
        echo "SKIP $name (no expected output)"
        continue
    fi
    cargo llvm-cov clean --profraw-only
    ./test.sh "$name" > /dev/null
    ./test_ml.sh "$name" > /dev/null
    cargo llvm-cov report $IGNORE --json --output-path "$OUT/${name}.json" 2> /dev/null
    echo "MEASURED $name"
done

python3 redundancy.py "$OUT"
