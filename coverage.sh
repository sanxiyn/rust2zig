#!/bin/bash
set -e

source <(cargo llvm-cov show-env --sh --remap-path-prefix)
cargo llvm-cov clean
cargo build --quiet
./test.sh
IGNORE='--ignore-filename-regex /build/[^/]+/out/'
cargo llvm-cov report $IGNORE --text --output-dir coverage
cargo llvm-cov report $IGNORE --json --summary-only --output-path coverage/summary.json
QUERY='
.data[0].files.[]
| "\(.filename) \(.summary.lines.covered)/\(.summary.lines.count)"
'
jq -r "$QUERY" coverage/summary.json
