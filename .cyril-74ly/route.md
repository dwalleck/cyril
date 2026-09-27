# Route: cyril-74ly

Change: Replace the two pre-existing private `CaptureWriter` copies in `crates/cyril-core/src/protocol/convert/mod.rs` (test module only) with the canonical `crate::test_support::CaptureWriter`.
Date: 2026-09-27

Branch: `chore/cyril-74ly-dedup-capture-writer` (linked worktree; base `main` @ `f9bc81d8`).

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | No external or system premise. The canonical writer already exists (`crates/cyril-core/src/test_support.rs:157-194`), is gated by `lib.rs:18-19` `#[cfg(any(test, feature = "test-support"))] pub mod test_support;`, and is already consumed from `crates/cyril-core/src/workflow.rs:1193` (`crate::test_support::CaptureWriter::default()` + `.captured()`) and `crates/cyril/src/app.rs:6842` (via the `test-support` dev-feature). Both convert sites already call `crate::test_support::tracing_capture_lock()` (`convert/mod.rs:846`, `:1636`), so the module is provably reachable under every feature matrix the tests compile in — `cfg(test)` is set for the lib's unit tests regardless of `--features`. The only behavior delta (write() returning `io::Error` on lock poisoning instead of `.expect` panic) is a requester decision, not a premise to probe. | no |
| 2 | Structural module shape | Test-only edit inside `#[cfg(test)] mod tests` of `convert/mod.rs` (lines 822-841 and 1610-1628 hold the two private copies; `:854` and `:1655` reach into the private `.0` field, which becomes `.captured()`). No production line changes; no public interface, schema, seam, or dependency direction changes; `tracing-subscriber` is already a `[dev-dependencies]` entry of cyril-core (`Cargo.toml:68`). Responsibility ownership unchanged: `test_support` already owns the canonical capture scaffolding (cyril-6beh); `convert/mod.rs` tests remain consumers. Length gate: none exists in the repository (`grep -rln "max-lines\|max_lines\|length gate\|file-length\|line-count" scripts .github` → no hits; no `clippy.toml`). `convert/mod.rs` is 2,915 lines today and this change removes ~38 lines; projected growth 0, so no length-review trigger. | no |
| 3 | Production-scale risk | None. Zero production code is touched; the writer only runs inside unit tests under a thread-scoped `with_default` subscriber. | no |
| 4 | Explicit behavior | Given the `mod tests` in `convert/mod.rs`, when a test needs a log-capture `MakeWriter`, then it uses `crate::test_support::CaptureWriter` and reads the buffer via `captured()`; the two private `struct CaptureWriter` definitions (and their `io::Write` / `MakeWriter` impls) are deleted. Given a poisoned capture mutex, when `write()` runs, then it returns `io::Error` (canonical behavior) instead of panicking — requester-approved reconciliation. Given the `--all-features`, default-features, `--no-default-features`, and `--features kas` builds of cyril-core, when `cargo clippy --all-targets -- -D warnings` and `cargo nextest run` execute, then all compile and pass. Given the existing tests in the affected module (`protocol::convert::tests::*`, notably `to_ext_notification_metadata_refusal_and_stop_reason_not_flagged`, `to_ext_notification_metadata_unknown_key_logged`, `from_permission_response_selected_foreign_id_warns_and_sends_as_is`), when run ≥5 times under nextest before and after the change, then the pass counts are identical (flake behavior unchanged). Out of scope by requester decision: cyril-2rgp (intermittent empty tracing-capture race); observed instances are recorded, not fixed. | yes |

Unknown tests: none.

## Selected route

Local — mechanical, test-only dedup within an existing responsibility owner; every behavior is explicit and requester-approved.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit (T4 verdict yes; requester decisions recorded in the task brief) |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise (T1 verdict no) |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in `checkpointed-build`: N/A — Local route: checkpointed-build does not run.

## Downstream sequence

none — implement with normal repository fix process (mechanical migration), then the focused verification below.

## Terminal criterion

Local — focused behavioral verification:

1. Fence (flake-behavior parity): `cargo nextest run -p cyril-core --all-features protocol::convert::tests` ×5 on the pre-change tree and ×5 on the post-change tree; pass counts must match run-for-run.
2. Focused compile/lint on the migrated module: `cargo clippy -p cyril-core --all-targets --all-features -- -D warnings`.
3. Full CI-mirror gate (fmt, clippy ×4 feature legs, nextest ×4 feature legs, doc tests) with real exit codes — receipts appended below.

### Fence receipts (flake-behavior parity), 2026-09-27

Environment: Linux x86_64 (7.2.3-1-cachyos), Rust 1.94.0 via `rust-toolchain.toml`, `cargo nextest`, this worktree's own `target/` (`CARGO_TARGET_DIR` unset via `env -u` — the inherited shell exported it as an empty string, which makes every cargo command fail before compiling; the first baseline attempt was discarded for that reason and rerun). Filter: `protocol::convert::tests` — the one `#[cfg(test)] mod tests` in `convert/mod.rs` that held both private copies. Runner script: session scratchpad `fence.sh` (loop of `env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features protocol::convert::tests`, one log per run, pass/fail taken from the process exit code).

| Tree | Source state | Run 1 | Run 2 | Run 3 | Run 4 | Run 5 |
|---|---|---|---|---|---|---|
| Pre-change baseline | `f9bc81d8`, `convert/mod.rs` clean | PASS 112/112 (961 skipped) | PASS 112/112 | PASS 112/112 | PASS 112/112 | PASS 112/112 |
| Post-change | `f9bc81d8` + this diff (7+/48−, test-only) | PASS 112/112 (961 skipped) | PASS 112/112 | PASS 112/112 | PASS 112/112 | PASS 112/112 |

- Five tests consume the writer (count corrected in review round 1, N1 — the original text said three): the four `with_captured_logs` callers `to_ext_notification_metadata_refusal_and_stop_reason_not_flagged` (`:849`), `to_ext_notification_metadata_unknown_key_logged` (`:864`), `to_ext_notification_metadata_refusal_corrupt_object` (`:932`), `to_ext_notification_metadata_refusal_corrupt_subfield_and_stop_reason` (`:956`, `:971` — two calls), plus the inline site in `from_permission_response_selected_foreign_id_warns_and_sends_as_is` (`:1597`); line numbers anchored to `6be4e1cc`. All five appear as `PASS` in every run's log on both trees (grep receipt, not inferred from the summary line) — the fence ran the whole `protocol::convert::tests` filter, so the receipts stand unchanged.
- No `FAIL`, `FLAKY`, or retry line in any of the ten logs; the cyril-2rgp empty-capture race (out of scope by requester decision) was not observed.
- Verdict: **PASS** — pass counts identical run-for-run before and after; the migration does not change flake behavior.

### Focused lint receipt

`Result: 2026-09-27 | env -u CARGO_TARGET_DIR cargo clippy -p cyril-core --all-targets --all-features -- -D warnings | PASS` (GATE-OK echo observed; 42.55 s).

### Final T2 recheck

`convert/mod.rs`: 2,915 → 2,874 lines (−41). No production line touched (`git diff` hunks are all inside `#[cfg(test)] mod tests`); no new responsibility, interface, seam, or dependency; `test_support` remains the single owner of the capture scaffolding. No length gate exists to trigger. Verdict: no new trigger — Local route stands.

### Full gate receipts, 2026-09-27

Source state: `f9bc81d8` + this branch's single Rust diff (the tree that is committed below; `route.md` is the only other change and is not a cargo input). Environment as above. Sequential run via session scratchpad `gate.sh`; every row comes from a `GATE-OK-<name>` / `GATE-FAIL-<name>` line written only from the command's real exit code (no pipes to `tail`).

| Command | Result | Receipt |
|---|---|---|
| `cargo fmt --all -- --check` | PASS | GATE-OK-fmt |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | GATE-OK-clippy-ws-allfeat |
| `cargo nextest run --workspace --all-features` | PASS | GATE-OK-nextest-ws-allfeat — 2057 passed, 13 skipped |
| `cargo test --doc --workspace --all-features` | PASS | GATE-OK-doctest-ws-allfeat |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS | GATE-OK-clippy-ws-default |
| `cargo nextest run --workspace` | FAIL on first run → PASS on rerun | first run: GATE-FAIL-nextest-ws-default (exit 100) — 2054 passed, **1 failed**, 13 skipped; the one failure is `cyril-workbench::reviewer runtime_drop_retains_evidence_until_core_completion` (see Discovered, not fixed). Rerun of the identical command on the identical tree: GATE-OK-nextest-ws-default-rerun — 2055 passed, 13 skipped. |
| `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings` | PASS | GATE-OK-clippy-core-nodefault |
| `cargo nextest run -p cyril-core --no-default-features` | PASS | GATE-OK-nextest-core-nodefault — 782 passed, 4 skipped |
| `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings` | PASS | GATE-OK-clippy-kas |
| `cargo nextest run -p cyril -p cyril-core --features kas` | PASS | GATE-OK-nextest-kas — 1250 passed, 10 skipped |

The convert test module is compiled and run in all four nextest legs (all-features, default, no-default, kas), so the `cfg(any(test, feature = "test-support"))` gate on `test_support` is proven to admit the canonical `CaptureWriter` under every feature combination CI builds.

### Discovered, not fixed

**`cyril-workbench::reviewer runtime_drop_retains_evidence_until_core_completion` is a pre-existing timing race, unrelated to this change.**

- Failure text (gate leg, `cargo nextest run --workspace`): `thread 'runtime_drop_retains_evidence_until_core_completion' panicked at crates/cyril-workbench/tests/reviewer.rs:712:25: assertion `left == right` failed / left: Some("R") / right: Some("Z")`.
- Mechanism (mode A, `reviewer.rs:712`): the assertion reads `/proc/<pid>/stat` for the spawned `pid` and `child` and accepts either NotFound (already reaped) or state `Z` (zombie); it observed `R` (still running) — the probe raced ahead of the child's exit.
- Characterization on the same tree: 10 isolated runs of `cargo nextest run -p cyril-workbench runtime_drop_retains_evidence_until_core_completion` → 7 PASS / 3 FAIL (runs 2, 3, 9). The isolated failures expose a **second mode B** in the same test: runs 2 and 9 panicked at `crates/cyril-workbench/tests/reviewer.rs:99:9: assertion `left == right` failed: private trees retained after finish / left: 1 / right: 0` — `peer.cleaned()` found one entry still under `runtime_parent`, i.e. the cleanup waiter had not yet removed the private tree when the assertion ran. Run 3 reproduced mode A. Both modes are teardown-timing races (child exit / directory removal vs. the test's probe), and the window widens under load — this host was running eleven sibling agents' cargo builds throughout. Full-leg rerun → 2055/2055 PASS.
- Provenance: `crates/cyril-workbench/tests/reviewer.rs`, last touched `b19b75c6` (2026-09-07, "fix(workbench): enforce reviewer privacy and lifetime contracts"); this branch has an empty diff under `crates/cyril-workbench` (`git diff --stat -- crates/cyril-workbench` prints nothing). No rivets issue mentions the test name (`grep` of `.rivets/issues.jsonl`, read-only). Not filed here — the orchestrator owns the tracker.
- cyril-2rgp (tracing-capture empty-buffer race, out of scope): **not observed** in any of the 10 fence runs, the 4 gate nextest legs, or the full-leg rerun.

### Terminal criterion result

`Result: 2026-09-27 | fence ×5 pre / ×5 post (env -u CARGO_TARGET_DIR cargo nextest run -p cyril-core --all-features protocol::convert::tests) + full CI-mirror gate above | PASS` — the focused verification named in this criterion is green, the full gate is green on the committed tree (the single red leg was a documented unrelated pre-existing flake, green on rerun), and the final T2 recheck records no new trigger. Local route complete.
