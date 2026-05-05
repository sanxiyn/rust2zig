#!/bin/bash
set -e

source <(cargo llvm-cov show-env --sh --remap-path-prefix)
cargo llvm-cov clean
cargo build --quiet
./test.sh
cargo llvm-cov report --text --output-dir coverage --ignore-filename-regex '/build/[^/]+/out/'
