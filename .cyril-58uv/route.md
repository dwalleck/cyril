# Route: cyril-58uv

Change: Make all unhandled extension conversion results visible at the mediator boundary.
Date: 2026-09-10

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The issue supplies measured KAS captures. The main-line baseline has a silent inbound.rs Ok(None) arm, but kiro.rs already logs unknown methods. No new external premise is needed: a regression exercises the real handler and converter directly. | no |
| 2 | Structural module shape | domain_mediator/inbound.rs retains dispatch and conversion-outcome diagnostic ownership (its Err arm already warns). Generic unknown-method and acknowledged-without-forwarding diagnostics move out of the converter to avoid duplication. Converter warnings explaining rejected input remain there. No interface, schema, dependency direction, or domain responsibility changes. | no |
| 3 | Production-scale risk | One mediator debug event per non-forwarded conversion, with borrowed canonical method only; no payload serialization, queues, or retained state. | no |
| 4 | Explicit behavior | Given conversion returns Ok(None), when the mediator processes an extension notification with debug enabled, emit a neutral non-forwarding DEBUG diagnostic carrying its canonical method, not payload, and retain Ok(false) without App delivery. Unknown methods and acknowledged multi-session methods receive exactly one method diagnostic. Given conversion produces a notification or an error, retain delivery or warning without classifying those outcomes as non-forwarded Ok(None). This implements the ticket's explicit minimum fix without adding KAS notification families. | yes |

Unknown tests: none.

## Selected route

Local — explicit diagnostic fix within existing dispatch ownership.

## Required artifacts

| Artifact | Owner | Status |
|---|---|---|
| route.md | change-workflow | this file |
| spec.md | interrogated-spec | N/A — behavior fully explicit |
| evidence.md, probe.* | prove-it-prototype | N/A — no unverified premise |
| design.md | falsifiable-design | N/A — Local route: no design gate |
| plan.md | budgeted-plan | N/A — Local route: no plan gate |

Oracle checkpoint in checkpointed-build: N/A — Local route.

## Downstream sequence

None — normal repository fix/regression process. Live KAS remeasurement and hypothesis expansion are unnecessary for this bounded diagnostic change; the real mediator/converter regression supplies the feedback loop.

## Terminal criterion

PASS — `protocol::domain_mediator::tests::serial::unhandled_extension_diagnostic_excludes_payload_and_preserves_dispatch` exercises unknown methods, recognized notifications discarded by conversion, successful delivery, and conversion errors with the existing thread-local structured tracing capture.

Checked source: fix/cyril-58uv linked worktree; inbound.rs adds method-only DEBUG on Ok(None), kiro.rs removes the duplicate unknown-method fallback log, serial.rs adds the behavioral regression. Environment: Linux x86_64, repository Rust 1.94.0, shared Cargo target directory `/home/dwalleck/repos/cyril/target`.

Results, 2026-09-10:

- Initial narrower unknown-method regression passed before edits because the converter already logs unknown methods. This does not establish mediator coverage; the regression was strengthened to cover a recognized method returning None.
- RED: `cargo test -p cyril-core --lib unhandled_extension_diagnostic -- --nocapture` failed before production edits: `every unhandled conversion must identify its method`. Source had the strengthened regression and original production code. Session evidence: artifact://10.
- GREEN: same focused command passed after the production edits. `cargo test -p cyril-core --features kas` passed: 1,018 tests, 9 ignored. The subsequent all-target Clippy check caught a test-only expect(), replaced with existing must_succeed. Session evidence: artifact://12.
- Final PASS: `cargo fmt --all --check`; `cargo test` (1,956 passed, 13 ignored); `cargo clippy -- -D warnings`; `cargo test -p cyril-core --features kas --lib unhandled_extension_diagnostic` (1 passed); `cargo clippy -p cyril-core --all-targets --features kas -- -D warnings`. Session evidence: artifact://16. The test-only error-unwrapping correction does not invalidate the prior full KAS suite; the focused KAS regression and all-target Clippy were rerun on final source.

## Cleanup and delivery

No temporary harness, instrumentation, dependency, public API, or UI change. No existing root changelog was found. This route documents the diagnostic change; architecture/domain docs are intentionally unchanged. This change does not implement additional KAS notification families or surface their payloads in the UI. Main now includes the separately delivered powers panel from PR #122. The issue remains unclosed pending merge.

Publication authorization, 2026-09-12: requester said “Prepare fix/cyril-58uv and make a PR for it”. Scope: integrate current main, review and verify the diagnostic fix, push fix/cyril-58uv and open its PR. No issue closure or merge authorized.

## PR preparation, 2026-09-12

Base: origin/main `131efdd4`; integration merge: `449c0b01`. The only merge conflict was the cyril-58uv tracker note. Main's tracker bytes were retained; branch implementation and historical verification evidence remain above. No unrelated assignment migrations were published.

The Local route remains applicable: method-only conversion-outcome diagnostics retain the existing domain owner and do not change notification delivery. Main integration invalidates reliance on the historical test counts as current gate results.

Independent review F1 found duplicate method diagnostics for acknowledged `kiro.dev/session/activity` / `kiro.dev/session/list_update` notifications. Verified by extending the existing regression: it failed with two DEBUG events for activity instead of one. Repair removes the redundant converter arm and uses the neutral mediator message “extension notification not forwarded”; recognized-but-not-surfaced outcomes are not mislabeled unknown. Converter warnings explaining invalid input remain intact.

Fresh focused proof:

- Original pre-fix converter and mediator arms restored temporarily: the regression compiled and failed at “every unhandled conversion must identify its method”; restored implementation passed.
- Original unknown converter fallback restored alone: the regression compiled and failed at the exactly-once assertion (two method diagnostics instead of one); restored implementation passed.
- F1 regression on `449c0b01` production: failed at the acknowledged notification's exactly-once assertion (two instead of one). After the ownership repair, the same focused KAS regression passed.

Command for each focused check: `cargo test -p cyril-core --features kas --lib unhandled_extension_diagnostic -- --nocapture`. Mutation files were backed up and restored byte-exactly outside the repository; no mutant is included in the PR.

Final local gates on `449c0b01` plus the F1 repair, 2026-09-12 (Linux, Rust 1.94.0, shared target above): PASS. No production or test edits followed this run.

- `cargo fmt --all -- --check`
- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo clippy -p cyril -p cyril-core --features kas --all-targets -- -D warnings`
- `cargo clippy -p cyril-core --no-default-features --all-targets -- -D warnings`
- `cargo nextest run --workspace --all-features`: 1,993 passed, 13 skipped.
- `cargo nextest run -p cyril -p cyril-core --features kas`: 1,216 passed, 10 skipped.
- `cargo nextest run -p cyril-core --no-default-features`: 751 passed, 4 skipped.
- `cargo test --doc --workspace --all-features`: PASS.

Cross-platform qualification is owned by the PR's Windows/macOS CI jobs, not claimed from the Linux runs.

Independent repair re-review: Review58uvRepair inspected the three-file F1 repair, the complete PR diff, both engine conversion paths, and the test capture helper. No findings; notification delivery and existing converter warnings remain unchanged. The final formatting and full local gates above cover the repaired tree. The review's stale-route caveat is resolved by this PR-preparation section and the reconciled route rows; historical 2026-09-10 receipts are not claimed as current evidence.
