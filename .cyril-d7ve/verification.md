# Exact cyril-d7ve delivery-tree qualification

Date: 2026-10-19. Result: **PASS** for the assigned exact-tree Linux checks and two-profile actual-binary smoke. No source-behavior edits, formatter writes, commits, PRs, tracker mutations, manual test exclusions, or retries were made in this qualification phase. Cargo slot has been released to Main. Native Windows/macOS and publication remain parent-owned.

## Exact delivery scope and source identity

Qualified worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-d7ve`.

HEAD: `8e5c325ebb5dac3a4ad89f71838fd198cc177543` plus exactly these tracked changes, confirmed by one initial `git status --short` and `git diff --name-only 8e5c325e` receipt:

- `README.md` — profile-only user documentation; existing native bundle/store paragraphs retain baseline behavior.
- `crates/cyril-core/src/kiro_agent_config.rs`.
- `crates/cyril-core/src/protocol/domain_mediator/mod.rs`.
- `crates/cyril-core/src/protocol/domain_mediator/tests/serial.rs`.
- `crates/cyril-core/src/protocol/kas/settings.rs`.
- Untracked issue artifacts only: `.cyril-d7ve/`.

This is the assembled independent d7ve delivery tree, not an arbitrary staged subset or the combined root implementation tree. `discovery.rs` is unchanged from base: current `git hash-object` and base `git rev-parse 8e5c325e:<path>` both returned Git blob `f894baf7c1d5c227cc7e3cad875a1d56fe0e1021`. The `relative-bundle-node.sh` fixture and `.cyril-0ssb` directory are absent. No KIRO_DATA_DIR override implementation was imported into this tree.

| File | SHA-256 |
|---|---|
| `README.md` | `a17b8d90d545c5a4a9be9bcde9ebc7a479201da4f19020fb9dff8c506dfa2f66` |
| `crates/cyril-core/src/kiro_agent_config.rs` | `18bd77b6739a9e2f3513a4d44e0eac692c21efb8c0ce3f35e6bbbcd5ffb847dd` |
| `crates/cyril-core/src/protocol/domain_mediator/mod.rs` | `d5b8bb6d3b1f89a710d584bb8faa10cca3040328d0e447a570c5e2a8fc8242c5` |
| `crates/cyril-core/src/protocol/domain_mediator/tests/serial.rs` | `822b1e616b319ade9ef9491b0edb06c1b438a33d0186aef7898d3e4d2335c6a7` |
| `crates/cyril-core/src/protocol/kas/settings.rs` | `030e416487846f6fa31119e3054f5ada4e5917ff931b23078905f476f57606c3` |
| Baseline `crates/cyril-core/src/protocol/kas/discovery.rs` | `7381cb60308015d04b1ecac6d4c197d24731bc6e3355346ac688cf7bf321b611` |
| `Cargo.toml` | `0f4556b40631d219dc441d2742df9b1da9a7e37928fe1cf6a8acbc7ba7f6d6ee` |
| `Cargo.lock` | `9d8af317e2be6201724d88bc5f0137f60d73bd673b244a269eb4c3842a94ecb8` |

The four profile source files and Cargo manifests/lockfile are byte-identical to the previous full Linux matrix's checked identities. Source and README hashes remained identical throughout checks and immediately before/after smoke. Main explicitly granted the Cargo slot after its active RED runs; all seven local crate entrypoints were freshly touched after that grant to defeat stale sibling fingerprints, with byte identities unchanged. Compile/check logs name this exact delivery worktree.

## Fresh exact-tree commands and observed status

All commands ran from the exact worktree above. Compilation/test/clippy/build environment: `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`. Formatting explicitly unsets that variable.

| Exact command | Status and observed result | Receipt label |
|---|---|---|
| `env -u CARGO_TARGET_DIR cargo fmt --all -- --check` | PASS, exit 0; no formatting/source modifications | `format-check` |
| `cargo test --workspace` | PASS, exit 0; 2823 runtime tests passed, 13 repository-declared ignored tests, 0 failures; 2 compile-fail doctests passed. Child reentry summaries are not double-counted. | `workspace-tests` |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS, exit 0 | `workspace-clippy` |
| `cargo build -p cyril` | PASS, exit 0; default KAS-enabled actual executable built from exact tree | `binary-build` |

Exact argv, cwd, environment, exits, durations, and full logs: `/tmp/cyril-d7ve-pr-qualification-20261019/runs.json` plus sibling `<label>.json`/`<label>.log`. Workspace summary: `workspace-summary.json`. No local skip, filter-based exclusion, or retry was introduced; the ignored tests are the repository's existing declarations, and are not claimed as executed proof.

Workspace feature unification includes KAS, so additional affected filters were unnecessary. `affected-fence-execution.json` records executed PASS lines for the profile default/trim and rejection tests, the actual global trust/mediator hook consumer test, and all five selected-profile initialize-settings cases (home-equal, home-ancestor, USERPROFILE ancestor, trimmed override, blank override). The full workspace log also carries the remaining profile lexical/symlink/persistence tests. No profile obligation was satisfied by a silently unexecuted filter.

## Retained unaffected feature-matrix evidence

Prior full Linux matrix and exact prior source identities: `local://root-current-verification.md`, `/tmp/cyril-root-current-verification-20261019/source-identity.json`, and that record's portable qualification archive.

Applicability rationale, as Main directed:

- All four behavior-owning d7ve files, their regression assertions, Cargo features, dependencies, and lockfile are byte-identical. Existing original-defect mutation RED/restored-GREEN evidence in `local://root-sensitivity.md` therefore remains applicable to d7ve's unchanged mutated lines/assertions.
- Prior binary/core **no-default-feature** tests and clippy remain valid for d7ve's feature-exclusion obligations: the sole production difference between the previously qualified combined root tree and this exact tree is KAS-only discovery code, which is absent from those no-KAS builds. The shared profile code and binary/core feature policy did not change.
- Prior all-features/KAS-isolation results remain evidence for the unchanged d7ve code's feature interactions, not a claim that the earlier combined-tree aggregate is this delivery tree's fresh whole-tree result. The fresh exact-tree workspace tests, clippy, default build, and native-default two-profile smoke above establish this assembled delivery and show d7ve does not depend on 0ssb's discovery override.
- No duplicate full all-features matrix was run. No 0ssb-specific padded/relative asset or override-store smoke result is imported into d7ve's completion claim.

## Actual exact-tree Cyril binary smoke — 2/2 PASS

After `cargo build -p cyril`, copied the built executable out of the shared target before releasing Cargo:

`/tmp/cyril-d7ve-pr-qualification-20261019/cyril-d7ve-smoke`

Executable SHA-256: `15269a006182044e21696702cb9824797c1a5cbb51c4169a9946afffe8d4b90d`.

Executed command, exit 0:

```sh
/usr/bin/python3 /tmp/cyril-d7ve-pr-qualification-20261019/smoke.py /tmp/cyril-d7ve-pr-qualification-20261019/cyril-d7ve-smoke
```

Harness SHA-256: `4b65f653b2174e5f258766f1e4c35b6b2ebf4bf1c5fdb12df2ad5cba5830e59e`. Independent static review of the two-case adaptation: `agent://VerifyRootSensitivity.D7veDeliverySmokeReview`, no concrete blockers. This execution, not predecessor smoke, is proof for the exact delivery tree.

Both cases use owned fake `kiro-cli`/`node`/JSONL-agent peers, fresh private HOME/USERPROFILE/XDG/cwd trees, and an explicit environment whitelist. The bundle exists **only** under the native Linux default `$XDG_DATA_HOME/kiro-cli/kas/<version>/.../acp-server.js`. `KIRO_DATA_DIR` is absent from each actual launch manifest; no 0ssb feature is required. No `--agent-command` or direct server override bypasses production discovery. All settings/hooks/auth/wire/render/graceful-shutdown assertions from the proven harness are retained.

| Case | Executed result |
|---|---|
| `trimmed-profile` | PASS. Padded Unicode KIRO_HOME selects relocated workflows/goal true and thinking false in actual initialize and session-new messages; hooks contain exactly relocated global + workspace IDs. Actual Node argv names the native-default bundle and correct flags. Synthetic auth callback returns AT-fake; helper version/token journal and rendered marker are checked. Ctrl+Q exits 0. |
| `invalid-ancestor` | PASS. KIRO_HOME containing private HOME is rejected: actual settings switch to default workflows/goal false and thinking true, hooks to default + workspace, and contains-home warning is required. Native-default bundle/auth/wire/render checks remain, then Ctrl+Q exits 0. |

Per-case portable evidence includes `launch.json` with actual argv/cwd/whitelist and binary hash, `node.argv`, selected server path, both wire directions, checked auth/hooks replies, terminal ANSI, exit code, and PASS receipt. No real credentials, account mutation, or live auth were used.

## Portable evidence

All paths below are under `/tmp/cyril-d7ve-pr-qualification-20261019/`:

- `d7ve-smoke-evidence.zip` — 49044 bytes; SHA-256 `4a0ddc0b914c4c5954ead65f165ce1ca667f41d69d0aee3b6f72590f9c6d95dc`. Two-case harness, source identity, indexed case receipts, and all owned launch/wire/terminal/helper fixtures.
- `d7ve-qualification-evidence.zip` — 162093 bytes; SHA-256 `a0e7f7d47f353bb582f888f359ee5d6911bf80aec77b92cccf2a9a6760221279`. Full command/log receipts, source/entrypoint identities, affected-fence execution receipt, gate plan, nested smoke archive, and exact checked README/source/Cargo files. It excludes the executable binary; the preserved private path/hash and build receipts bind that binary.

Archives use relative member paths and synthetic fixture state only. Parent-owned `.cyril-d7ve` artifacts were not overwritten.

## Limits and remaining ownership

No unresolved assigned local gate or smoke failure remains. Native Windows/macOS execution is not claimed; source-platform fixtures and Linux PTY smoke do not replace native CI. No actual Node/KAS backend login, authenticated upstream extraction, TUI trust-persistence smoke, or sidecar-enrichment smoke is claimed. Parent's supplied user acceptance for the specific known Windows cyril-u62f memory-readiness failure is not invoked by these passing Linux runs and does not waive new failures. Main identifies that public acceptance as PR183 comment6284441811.

Main owns final issue-artifact integration, native CI qualification, commits/PR separation, and publication. This exact d7ve delivery qualification intentionally excludes the later cyril-0ssb bundle behavior.
