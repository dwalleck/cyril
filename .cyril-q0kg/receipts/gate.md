# cyril-q0kg gate receipt

Checked head: `7962f526bb00e77f7ba267ff489cc8e11fbf7eec` (clean tree). Every
ci.yml lane, each run with `env -u CARGO_TARGET_DIR`:

| Lane | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | OK |
| clippy-all-features | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | OK |
| nextest-all-features | `cargo nextest run --workspace --all-features --no-fail-fast` | OK — 2381 passed, 13 skipped |
| doctest | `cargo test --doc --workspace --all-features` | OK |
| clippy-kas | `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | OK |
| nextest-kas | `cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast` | OK — 1477 passed, 10 skipped |
| clippy-default | `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | 2373 passed, 1 failed: `capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown` (known flake cyril-xy4x, 100 ms wall-clock budget under load; no path through this diff). Isolated rerun on the same head: PASS. Same lane on the prior head `670e03a8` (which differs only in the log field's `%` formatter): 2374 passed. |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK — 875 passed, 4 skipped |
