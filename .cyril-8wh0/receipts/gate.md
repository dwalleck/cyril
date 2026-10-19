# cyril-8wh0 gate receipt

## Review round 1: head `e75c745a` (code identical to the commit carrying this receipt)

All ten lanes OK: fmt; the three clippy lanes (all features, kas, default);
clippy for core without default features; doctest. nextest: 2600 passed
with all features, 1606 with kas, 2593 default, 966 for core without
default features.

## Before review round 1: head `a17d8584`, clean tree. Every lane is run with `env -u CARGO_TARGET_DIR`.

| Lane | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | OK |
| clippy-all-features | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | OK |
| nextest-all-features | `cargo nextest run --workspace --all-features --no-fail-fast` | OK, 2595 passed, 13 skipped |
| doctest-all-features | `cargo test --doc --workspace --all-features` | OK |
| clippy-kas | `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | OK |
| nextest-kas | `cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast` | OK, 1606 passed, 10 skipped |
| clippy-default | `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | OK, 2588 passed, 13 skipped |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK, 966 passed, 4 skipped |

An earlier run on `a6cb02d2` failed three clippy lanes on
`large_enum_variant`: `ReplayFrame::Permission` grew with the inline plan.
Fixed in `03465a72` by boxing `PermissionRequest.workflow_plan`.
