#!/bin/sh
set -e

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
    (cd "$dir" && rust-analyzer lsif . > "$name.lsif")
    (cd "$dir" && rust-analyzer scip .)
done
