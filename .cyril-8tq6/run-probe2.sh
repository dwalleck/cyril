#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cp .cyril-8tq6/probe2_resolution.rs crates/cyril-core/tests/probe2_cyril_8tq6.rs
trap 'rm -f crates/cyril-core/tests/probe2_cyril_8tq6.rs' EXIT
cargo test -p cyril-core --test probe2_cyril_8tq6 -- --nocapture | tee .cyril-8tq6/probe2-output.txt
