# cyril-xavt actual delivery handoff

## Delivery identity and qualification boundary

- Delivery worktree: `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt`.
- Delivery branch: `fix/cyril-xavt`, newly created on the published EJ base.
- Published EJ base: `9400e7c8ddd53776f6fd8b4d28d4c7fc69e45ab4` (PR190), following un4p PR188, commit `2fd94879db1b4281dc85cf7dcf528649c1da3087`. The common test-only authentication prerequisite is `6c0f41c74bd94fa49ab1b3a2ae06e3ed70d3de69`; it is not an X feature file.
- X implementation remains a working-tree delta. Main owns staging, commit, tracker updates, and the issue's own PR; this handoff performs none of them.
- **Actual-delivery repository qualification PASS:** parent bg128/artifact860 completed workspace/all-target Clippy with `-D warnings`, workspace tests (3,117 passed, 13 ignored), default build and formatting checks. The qualified private binary is `/tmp/cyril-xavt-delivery-tested-20261020`, SHA-256 `33891699d8cfa71f5ce8e16fae3675b23c4e104d384a7f7d69416880ffebfb84`. Fresh actual-source ReviewShellApproval reported correct/confidence0.98, with the subsequent single-field F3 fixture correction recorded below.
- [verification.md](verification.md) owns the final gate/review/consumer receipts and their applicability boundaries. Real KAS was exercised once on the historical combined binary, not this actual-delivery binary. Its complete-command-before-Allow and successful execution-wire evidence must not be described as a clean final-model turn or rendered-stdout proof. The actual-delivery supplement uses the exact safe captured pending frames and one RejectOnce; it executes nothing.

## Frozen approved input

Read-only source: `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt-approval-previews`.

Read-only integration contract: `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt-approval-previews/.cyril-xavt/stack-handoff.md`.

Applied latest approved delta:

- Path: `/home/dwalleck/repos/cyril-wt-fix-cyril-xavt-approval-previews/.cyril-xavt/delivery-patches/03-cyril-xavt.patch`.
- Size: **125,436 bytes**.
- SHA-256: `9cfedec4145a65a367e274c569159630f59d105a3debbe933520390bd28ae662`.

The obsolete `03b` replay-support delta was not applied. No published EJ/un4p file or frozen combined-source file was modified.

## Application and reconstruction

Applied the approved patch directly in this delivery worktree with `git apply --reject`. All hunks applied except four formatting-context rejections in `crates/cyril-ui/src/widgets/approval.rs`. Reconstructed only those rejected hunks with surgical edits to the actual formatted EJ file:

1. Original hunk3, `Preview::new`: keep the actual unclamped body width, build the body using `text_width.max(1)`, make typed Shell content scrollable alongside the inherited workflow-plan case, and populate the existing new `body_width` field.
2. Original hunk6, `Preview::body_window`: pass `self.path_rows.min(self.path_budget(rows))` to the inherited `fit_path`, preserving the actual EJ formatting and body-window owner.
3. Original hunk7, `Preview::into_lines`: use that same path budget and delegate body emission to `PreviewContent::extend_lines` rather than the old Vec iterator. The surrounding actual EJ layout remains intact.
4. Original hunk15, `OptionPage::selection_shown_with_path`: retain the actual formatted inherited path predicate and append the approved current-viewport `command_shown` condition.

The rejected-hunk file was removed after these four edits; no unresolved hunks or conflict scaffolds remain. No wholesale combined-source replacement was used and no conflict required a new semantic decision.

Approved behavior remains unchanged: private `Option<Box<ShellCommand>>` storage, moving builder and borrowed getter; no ReplayFrame boxing; KAS-only projection after the existing source-aware permission-ledger join; verbatim typed command/cwd; safe grapheme-width bounded paging; wholly drawn one-meaningful-grapheme command confirmation; no cwd-only confirmation; unchanged permission authority and grant semantics.

### Main-authorized fixture-fidelity correction

After reconstruction, Main reported fresh actual-X independent review correct/confidence0.98 with no behavioral findings and authorized one source-fidelity correction before gates. Direct reading of the cited `experiments/conductor-spike/audit-2.32.0/live/kas-lead-interrupt-sig15-2.32.0.jsonl:56` confirms `params._meta.kiro.consent.triggeringResource` is exactly `echo SLEPT`, while the actual command/resource remain `sleep 45 && echo SLEPT`.

Only `without_cwd.permission._meta.kiro.consent.triggeringResource` in `crates/cyril-core/tests/fixtures/kas_permission/shell-preview-2.32.0.json` was corrected from `sleep 45 && echo SLEPT` to `echo SLEPT`. Actual raw command/cwd, production code and test assertions were not changed. The frozen patch remains the historical approved input above, not a byte-identical representation of this corrected final fixture. ApprovalVisibility acknowledged retaining this correction in any future Main-authorized exports without touching frozen source. Review decision F3 records the evidence and bounded disposition; actual-delivery gates remain parent-owned. No check, formatter or commit was run for this correction.

## Exact changed paths and actual source identity

The complete X source/test/fixture scope is the eleven paths below. These are the **historical reconstruction-time, pre-format** digests, not the final source-freeze digests and not a gate-time manifest.

The historical reconstruction-manifest SHA-256 is `f37fd6de5ea9a399bf86490fa57afb6fa3e4007e9279188054acb568b0e57209`. It hashes this code block's ASCII bytes with one LF after every line, lexicographic paths and two spaces after each digest. Parent's final formatting subsequently changed traits.rs, approval.rs and app.rs. The final eleven-file manifest is recorded separately in verification.md; it must not be backdated to assembly or to an unrecorded gate-time hash.

```text
4ddefb5006ae3af60b67fc54e0e6abba8fd61c39ef3d3f605c7cdcf4168ee865  crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/always_allow.rs
43638a819e0aef1efb4a2a78fef5fb7dae3a54c2c30b1c27266e7c0f0f3f1644  crates/cyril-core/src/protocol/bridge/tests/harness.rs
fa03cec810c04914e04256eeca725724b73db5ecd5ebe0493200d1599e44d935  crates/cyril-core/src/protocol/convert/kas.rs
ff0efe842af37d5964ad76a9c2071746adb66b3ad9f8da15865c79d6b49799e3  crates/cyril-core/src/protocol/domain_mediator/mod.rs
85cb757f0ed84189ff7eab03716cb668038d1b5607e44ae27fbe58099cda2f59  crates/cyril-core/src/types/mod.rs
8c10073c6eedd28a0eb390d4d29e7bab57a928c5250802591536753e15ea17b6  crates/cyril-core/src/types/tool_call.rs
d1c7269f08e9a8ef2ebb1e39ac488b57854d0185283c9f97f096d942df70954a  crates/cyril-core/tests/fixtures/kas_permission/shell-preview-2.32.0.json
69c00cb87f3fcbc9954dc29789946a1aa14c97aea4280ff6da14a0fa412bd963  crates/cyril-ui/src/traits.rs
b3ec509e14b89357bd8e84e1ee0ce6825e1602d5a36cc38841ca039854331b6a  crates/cyril-ui/src/widgets/approval.rs
d38fe62a05dd135dd6287d5e4618307ae03da330365ad2a898a346a222022784  crates/cyril-ui/src/widgets/workflow_plan.rs
41ae0d8aad57da99b3d8b16736f20d1610dcf669a2ce51e25cbf9b2300bfcf40  crates/cyril/src/app.rs
```

Final publication also includes README shell-approval usage and the X-owned workflow/evidence records:

- `.cyril-xavt/route.md`, `design.md`, `plan.md`, and `review-decisions.md` retain approved ownership and amendment history.
- `.cyril-xavt/delivery-handoff.md` is this reconstruction/publication boundary.
- `.cyril-xavt/verification.md` is the canonical result and provenance record.
- The portable evidence archive/index and exact publication manifest identify only explicitly selected nonsecret files; their authoritative paths and hashes are in verification.md.

No Cargo/dependency, tracker, inherited EJ/un4p record, output-alias production fix, replay-support patch, private journal, private HOME, or compiled binary belongs to this X publication delta. Main selects only the enumerated publication files; do not stage the frozen combined patch as implementation.

## Parent publication handoff

Use verification.md and the exact publication manifest for issue-scoped staging. Artifact860, not historical combined gates688, is the actual-delivery repository receipt. The post-format source freeze is distinct from the historical f37 reconstruction snapshot. The original native archive is immutable and remains labeled combined-source; its separately retained completed-frame supplement was not inside that archive.

No second native KAS execution is required or authorized by this handoff. The owned actual-delivery proof never selects Allow, never replays completion after rejection, and does not borrow the separate PR183 output-alias repair. Main retains ownership of final publication and tracker actions.
