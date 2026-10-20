# cyril-ej7y implementation plan

Consumes approved design.md (DG Main, 2026-10-19). One issue increment after cyril-un4p and before cyril-xavt. Actual delivery is based on published parent `2fd94879db1b4281dc85cf7dcf528649c1da3087` (PR #188), not the original combined worktree. Parent performed the qualification; this finalization changes documentation/receipts only. The 12-visible-row and 600-code-point limits remain hard and unchanged. No feature line budget was supplied.

## Slice1: full comparison through a bounded navigable preview

**Outcome:** design C1–C5: full KAS comparison reachable without a four-row information cap, <=12 visible body rows including status; plan fallback marker counted within12; path/choice/editor and input guarantees preserved.

**Changes:** workflow_plan retains complete cleaned comparison and shrinks tree with an explicit omission-marker minimum. ApprovalState gains one request-local offset and all constructors migrate atomically. Existing approval widget owns bounded demand, effective window, navigation affordance and shared geometry query; UiState mutates offset, App delegates dedicated navigation before editor handling. Keep exact unmodified keys and later scope/trust/folder behavior. Include settled fallback accounting and all related regression tests in this same issue's commit. No new dependency/framework.

**Verification:**
- Retained parent gate G1: renderer/App/state boundary, geometry, key, fallback, cleaning and snapshot regressions exercised by `cargo test --workspace`; clippy all targets with warnings denied, default build and fmt check completed in the same chained invocation. Canonical commands, checked-state provenance and result are in verification.md.
- Retained actual process smoke S1: safe owned ACP workflow approvals exercised comparison pagination, wire decisions, geometry, resize, selection/reason editing and queue reset. Documentation follows that smoke.
- Independent review R2 freshly inspected the exact reconstructed nine-file EJ tree and found no actionable defect (correct/confidence0.94). Earlier R1 remains explicitly historical. Exact reviewed blob identities, consuming-dispatch subreview and the separate runtime evidence live in verification.md.
- No historical pre-fix behavioral RED receipt was retained. Parent subsequently obtained S2: one identical actual workflow fixture against preserved old-parent/new-EJ binaries, with the same keys/viewport and opposite late-comparison reachability. Fixture compilation and pre-launch static observation-fence repair are not product RED. No baseline-failure waiver is inferred; exact chronology and bounded coverage live in verification.md.

**Dependencies:** cyril-un4p edit slice for width/tail visibility behavior; parent gate/review before publication. No unresolved pre-implementation premise or design decision remains.

## Checkpoint receipt

**Delivered:** complete retained workflow comparison through the bounded shared viewport, request-local navigation and migrated constructors, explicit tree omission, comparison-only narrow continuation indentation, and plan fallback accounting. Ordinary Text/raw-text remains five content rows plus a marker; ordinary Diff remains 20 diff lines plus a marker. “Full” does not mean full file Diff/Myers output or cache expansion.

**Performed checks and results:** [verification.md](verification.md) owns G1 (actual artifact647), S1 (first runtime smoke, 24/24 manual RejectOnce responses, exit0), S2 (actual old/new behavioral sensitivity), R1 (historical scoped review) and the exact-tree review disposition, command/input provenance, limits and portable artifacts. These are retained parent-run results, not checks performed by the documentation worker. Frozen production, test, fixture and Cargo paths are intentionally unchanged during finalization.

**Unresolved failures:** none in the local G1/S1/S2/R2 obligations. S2 discharges retained-comparison sensitivity without inventing a historical RED; R2 supplies fresh exact-tree review. Main still owns assembled publication/remote CI and tracker actions; this documentation-only handoff claims none of them.

## Integration boundary

Issue order is un4p → ej7y → xavt. Files overlap without commits by instruction. Preserve un4p's actual path width/tail predicate, positive inner area and reason context priority in the first PR. This PR owns ApprovalState offset/constructor migration, BodyWindow and navigation geometry, Preview's generic window/status, workflow comparison/tree projection, plan fallback marker accounting, UiState key dispatch, App's one-line dispatch plus ej7y-labelled tests, and its own route/design/plan/review decisions. Shell-specific PreviewContent/ShellBody/scanner, PreviewBody::Shell and command context extensions belong to the following xavt PR together with core. No empty type-only or consumer-only placeholder PR is intended.

## Narrow-comparison checkpoint

Main DG approved comparison-only adaptive continuation indent `min(4,width.saturating_sub(2))`; tree wrapping retains four columns. Actual renderer regression `narrow_comparison_tail_is_drawn_without_clipped_continuation_indent` covers ASCII one–six text cells and wide glyphs two–six cells. Inline `ej7y_long_comparison_references` snapshot retains 24 references beyond four rows. The 600-code-point tree fence isolates the first tree body row, rather than accepting a comparison ellipsis as a tree marker. G1 records their executed regression results; S1 adds actual narrow option/reason endpoints and zero-text-width controls.

LSP references were attempted but returned empty/timed out despite known callers; explicit constructor/caller inventory was used. This is a tool-coverage limitation, not a claimed successful semantic-reference result. The original implementation worker ran no gates; that historical restriction does not negate the subsequent parent qualification now linked above.
