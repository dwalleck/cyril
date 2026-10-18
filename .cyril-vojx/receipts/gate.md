# cyril-vojx gate receipt

Every lane is run with `env -u CARGO_TARGET_DIR`.

## Head `3b933531` (after the review repairs), clean tree

| Lane | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | FAIL: one line in a new `app.rs` test was not rustfmt-shaped. Fixed by `cargo fmt` in the next commit (a formatting-only change; it leaves the other lanes' evidence valid) |
| clippy-all-features | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | OK |
| nextest-all-features | `cargo nextest run --workspace --all-features --no-fail-fast` | OK |
| doctest-all-features | `cargo test --doc --workspace --all-features` | OK |
| clippy-kas | `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | OK |
| nextest-kas | `cargo nextest run -p cyril -p cyril-core --features kas --no-fail-fast` | OK |
| clippy-default | `cargo clippy --workspace --all-targets -- -D warnings` | OK |
| nextest-default | `cargo nextest run --workspace --no-fail-fast` | OK |
| clippy-core-nodefault | `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | OK |
| nextest-core-nodefault | `cargo nextest run -p cyril-core --no-default-features --no-fail-fast` | OK |

## Head after the fmt commit

`cargo fmt --all -- --check`: OK. GitHub CI runs every lane on the pushed
head; its results are recorded in the PR.

## Earlier: head `caf2d8b4` (before review)

All ten lanes OK. nextest: 2565 passed with all features, 1597 with kas,
2558 default, 966 for core without default features.
