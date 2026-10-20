# cyril-0ssb — exact stacked delivery verification

Date: 2026-10-19. **PASS** for the exercised exact-branch Linux checks and original five-case actual-binary smoke. Qualification was executed by VerifyRootSensitivity; this publication-artifact update ran no checks and changed no source or README.

## Qualified tree and scope

- Worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-0ssb`.
- Base: `8146f7a53cd85088917bf64e05ed665f8183ecd3`, the published cyril-d7ve commit / [PR #185](https://github.com/dwalleck/cyril/pull/185).
- Owning changes: bundle-only README paragraphs, `crates/cyril-core/src/protocol/kas/discovery.rs`, executable `crates/cyril-core/tests/fixtures/relative-bundle-node.sh`, and `.cyril-0ssb/` records. The d7ve profile implementation/documentation is inherited, not reintroduced by this issue.
- The verifier observed fixture mode 755, invalidated stale shared-target entrypoint metadata without changing bytes, and bound compile markers to this exact worktree. Source and README stayed frozen through qualification and smoke.

## Portable receipts

[receipts/index.json](receipts/index.json) is the owning publication index. Paths below are relative to this record; archives use relative members and contain synthetic credentials only, not executable binaries.

| Archive | Bytes | SHA-256 from verifier receipt |
| --- | ---: | --- |
| [receipts/0ssb-qualification-evidence.zip](receipts/0ssb-qualification-evidence.zip) | 411248 | `0d87faa8b35e9470268a24aa77ff822d6355b60595007c39c0d70254b58e8236` |
| [receipts/0ssb-smoke-evidence.zip](receipts/0ssb-smoke-evidence.zip) | 189145 | `1e717654e338e8f5636ff7c0ec9049969d38e8f2fc513af73ba27888ef980f35` |

The qualification archive preserves exact command/log/count/fence/source receipts, checked source/README/Cargo bytes, the nested smoke archive, and the retained full Linux matrix. Original execution receipts came from `/tmp/cyril-0ssb-pr-qualification-20261019/`; published evidence does not depend on that temporary path remaining available.

## Fresh exact-branch commands

Cwd: the qualified worktree above. Compile/test/clippy/build used `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target`; formatting explicitly unsets it. All commands exited 0, with no formatter writes, manual exclusions or retries.

| Executed command | Observed result |
| --- | --- |
| `env -u CARGO_TARGET_DIR cargo fmt --all -- --check` | PASS; no writes |
| `cargo test --workspace` | 2826 runtime tests passed, 0 failed, 13 existing repository-declared ignored; 2 compile-fail doctests passed. Reentered child summaries were not double-counted. |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo build -p cyril` | PASS; default KAS-enabled executable built in this tree |

The portable archive's `runs.json`, per-command `.json`/`.log`, and `workspace-summary.json` preserve exact argv/environment/cwd/exit/duration/counts. `affected-fence-execution.json` confirms the actual cross-cwd AgentProcess regression, environment-wrapper discovery regression, original-spelling/lazy-default override test, and inherited global-profile trust/hooks consumer executed successfully. Ignored live/integration tests are not represented as executed.

## Retained full-matrix applicability

The verifier established byte identity for all six relevant behavior/test/fixture files, the existing fake helper, Cargo manifests and Cargo.lock against the previously fully qualified combined-root snapshot. Prior all-feature/default/KAS-isolation/no-default tests and clippy, release build, doctests and native POSIX prefix evidence therefore retain their original source/feature applicability. Their exact receipts and source identity are preserved under `retained-full-matrix/` in the owning qualification archive.

Those commands are **retained evidence**, not fresh commands from this stacked tree: their original cwd/revision remain recorded. Main explicitly directed reuse rather than a redundant full-matrix rerun. Fresh exact-branch workspace checks and smoke above additionally qualify the assembled stack. Original-defect mutation and parent-observed F1 RED retain their historical evidence owners; the fresh workspace run supplies exact-delivery GREEN.

## Fresh actual-binary smoke — 5/5 PASS

After the successful build, the verifier copied the actual executable out of the shared target before releasing Cargo ownership. Smoke ran against that immutable private copy:

- Executable SHA-256: `b569d68a7c059e64bfd6044a7fbdcec23be6cbbcb02344e44e6bc6e4e7b77ad7`.
- Harness SHA-256: `8f6f5f00143ef648175d0e4dce2b2fda63308bc9760def265cad09f2d9f05dde`, identical to the already reviewed original five-case harness.
- Executed command, exit 0: `/usr/bin/python3 /tmp/cyril-0ssb-pr-qualification-20261019/smoke.py /tmp/cyril-0ssb-pr-qualification-20261019/cyril-0ssb-smoke`.

| Case | Discriminating observed result |
| --- | --- |
| `trimmed-profile` | Relocated settings in initialize/session-new and relocated global plus workspace hooks; original override bundle root directly under `/kas`; exact fake CLI version wins over newer/default decoys. Exit 0. |
| `invalid-ancestor` | HOME-containing profile rejected to default settings/hooks with warning; invalid-root decoy excluded. Exit 0. |
| `padded-assets` | Original space-containing asset spelling reaches selected server/Node argv instead of the competing trimmed-root extraction. Exit 0. |
| `relative-assets-different-cwd` | Bundle resolves from launch cwd and remains selected when the real child starts under a different `--cwd`; wire/render/shutdown complete. Exit 0. |
| `stored-fallback` | Auth callback returns native-default synthetic SQLite token rather than override-store token; both owned database byte hashes unchanged; unsupported-helper result cached after one probe. Exit 0. |

Each case requires actual Cyril CLI/environment selection, production bundle discovery without server/agent-command bypasses, child spawn, initialize/session-new/prompt, auth/hooks callbacks, selected file/argv, terminal completion marker and graceful Ctrl+Q exit 0 before PASS. Archive `index.json` and per-case launch/wire/terminal/helper/store receipts preserve those observations; completion files alone were not accepted.

Key checked identities (complete map in the qualification archive's `source-identity.json`): discovery `0c6868d443f3081904e0d3f9c8c8393504207adc663033870fa46234e6e57b85`; executable fixture `0afc9d9a21dda5e0a08605d1ce520f4834daee91a484e2e13fa2c9762e45a95d`; README `4cf73642059c3f5d56b22f0e1f791ad22a6bb7ac30d8d6d094e2afb6bfd6429b`.

## Review, limits and publication ownership

Main reports final ReviewKiroData verdict **correct**, confidence 0.98, for this exact stacked delivery. Prior repair reviews remain applicable because production/regression bytes are unchanged; accepted F1–F4 decisions are recorded in [review-decisions.md](review-decisions.md). README already carries the bundle-only behavior; no additional README or architecture edits were made during this publication-artifact task.

This is Linux-native Cyril under PTY with owned fake helper/node/agent peers and synthetic credentials. It does not claim native Windows/macOS execution, real Node/KAS backend authentication, authenticated upstream extraction, TUI trust-persistence smoke or sidecar-enrichment smoke. Source-derived Windows normalization review and Linux fixtures do not replace native CI. No real credentials/account state were used. The separate user-accepted Windows cyril-u62f memory-readiness exception was not invoked and waives no unrelated failures.

No unresolved assigned Linux check/smoke failure remains. Main owns dedicated commit/PR publication, native CI evidence and tracker closure. The 0ssb PR stacks on d7ve / PR185 and does not expand that issue's independent scope.
