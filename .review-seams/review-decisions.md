# Review seam remediation — ROADMAP W3

Approved design: the five concrete interface sketches in the conversation. Requester approval: "Okay, make these changes".
Base: e0a8e105200ae6fbd2019de9fb776c56488cd6af. Isolated worktree: /tmp/cyril-review-seams.
No pre-existing route.md + plan.md: repository focused verification applies per Gilfoyle assessing-review-feedback.

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | Separate check execution, capture and report generation | design assessment | Verified | check.rs run_check combines all three; Collector loses read failure state | Accept | 441aa17f: check.rs execute_check/render_diagnostics | blocking: requested scope |
| F2 | Extract comment normalization and rendering | design assessment | Verified | report.rs comments mixes policy and persistence | Accept | ac7b816b: report.rs resolve_comment/render_comment | blocking: requested scope |
| F3 | Type verdict state and decode ranking explicitly | design assessment | Verified | verdicts.rs load/tally use normalized strings; report.rs ranking silently empties malformed notes | Accept | ac7b816b: verdict.rs Verdict, report.rs decode_ranking, core summary | blocking: requested scope; preserve extensible JSON |
| F4 | Relocate shared record and queue helpers | design assessment | Verified | report/verdicts import helpers from merge | Accept | ac7b816b: record.rs accessors and run.rs write_queue | blocking: requested scope |
| F5 | Remove run-directory dependency from repository probes | design assessment | Verified | core review launch constructs placeholder ReviewRun for probes | Accept | 34e911b6: gather/git workspace interfaces and core launch | blocking: requested scope |

The earlier Python-shared sweep/ballot ID collision is outside these five approved interface changes; no reference-fidelity change is made here.

## Verification

Source state: production/test tree at 34e911b6, based on e0a8e105. The commits only record the already-checked files; they did not alter the tested contents. Commands ran in /tmp/cyril-review-seams with `CARGO_TARGET_DIR=/home/dwalleck/repos/cyril/target RUSTC_WRAPPER=`. Existing host settings otherwise supply an empty target dir and a failing sccache wrapper.

- F1 regression sensitivity: on the base check.rs implementation plus FailingReader regression, `cargo test -p cyril-review --lib check::tests::read_failure_keeps_bytes_and_reports_incomplete_capture -- --exact` ran one test and failed at "a failed read is not EOF". After the repair it passes. The initial unqualified --exact invocation ran zero tests and is not evidence.
- F1 checkpoint, contents committed as 441aa17f: `cargo test -p cyril-review` PASS (56 tests); `cargo clippy -p cyril-review --all-targets -- -D warnings` PASS. Controlled readers cover EOF, read failure, task panic, both open streams at a shared deadline and reader termination. Existing process tests cover success, failure, timeout, cancellation and inherited pipes. Pure rendering covers path matching and capture failure independent of clean process exit.
- F2 checkpoint before typed verdicts: `cargo test -p cyril-review` PASS (58 tests), log comments-tests.log; matching all-targets Clippy PASS. Table-driven policy cases cover labels/subjects, fallback, duplicate decorations, blocking eligibility, and trailer rendering; existing pipeline goldens cover ranking-dependent duplicates.
- F3 sensitivity: malformed ranking notes regression fails on the pre-decoder implementation (ranking-red.log), then passes. Typed verdict serialization and normalization, ranking absence/malformed shapes, and application summary tests pass.
- F2/F3 checkpoint, subsequently combined with mechanical F4 moves in ac7b816b: `cargo test -p cyril-review -p cyril-core` PASS (906 tests, 4 existing ignored); matching all-targets Clippy PASS (verdict-tests.log/verdict-clippy.log).
- F4 checkpoint: review crate tests PASS (62), all-targets Clippy PASS (helpers-tests.log/helpers-clippy.log). Queue path-formatting test moved with its helper. F2-F4 are committed together because the final report/verdict implementation imports the relocated helpers; the assembled record-processing change was tested before commit.
- F5 checkpoint, contents committed as 34e911b6: `cargo test -p cyril-review -p cyril-core` PASS (906 tests, 4 existing ignored), matching all-targets Clippy PASS (probes-tests.log/probes-clippy.log). Probes retain scoped counts, root refusal, branch filtering and tracked changes without placeholder run directories.
- Final integration at 34e911b6: `cargo test` outside the sandbox PASS (2,288 tests, 13 existing ignored, 44 test/doc-test suites), workspace-tests-unrestricted.log. Sandbox attempt failed nine local-memory-runtime readiness tests because local IPC was restricted; the identical source passed unrestricted. No baseline exception or waived failure remains.
- Final quality: `cargo clippy --all-targets -- -D warnings` PASS (workspace-clippy.log); `cargo fmt --all --check` PASS; `git diff --check` PASS.
- All nine review golden tests pass; no golden fixture changed. Validation ran on Linux. Windows/macOS execution remains the repository CI's responsibility; no platform lint or test was weakened. Live Kiro streaming smoke is inapplicable: the changes do not rewrite event handlers, rendering or chat lifecycle; public crtool and application consumers are covered by the workspace tests.

Logs remain in this worktree; this record captures commands, checked source, and results without committing thousands of routine test-output lines.

## Independent repair review

- review_diagnostics: F1 source and crtool contract review, PASS; no actionable defect. Checked final check.rs content now in 441aa17f, including reader ownership, shared deadline, abort-and-await and explicit capture failure.
- spec: F2/F3 source and consumer review, PASS; no blocking regression. Checked final normalized verdict, ranking and comment seams now in ac7b816b. Artifact spelling, warning order, fallback and summary semantics preserved.
- standards: F4/F5 plus assembled design conformance, PASS; no substantive findings. Checked final helper ownership and workspace-only interfaces now in ac7b816b/34e911b6 and confirmed no framework introduced.
- Subsequent edits only added contract documentation and this record; source review remains applicable.

## Size

Production Rust before each file's `#[cfg(test)]`, including comments and blank lines: 3,441 base, 3,706 final (+265). The approximate 3,300-line contract budget permits a stated rationale: explicit capture state/ownership, pure comment policy and typed consumer verdicts add meaningful seams. No storage/process/rendering framework or duplicate implementation was added. docs/crtool-contract.md records the rationale. Tests/proof are separate.

## Unresolved failures

None. All five accepted changes, required checks and independent reviews are complete. Local commits only; no push or merge.
