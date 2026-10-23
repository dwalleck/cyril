# Route: cyril-dpfh

Change: classify demonstrated Linux proc-stat identity disappearance in the reviewer integration-test helper.
Date: 2026-10-23
Issue: `.rivets/issues.jsonl`, `cyril-dpfh` (separate from the closed `cyril-peaq`).
Source and execution provenance: [portable proof receipt](proof/receipt.json). Main's tracker-only commit does not itself contain the repaired working source.
Authorization/order: Main authorized the standalone owned-process probe and original-classifier RED; after those observations, Main authorized the minimal ESRCH guard, then the two exact formatter-reported reflows. This post-proof route and packet were requested only after corrected GREEN. No earlier user-specific design approval or earlier route artifact is claimed. Git, tracker, PR, integrated CI and merge remain Main-owned.

## Route selection

| Test | Verdict | Evidence |
| --- | --- | --- |
| T1: consequential unresolved empirical premise | No remaining premise | Before changing classification, the authorized owned-child probe demonstrated that an unread proc-stat descriptor opened while alive returns actual ESRCH after normal release and exact-child reap. The historical CI throwing operation remains unknown, but does not determine the demonstrated observation contract. |
| T2: structural ownership/interface change | No | Only `crates/cyril-workbench/tests/reviewer.rs` changes behavior. A minimal private classifier accepts the actual proc-read result; the same test owner retains the proc observation and lifecycle predicate. No public interface, dependency, production module or process owner changes. |
| T3: production-scale risk | No | No production path changes. The regression owns one stdin-gated child with detached output, bounded exact-child wait/cleanup and no sleeps. |
| T4: explicit behavior | Yes | A live identity remains not exited; actual retained-descriptor ESRCH means that proc identity disappeared. Existing NotFound/zombie handling remains; unrelated I/O errors retain their kind/errno and propagate. Original owning-root, both-PID, Core-completion, evidence-retention, late-write, empty-parent and deadline controls remain unchanged. |

Selected route: **Local**. The empirical premise was resolved before the classification change; no public architecture or unresolved behavior requires a separate design/prototype/plan. This route owns scope and completion; `proof/receipt.json` owns source pins, commands, results and the exact checksummed packet allowlist.

## Delivered behavior and negative space

- `process_has_exited` still reads `/proc/<pid>/stat`. Its private classifier recognizes only NotFound or Linux raw errno 3 (ESRCH) as disappearance, retains the original successful zombie-state classification and propagates every other error.
- The existing cleanup-predicate regression retains its partial-root and live-current-process controls. It also verifies a real unrelated directory-read error retains its I/O kind/errno, observes a gated live child as not exited, retains an unread proc-stat descriptor, normally releases and exactly reaps that child, requires the actual descriptor read to return ESRCH and classifies that observation as exited.
- Regression stdout/stderr are null. Fallible live observation is saved through normal wait or a bounded cleanup attempt; success assertions follow exact-child reap. Wait and cleanup each have a five-second bound; cleanup failure propagates and kill-on-drop remains a backstop, not an infallible-reap claim. Cleanup uses the same owned Tokio `Child` handle, without claiming pidfd internals. The standalone Python probe separately used pidfd-only emergency signalling.
- No product kill/killpg, group supervision, runtime teardown, evidence ownership, EOF/late-write behavior, fixture lifetime, retry policy, configuration, Linux gating or existing lifecycle deadline changed. The two affected lifecycle consumers retain their original assertions. No native/provider/auth acceptance is claimed.

Historical context only: `.cyril-peaq/route.md` records earlier owning-root/process-state corrections; `.cyril-tqr8/shared-reviewer-eof-prerequisite/VERIFICATION.md` records earlier EOF/retention evidence. Neither is reopened or represented as the current ESRCH proof. The scalar `No such process` in [PR201 CI run39873211761](https://github.com/dwalleck/cyril/actions/runs/39873211761) does **not** prove its throwing operation. This repair fixes the demonstrated proc-observation defect, not a definitively attributed historical CI cause.

## Source-bound proof

The [portable receipt](proof/receipt.json) indexes exact original/RED/pre-format-GREEN/qualified-GREEN source, source patches, nonsecret build configuration, the owned probe and its result, raw RED/format-stop/GREEN logs, and retained toolchain/host observations. Payload paths are packet-relative; absolute provenance is metadata rather than a dependency on the original host. No binaries, old archives or nested historical evidence trees are included.

| Obligation | State | Portable evidence |
| --- | --- | --- |
| Owned-process kernel mechanism | PASS | `proof/owned-proc-stat-probe.py` and sibling JSON separately record live control, normal exact-child wait/reap and actual retained-descriptor ESRCH. One authorized execution, not rerun. |
| Original classifier discrimination | PASS — intended RED observed | `proof/red.log`, `proof/reviewer.red.rs`, and `proof/original-to-red.patch` retain the actual original-classifier failure. [INFERENCE] Final classifier propagation follows the controlled source path; the scalar log has no dynamic callsite trace. No unchanged-CI-test confirmation rerun. |
| Formatting | PASS on qualified source | `proof/green-fmt.log`. The earlier formatting-only stop before any GREEN tests/Clippy remains in `proof/fmt-stop.log`; the exact pre-format source and formatting patch are retained. |
| Regression and actual affected consumers | PASS | `proof/green-reviewer.log` records the complete reviewer target, including the deterministic regression and both affected lifecycle consumers. Counts and exact commands are owned by the receipt. |
| Strict lint | PASS | `proof/green-clippy.log`, strict reviewer-target Clippy. |

These are Linux default-feature reviewer-target results, not whole-workspace, all-feature, Windows, other-platform or published-head CI claims. No additional mutation program is necessary for this narrow classifier: original-classifier RED and actual retained-descriptor GREEN exercise the changed observation directly. This does not requalify or supersede unrelated historical retention-mutation obligations.

## Ownership and terminal criterion

The implementation worker ran only the authorized standalone owned-child probe; the coordinator owns all Cargo/format/lint executions above. No worker Git/index/commit/push/tracker/PR operations or broad documentation changes were performed. This route does not close the issue or authorize publication.

- **COMPLETE — independent review:** [final ordinary review](review.md) found no actionable source, proof, or documentation defects (confidence 0.96), covering the qualified source, actual RED/probe/GREEN evidence, and this narrow route. It does not establish the historical CI throwing operation or replace current-head CI.
- **PENDING — Main:** open the separate repair PR, satisfy applicable current-head CI and review gates, and perform tracker/publication/merge operations under the existing authorization.

Terminal criterion: the issue's demonstrated classifier behavior, preserved controls and source-bound RED/GREEN/lint evidence remain valid, independent review is complete, and Main's separate PR has the required green CI and publication disposition. The local implementation/qualification handoff is complete; end-to-end issue delivery is not yet declared complete.
