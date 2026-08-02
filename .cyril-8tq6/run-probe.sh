#!/usr/bin/env bash
# Probe driver for cyril-8tq6 — copies the probe into cyril-core's tests/,
# runs it against the REAL crate, tees output, removes the copy.
set -euo pipefail
cd "$(dirname "$0")/.."
cp .cyril-8tq6/probe_translation.rs crates/cyril-core/tests/probe_cyril_8tq6.rs
trap 'rm -f crates/cyril-core/tests/probe_cyril_8tq6.rs' EXIT
cargo test -p cyril-core --test probe_cyril_8tq6 -- --nocapture | tee .cyril-8tq6/probe-output.txt
