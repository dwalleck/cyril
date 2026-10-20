# cyril-zj0m review decisions

| finding-id | finding | reviewer | evidence-state | evidence | decision | fix | note |
|---|---|---|---|---|---|---|---|
| F1 | The new pending-approval regression fixture omits PermissionOption's required is_destructive field; set it false as in the benchmark. | ReviewDiffReuse, relayed by Main | Verified | Direct core/fixture source comparison; original scoped repair review; actual-tree workspace log executes the corrected regression successfully. | Accept | Add exactly `is_destructive: false,` to the pending-approval test's AllowOnce PermissionOption. | Resolved; no production cache or permission behavior changed. Historical review and current runtime are separately scoped in verification.md. |

## F1 bounded fixture repair

- **Governing obligation / owning slice:** plan.md's coherent retained diff slice and the approved pending-permission consumer regression. The fixture must implement the existing PermissionOption domain contract; no change to permission behavior, cache semantics, public APIs or approval rendering is needed.
- **Delivered change:** one explicit false field in `widgets::chat::tests::pending_approval_diff_preview_reuses_derivation_and_current_colors`; the benchmark's existing matching field and all production code are unchanged by this repair.
- **Performed checks and results:** direct source comparison established the one-field correction. ReviewDiffReuse's original scoped re-review reported no remaining blockers; it ran no checks and does not itself establish current-tree byte identity. The actual-tree workspace log subsequently executes `pending_approval_diff_preview_reuses_derivation_and_current_colors ... ok`. [verification.md](verification.md) owns the full qualification and current review scopes.
- **Focused command disposition:** the suggested exact filtered command was not run separately; the full `cargo test --workspace` run executed the permanent regression with no filters. That satisfies the required runtime outcome rather than claiming a compile-only or zero-test result. The fixture repair changes no diff defect-sensitivity obligation.
- **Retained independent evidence:** the portable archive's `review/original-independent-review.json` and `sensitivity/receipt.md` preserve original review and original-base executed RED/GREEN for their exact boundaries. They are not relabeled later c6la, current-tree full gates, timing or native proof.
- **Publication ownership:** Main owns the issue-specific commit/PR/CI/tracker actions. Documentation and evidence packaging performs none of those actions and prequalifies no future output-alias integration.

## Historical qualification failure / bounded owner repair

The first actual assembled qualification stopped on `clippy::manual_is_multiple_of` in the dense benchmark predicate (`line % 4 != 0`); no workspace test, CLI build, final format check or benchmark ran in that attempt. This is a retained **historical FAIL**, not a baseline waiver or masked success. Main authorized the owner repair, after which the qualifier captured a new source tree and freshly ran every required assembled gate plus the completed-write benchmark. No retry, suppression, skipped check or timing relaxation is claimed. The original failed source identity/output and repaired qualification remain separate in [verification.md](verification.md) and the portable archive.

## Fresh actual-delivery independent review

ReviewDiffReuse then performed a fresh seven-source review of actual qualified tree `6241931fecb663e9bb17e5cc0e76c35c12833828` against observed HEAD `6ab6d77e03b73a3a45d6d526fbc19641dace5513`, including the two qualified new-file blobs, current consuming seams, final formatting and bounded benchmark predicate correction. Verdict: **correct**, confidence **0.98**, no findings; F1 remains resolved. The reviewer and its scout ran no build, test, benchmark, lint, formatter or smoke and made no source edits. This is independent static review, separate from the qualifier's executed gates and owned actual-App consumer.

The portable `review/actual-delivery-independent-review.json` receipt in `verification-evidence.tar.gz` owns scope and conclusions. It expressly excludes unlinked c6la, inherited unrelated fixes, documentation/publication artifacts and the later output-alias refresh. Future source changes require their own applicability assessment or scoped re-review; original review was not backdated into this verdict.
