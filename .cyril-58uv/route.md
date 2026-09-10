# Route: cyril-58uv

Change: Make all unhandled extension conversion results visible at the mediator boundary.
Date: 2026-09-10

## Route tests

| # | Test | Evidence | Verdict |
|---|------|----------|---------|
| 1 | Empirical premise | The issue supplies measured KAS captures. Current source has a silent inbound.rs Ok(None) arm, but kiro.rs already logs unknown methods. No new external premise is needed: a regression exercises the real handler and converter directly. | no |
| 2 | Structural module shape | domain_mediator/inbound.rs retains dispatch and conversion-outcome diagnostic ownership (its Err arm already warns). Unknown-method logging moves out of the converter fallback to avoid duplicate diagnostics. No interface, schema, dependency direction, or domain responsibility changes. | no |
| 3 | Production-scale risk | One debug event per unhandled conversion, with borrowed canonical method only; no payload serialization, queues, or retained state. | no |
| 4 | Explicit behavior | Given conversion returns Ok(None), when the mediator processes an extension notification with debug enabled, emit a DEBUG diagnostic carrying its canonical method, not payload, and retain Ok(false) without App delivery. Unknown methods receive exactly one method diagnostic. Given conversion produces a notification or an error, retain delivery or warning without classifying those outcomes as unhandled. This implements the ticket's explicit minimum fix before deciding which KAS notification families to model. | yes |

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

No temporary harness, instrumentation, dependency, public API, or UI change. No existing root changelog was found. This route and the tracker note document the diagnostic change; architecture/domain docs are intentionally unchanged. The seven KAS feature families remain unmodeled, as required by the ticket's minimum-fix-first scope; this change does not claim to surface their payloads in the UI. Issue remains in_progress pending merge.

Publication authorization: requester said “commit and push”. Scope: commit this issue's implementation, regression, route, and tracker update to fix/cyril-58uv and push that branch to origin. No issue closure or merge authorized. Prior verification remains applicable: no production or test edits since the final passing checks.
