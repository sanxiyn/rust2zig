#!/bin/sh
set -e

for dir in rust/*; do
    name=$(basename "$dir")
    (cd "$dir" && rust-analyzer lsif . > "$name.lsif")
    (cd "$dir" && rust-analyzer scip .)
done
