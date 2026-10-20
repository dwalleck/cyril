# cyril-zj0m implementation plan

Approved inputs: design.md Approval (explicit user's first-Diff-for-both decision and DG lazy derivation approval, 2026-10-19). Parent owns all verification/review/PR publication; workers do not run build/test/lint/formatter/smoke or commit/push.

## Slice 1: coherent retained diff view

**Outcome:** C1–C3: one lazy theme-independent derivation supplies exact first-Diff counts and bounded rows; unchanged source and metadata-only/status updates reuse it; content replacement invalidates coherently, including cloned snapshots and pending approval previews.

**Changes:** Private `tool_diff.rs` owns derivation and compact source-range rows with existing one-context/20-row/ellipsis policy. `TrackedToolCall` owns OnceLock<Arc<DiffPreview>> and narrowly invalidates on effective selected old/new source replacement; no copied source body. Chat deletes duplicate summary/Myers work and renders one shared result; approval preserves the same real diff adapter with explicit borrowed source lifetimes. `lib.rs` wires the private module. Preserve separate fiv1 error-message behavior; no state/merge repair duplication. Behavioral tests cover exact counts/rows, unchanged first Diff + changed second, no diff/no change, old absence, truncation, clone/update/theme coherence and reuse sensitivity.

**Verification:** Parent runs focused diff/tracked-wrapper/chat/approval tests with actual pre-change RED and post-change GREEN, full UI and affected workspace gates, real notification→App draw/approval smoke, post-smoke docs/changelog, and independent review. No second diff oracle or timing threshold.

**Dependencies:** x5xi concrete style identity edits are present; parent verifies assembled source. The first-Diff decision is settled. Reviewed1v25/fiv1 commits are required before c6la state migration, but not before these distinct cache/accessor hunks.

## Slice 2: repeated production-shaped Write benchmark

**Outcome:** C4: reproducible cold/warm full-root rendering workload exists, exercising actual completed Write source and the coherent cached derivation.

**Changes:** Add the dependency-free `harness = false` `conversation_render` target in cyril-ui and a completed-write case using UiState/typed notifications/root draw/TestBackend. Fixtures and warmup are outside steady samples; report cold cost, raw samples, median/p95, checked environment. Later c6la extends the same target and migrates root render state atomically rather than replacing this issue's qualification.

**Verification:** Parent runs `env -u CARGO_TARGET_DIR cargo bench -p cyril-ui --bench conversation_render -- completed-write`, with the design's workload/warmup/sampling protocol, plus deterministic no-rederivation consumer coverage. Timings are report-only until calibrated requirements exist.

**Dependencies:** Slice 1; benchmark is authored concurrently in disjoint files but final assembly waits for production edits.

## Delivery

One substantive issue-specific PR boundary includes both slices, consumer migration, required tests, benchmark and post-smoke documentation. No scope, limits, fidelity or cache policy changes outside design. No invented feature budget. Parent records actual checked states/results and resolves all required evidence before issue closure/publication.

## Retained baseline RED protocols

### First-Diff semantic regression

This exact test uses only pre-existing production types and the existing `rendered_tool_lines` test helper, so it can be added inside the original chat.rs test module without the new cache implementation:

```rust
#[test]
fn baseline_first_unchanged_diff_does_not_show_later_counts() {
    use cyril_core::types::*;
    let tc = TrackedToolCall::new(
        ToolCall::new(
            ToolCallId::new("first-diff-coherence"),
            "Edit".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![
            ToolCallContent::Diff {
                path: "first.rs".into(),
                old_text: Some("same\n".into()),
                new_text: "same\n".into(),
            },
            ToolCallContent::Diff {
                path: "later.rs".into(),
                old_text: Some("old\n".into()),
                new_text: "changed\nextra\n".into(),
            },
        ]),
    );
    let theme = crate::traits::test_support::marker_theme();
    let lines = rendered_tool_lines(&tc, &theme);
    assert_eq!(
        lines.iter().map(Line::to_string).collect::<Vec<_>>(),
        ["✓ Edit(first.rs)"],
    );
}
```

The independent original-base sensitivity run executed the equivalent first-Diff probe and required a real assertion RED, then GREEN; [verification.md](verification.md) records its exact pinned patch boundary. Permanent `render_tool_call_unchanged_first_diff_hides_later_changes` passed in the current qualified workspace. This illustrative protocol is not a claim that its suggested filtered command ran separately.

### Actual duplicate/repeated Myers work

The authored root consumer test `completed_write_root_draw_reuses_diff_after_notifications_resize_and_theme_change` uses the real notification/root draw path and passed in the actual-tree workspace run. Historical independent probes counted immediately at both actual baseline `TextDiff::from_lines` calls and at the retained implementation's single constructor. Cold `2→1`, ten warm draws `20→0`, warm plus resize/theme `24→0` and replacement `2→1` are original-base sensitivity results, not re-labeled instrumented execution of the current assembled tree. [verification.md](verification.md) owns the receipt and applicability limits.

### Benchmark commands

The completed-write command ran all three declared controls and retained cold plus 100 warmup draws and 30×100 measured draws per workload. The unknown-mode rejection was exercised later by Main's explicit authorization against a private, newly hashed copy of the exact existing executable resolved from the completed-write log; it was not another Cargo/build/measurement run. [verification.md](verification.md) owns both separate receipts. Timings have no invented pass threshold; TestBackend excludes terminal I/O and sequential cases explicitly share process caches.

## Suggested focused commands and actual assembled checkpoint

The following focused command list remains useful for deliberate reproduction. These commands were not run separately; the qualified `cargo test --workspace` executed their permanent tests without filters:

```sh
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib tool_diff::tests
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib traits::tests::tracked_diff_cache
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::chat::tests::render_tool_call_
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::chat::tests::completed_write_root_draw_reuses_diff_after_notifications_resize_and_theme_change -- --exact
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::chat::tests::pending_approval_diff_preview_reuses_derivation_and_current_colors -- --exact
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::approval::tests
```

The full actual-tree workspace run passed, including the exact root and pending-approval regressions above; it is not a zero-test or compile-only result. The original pending fixture repair is independently reviewed and its test runtime is present in the current workspace log.

The authorized actual-App smoke uses an owned ACP peer and the private tested binary, rather than depending on a live provider choosing a particular tool payload. It observes exact first-Diff counts/body/gutters, unchanged-first suppression, replacement, width/palette/offscreen/back coherence, pending ledger comparison, RejectOnly wire response and turn completion. This supplies actual App/bridge/terminal-event integration evidence, not native graphical-terminal glyph or 50,000-message qualification. The instrument changes no product behavior, coverage, scope or architecture.

Both implementation slices and README documentation are delivered for exact qualified tree `6241931fecb663e9bb17e5cc0e76c35c12833828` at HEAD `6ab6d77e03b73a3a45d6d526fbc19641dace5513`. [verification.md](verification.md) is the single owning evidence index for gates, benchmark, CLI rejection, review, consumer and historical failures. Main owns this issue's commit/PR/CI/tracker closure and any later output-alias/stack refresh; no such later source is prequalified here.
