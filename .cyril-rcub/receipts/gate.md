# cyril-rcub gate receipt

Checked head: `6f5b8ecdf7ac09987bf20c4380dcb3acdf76734d` (clean tree). Every
ci.yml lane, each run with `env -u CARGO_TARGET_DIR`:

| Lane | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | OK |
| clippy-all-features | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | OK |
| nextest-all-features | `cargo nextest run --workspace --all-features --no-fail-fast` | OK — 2395 passed, 13 skipped |
| doctest | `cargo test --doc --workspace --all-features` | OK |
| clippy-kas | `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | OK |
| nextest-kas | `cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast` | OK — 1477 passed, 10 skipped |
| clippy-default | `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | OK — 2388 passed, 13 skipped |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK — 875 passed, 4 skipped |

The insta snapshots ran with `CI=true` semantics in the lanes above (no
`INSTA_UPDATE`), so they match the committed `.snap` files.
