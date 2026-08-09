#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd "$(dirname "$0")/.." && pwd)
example="$repo_root/crates/cyril-ui/examples/cyril_z4eo_probe.rs"
trap 'rm -f "$example"' EXIT
mkdir -p "$(dirname "$example")"
cp "$repo_root/.cyril-z4eo/probe.rs" "$example"
cargo run --quiet -p cyril-ui --example cyril_z4eo_probe
