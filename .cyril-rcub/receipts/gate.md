# cyril-rcub gate receipt

## Review round 2 (the commit after `c8571ada`)

The gate ran on the round-2 working tree (base `c8571ada` plus the round-2
code, tests, snapshots and docs). That tree then became the round-2 commit
without further change, apart from this file. Every ci.yml lane was run with
`env -u CARGO_TARGET_DIR`:

| Lane | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | OK |
| clippy-all-features | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | OK |
| nextest-all-features | `cargo nextest run --workspace --all-features --no-fail-fast` | OK — 2405 passed, 13 skipped |
| doctest | `cargo test --doc --workspace --all-features` | OK |
| clippy-kas | `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | OK |
| nextest-kas | `cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast` | OK — 1477 passed, 10 skipped |
| clippy-default | `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | OK — 2398 passed, 13 skipped |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK — 875 passed, 4 skipped |

The insta snapshots ran without `INSTA_UPDATE`, so they match the committed
`.snap` files.

## Earlier heads

`14f5dfec` and `6f5b8ecd` were each green in every lane before round 2.
