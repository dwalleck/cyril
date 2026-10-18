# cyril-q0kg gate receipt

Checked head: `80f0433b397a970a365a475c368909d4076e5f61` (clean tree). Every
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
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | OK — 2374 passed, 13 skipped |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK — 875 passed, 4 skipped |

Earlier heads were also checked:

- `670e03a8`: every lane green.
- `7962f526`: every lane green except one known-flake hit, cyril-xy4x
  (`capture_forwarder::tests::c9_forwarder_batches_and_drains_before_runtime_shutdown`).
  That test passed on an isolated rerun.

`80f0433b` only moves the warning below `push_back`, so it merges cleanly with
the current PR #174.
