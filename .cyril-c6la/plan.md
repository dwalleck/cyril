# cyril-c6la implementation plan

Approved design: design.md Approval, DG by Main 2026-10-19; strict indexed visible-row path, exact pinned Ratatui cell/style fidelity, source attribution, no generic indexing framework. zj0m first-Diff behavior is explicitly user-approved in its design. No check/format/build/benchmark/smoke/commit/push runs in workers; parent owns integration verification/review/PRs.

## Slice 1: isolated exact row-layout and cumulative-height primitives

**Outcome:** C1/C2 groundwork through real concrete layout interfaces: physical rows retain source ranges, exact pinned Paragraph wrapping is preserved, row selection does not walk hidden prefixes, and heights support local changes/pruning without source reprocessing.

**Changes:** Private `widgets/chat_layout/wrap.rs` implements the minimal source-derived pinned WordWrapper behavior with license notice; `index.rs` implements a concrete vector-backed physical-row height index. Input is borrowed styled source spans with source slot/byte offset; output is compact row-run ranges/styles/width/alignment, not cloned text or persistent per-grapheme records. Index owns only height metadata and amortized front-compaction; no MessageHistory/core/event dependency. Author differential Paragraph/Unicode/whitespace/width/style tests and point/append/drop-prefix/deep seek tests through these concrete interfaces. Do not wire into production root before settled assembly.

**Verification:** Parent exact Buffer cell/style/reference comparison, row counts/arbitrary windows and index behavior tests; subsequent full-root consumer work assertions. Mere unreferenced modules do not complete this slice's final assembled acceptance.

**Dependencies:** Isolated modules are authored. Reviewed1v25 is integrated as `1ee1d92f` from approved `a5d2c4b5`; Main now authorizes full Slice 2 without waiting for fiv1's distinct error_message integration.

## Slice 2: complete concrete history revision migration

**Outcome:** C3/C4: every display-changing main/child mutation is observable without scanning all history; stable tool positions survive prune/clear/adoption and preserve reviewed missing-result behavior.

**Changes:** New MessageHistory owner, coalesced latest-revision metadata, stable positions and read-only history/live revision interface. Migrate UiState, SubagentStream, workflow storage/adoption, TuiState and every caller/mock/fixture. Remove obsolete messages_version path; tool-ID maps resolve stable positions and validate IDs. Preserve reviewed1v25/fiv1 lifecycle/error-text/consumer fences and update their cache invalidation calls instead of duplicating merge logic.

**Verification:** Parent state/child/workflow/peer consumer tests; old-tool/receipt edits, multiple changes per frame, stale evicted ID vs retained B, clear/reused IDs and adoption; changed-block work counters after assembly.

**Dependencies:** Reviewed1v25 is integrated as `1ee1d92f`; exact reviewed fiv1 `7eefe275` is integrated as `75aaa6f1`. Parent explicitly authorized full c6la migration. Dirty history/cache source was protected and reapplied without conflict; no unreviewed peer repairs are copied. zj0m's saved source patch remains frozen while c6la migrates the live consumers.

## Slice 3: retained renderer and all caller migration

**Outcome:** C1–C5: warm rendering paints only indexed visible source runs; source parsing/hashing/wrapping/derivation occurs only for changed blocks; main/child/stream/scroll/width/theme behavior remains correct.

**Changes:** App-owned concrete RenderState, root/chat signatures and every test/benchmark caller migrated atomically. Shared Arc Markdown result and complete borrower migration; cache outputs remain immutable. Chat owns presentation-source selectors/generated text and transcript selection; layout owns prepared rows/heights/visible painting. Keep source strings in authoritative history/live state or shared Markdown backing, never self-referential pointers or copied plain/diff bodies. Separate live/header/activity blocks. Drop retired prepared blocks; metadata-only compaction does not rederive source. Panic fallback invalidates partially updated render state.

**Verification:** Parent focused/full UI/binary/workspace gates; existing frozen/floor/overlay/chronology/theme fixtures, focused-child behavior, >65,535 rows, giant line/message direct seek, 1,000 live deltas and invalidation work counters. Exact reference cell/style fidelity is mandatory, not an approved deviation.

**Dependencies:** Slices 1–2 plus zj0m; parent-supplied prerequisite integration settled. All root render and TuiState consumers migrate together, no compatibility shim.

## Slice 4: long-history qualification and documentation

**Outcome:** C6 and assembled performance acceptance: production-shaped 100/1k/10k/50k histories, repeated Writes, giant message/line and live/mutation/invalidation cases have deterministic work evidence, practical benchmark receipts and real App/native smoke.

**Changes:** Extend existing conversation_render benchmark; add consumer behavior/work tests using real UiState and retained renderer, keep fixtures outside timing and assert actual retention counts. After parent smoke proof, update existing architecture/changelog docs at discovered owners. No fabricated time budget or new history cap.

**Verification:** Parent runs declared release sampling protocol and records source/environment/raw samples/median/p95/cold costs/storage counts, then native App scroll/resize/theme/focus/stream smoke. Parent runs required workspace gates, obtains independent review, resolves findings and opens c6la's own substantive PR.

**Dependencies:** Assembled Slice 3; no proof/issue completion claimed from isolated modules or an unexecuted benchmark.

## Delivery and scope

One c6la PR after x5xi and zj0m; internal slices preserve atomic caller/interface migration. Parent owns prerequisite integration and issue/PR boundaries. No universal feature/line budget, fidelity relaxation or generic indexing framework is authorized. Main explicitly approved the existing locked unicode-segmentation1.12.0 direct UI edge, with shared workspace default-features=false and no package/version change. The requester explicitly approved recursive widget cap22 here/cap23 with MCP, unchanged aggregate byte cap/prohibitions, and only the documented existing Windows baseline exception. No other risk waiver or acceptance narrowing is authorized.

## Exact painter integration constraints from pinned source

The row-layout worker identified pinned trim=false behavior that final assembly must preserve: a terminal-width-3 row can end at width 4 (for example a final `界界`); retain the actual row width and use Paragraph's alignment formula `(area.width / 2).saturating_sub(row.width / 2)` for centered rows. The pinned Paragraph painter writes each nonzero-width grapheme's start cell and does not reset wide continuation cells. Final painting must use the same source-run styled-grapheme/cell-write semantics, not `set_stringn`, Text/Line or an unwrapped Paragraph sink that changes overhang/continuation handling. Keep input-span boundaries as distinct row runs even for identical source/style, because joining them can resegment combining/RI/ZWJ sequences. These are existing approved fidelity obligations, not authorized visual corrections. Parent compares exact reference cells/styles; no such tests have run yet.

## Authored qualification / parent execution

The actual consumer module is `crates/cyril-ui/src/conversation_performance_tests.rs`, registered cfg(test) in lib.rs. Fourteen unexecuted tests cover C1–C5: identical visible suffix at 100/1k/10k/50k retained histories; exact first/middle/deep/follow/rows-back windows and >65,535 rows; multi-megabyte Markdown and giant unbroken line; old tool/receipt coalescing; append/prune/metadata compaction/clear/reload/reused IDs; 1,000 live deltas and independent thought/input/spinner; focus/completion/removal/adoption; width/theme invalidation versus height-only reuse; actual three-row chat floor; one real Myers per coalesced old Write source; focused child deep-follow; and storage-only workflow adoption with valid both-side tool indices. Existing frozen/Paragraph-reference fixtures remain unchanged independent fidelity controls.

Production benchmark `conversation_render` retains `completed-write` and adds `long-history`. Both use real UiState, typed notifications, retained RenderState, public root draw and TestBackend. Long-history fixtures assert exact retained counts, exceed the 256-entry Markdown cache, and use constant retained padding plus identical five-message source controls so earlier unique history cannot leak into the compared viewport. They exercise oldest browsing, old Execute and receipt edits, 1,000 live deltas, thought, append/prune, child/main focus, width 160→80→160, height 48→12→48, palettes and no-color. Giant fixtures include multi-megabyte Markdown and an approximately 12.8 MB unbroken line with top/middle/bottom/source-row-70,000 windows. No new latency budget or retention cap is used.

Every measured stable case has 100 warmups and 30×100 draws. Cold/transition cost and measured mutation cost are separate; unavailable mutation timing is printed `not_measured`, never zero. Fixture construction, positive-control validation and metrics reads stay outside frame timing. `owned_text_bytes` counts retained generated String lengths, the child-name key and owned Markdown span bytes, deduplicating identical shared Arc allocations transiently within this explicit metrics call. Borrowed/static bytes are excluded. It is NOT allocator capacity, descriptor bytes, process-wide ownership or RSS; row/run/source-slot counts make the descriptor tradeoff visible without a fabricated memory figure. No retained registry is introduced.

Parent commands, serialized only after assembled source is frozen:

```sh
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib conversation_performance_tests:: -- --test-threads=1 --nocapture
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::chat_layout::wrap::tests -- --test-threads=1 --nocapture
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib widgets::chat_layout::index::tests -- --test-threads=1 --nocapture
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib presentation::tests:: -- --test-threads=1 --nocapture
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib tool_text_tests:: -- --nocapture
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --test missing_tool_updates
env -u CARGO_TARGET_DIR cargo test -p cyril-ui --test v2_failed_validation
env -u CARGO_TARGET_DIR cargo test -p cyril-ui -- --test-threads=1
env -u CARGO_TARGET_DIR cargo test -p cyril
env -u CARGO_TARGET_DIR cargo check --workspace --all-targets
env -u CARGO_TARGET_DIR cargo clippy --workspace --all-targets -- -D warnings
env -u CARGO_TARGET_DIR cargo fmt --all -- --check
env -u CARGO_TARGET_DIR cargo bench -p cyril-ui --bench conversation_render -- completed-write
env -u CARGO_TARGET_DIR cargo bench -p cyril-ui --bench conversation_render -- long-history
```

CLI qualification: `--help` succeeds; `unknown-mode`, `completed-write completed-write`, and `completed-write --unexpected` must each exit nonzero before measurements. Record source pin, CPU, rustc -Vv, target/profile/features and exact invocation with real output. All commands remain PENDING with Main; no worker result is inferred.

### Portable pre-c6la actual-work RED

The two `c1_portable_baseline_*` tests in the consumer module avoid history/layout-owner APIs. Parent transplants those tests plus `portable_operations`, `portable_unchanged`, fixture helpers/constants and Root into the pinned unchanged pre-c6la root. Adapt ONLY Root's old/new draw API and retained-state field; preserve all inputs, controls and assertions. Instrument actual baseline historical render_message, complete Markdown source hashing, parser invocation, Paragraph line_count/wrapped rendering and real Myers calls. Merely configuring `.wrap(...)` is not a performed wrap; no fake zero row/index counters supply baseline evidence.

Run these exact suffixes individually with `-- --exact --nocapture --test-threads=1`:

- `conversation_performance_tests::c1_portable_baseline_unchanged_root_draw_does_not_reprepare_history`
- `conversation_performance_tests::c1_portable_baseline_giant_middle_window_does_not_scan_hidden_prefix`

Each invocation must execute one test and emit `C6LA_BASELINE_CONTROLS_EXECUTED`. Qualifying RED occurs only at the final unchanged-operation assertion AFTER correct visible output and cold-operation controls succeed. Compile errors, zero tests, wrong/missing content, root fallback or failed cold controls are not RED. Giant portable selection stays below the old u16 ceiling so work, rather than an unrelated wrong-output ceiling, causes this RED. Retain actual baseline and integrated GREEN receipts; x5xi/zj0m's earlier six-probe receipt cannot discharge these c6la obligations.

## Frozen source / current approval gates

All four source slices are authored and owners have frozen their edits; route.md pins the refreshed461128-byte/32-file c6la-only patch, predecessor tree and prerequisites. No worker executed verification. Source includes F1 production-boundary, F2 qualification-input, F3 complete-recursive-inventory, F4 allocation-free-boundary-detection and F5 source_text naming corrections; executed proof and independent review remain with Main.

Both DG points are resolved: Main approved necessary crossing-grapheme derivation, and the requester approved a single fully recursive widget inventory cap22 (23 only with MCP), not a split17 subgroup. F3/F4 and the exact approved existing1.12.0 manifest/lock edge are included in the refreshed c6la artifact; frozen x5xi/zj0m evidence boundaries remain unchanged. Parent owns all commands above, portable RED/GREEN, uninstrumented source-gate/mutation proof, benchmark/native smoke, permanent docs/changelog, independent review and the separate c6la PR. The requester-accepted cyril-u62f Windows baseline readiness/cancelled-coverage limit does not discharge any other proof or new failure.

### Parent-owned actual source-gate sensitivity

Run `python .cyril-c6la/gate-review-f3.py --target-dir /nonempty/owned/target` on this exact22-source boundary before combined MCP integration. The harness reconstructs the actual pre-F3 guard from reviewed fiv1 plus hash-pinned x5xi/zj0m patches, swaps only the guard function and keeps F1's splitter/current source identical. Fresh cases require unchanged both PASS; forbidden nested production and unlisted nested source BEFORE false-PASS/AFTER FAIL; forbidden existing production both FAIL. Retain hashes/full logs and one-test receipts; no compiler failure or zero-test result qualifies. F1's separate old/new production_source helper proof remains required. No harness or Cargo invocation ran in this worker.

F5 mechanically names the new private source getter/painter callback `source_text` so the unchanged broad `resolve(` prohibition does not reject legitimate source access. All scoped source/test callers migrate, without a theme-policy exception. No formatting qualification is claimed: Main owns formatting the assembled touched code, refreshing the final source identity and then executing the declared check/proof commands; worker did not invoke cargo fmt or a formatter.

## Independent review: tests-only checkpoint before production authorization

The product baseline remains production.patch SHA256 `b84fb6d259ad446bba113de49b08a103b3aa8d0f9b1ae06fbcc63e9c7331f8fa`, reconstructed tree `51dd940ecd4e033b9eeb4e967ea2199596a1d378` after the reviewed prerequisites/x5xi/zj0m. Do not overwrite that baseline or apply product fixes before parent RED. The independent Layout P2 and Root P1/P2 findings are accepted; History corroborated the root findings without duplicates/additional defects.

The authorized setup correction changes only five original receipt binding/marker literals to `steer-older-receipt`; it preserves all fourteen original tests and benchmark controls. It is not performance RED. Actual-loop cfg(test) diff selection counting is added in traits.rs; no production algorithm/cache/state/parser behavior is repaired.

### Exact new filters and intended outcomes

Run all five with `env -u CARGO_TARGET_DIR cargo test -p cyril-ui --lib conversation_performance_tests::review_ -- --nocapture --test-threads=1`, or use each exact suffix under `conversation_performance_tests::`:

- `review_hidden_diff_blocks_are_not_reselected_by_unchanged_root_draws`: real completed Write, identical visible buffers/styles, 0/256/4096 hidden Text blocks, 100 warm draws each. All three cold/visible/paint controls and printed actual visit counts precede target zero-warm-selection/derivation assertions. Intended RED is warm_selection, not missing output or cold work.
- `review_complete_receipt_keeps_large_live_markdown_prepared`: warmed >256KiB/4096-paragraph live Markdown, valid bound complete receipt, exact unchanged displayed bytes, Applied note/queue/cell/style controls. Intended RED is prepared_live/hash work; only one old-chip preparation/wrap is legitimate.
- `review_chunked_receipt_opening_keeps_large_live_markdown_prepared`: ten valid incomplete opening/id/note chunks, then completion; unchanged source/Queued/withheld/live-style controls at every partial state. All states execute before target no-derivation assertions. Completion allows only changed-chip work.
- `review_chunked_receipt_body_keeps_large_live_markdown_prepared`: five valid incomplete body chunks, then completion, with the same controls/work distinction.
- `review_nested_receipt_visible_prefix_semantics`: positive standing, NOT RED. Preserve same-length/different-source nested-note harvesting and earlier-prefix exposure that becomes completely withheld. Require correct cells and actual changed-live work, then unchanged warm reuse.

Receipt fixtures pin activity elapsed at three seconds with the existing test hook, without sleeps. Hidden-Diff fixtures explicitly enter Idle before the cold draw and stay unchanged throughout all100 warm draws, so no clock-bearing chat activity exists. Require executed test counts and C6LA_REVIEW_CONTROLS_EXECUTED; fixture/protocol/cold-control failures cannot qualify. Parent owns all executions and any resulting test-only compile/setup correction.

### Owner repair proposal retained from the pre-RED checkpoint

**F6 / presentation plus existing diff-source owner:** capture the first Diff content ordinal during changed-block preparation, retain it together with row ordinal in Source::DiffRow, and resolve current content().get(ordinal) directly at paint. Preserve first-Diff semantics, current-source ranges, no clones and equivalent-source preview reuse. Prefer one concrete indexed selection adapter on TrackedToolCall over a new cache/provider abstraction; the actual visited-item hook must remain on any real linear selection after factoring, never disappear by renaming. No change to approval's completion-independent diff adapter.

**F8 / UiState append-harvest owner, not renderer:** cache the displayed cutoff and make the getter a direct borrow. Replace independent raw-delta/receipt-count bumps with a displayed-source decision after existing append/harvest. The existing parser may expose first-removed-start metadata alongside ordered receipts, with recognition/removal unchanged; migrate all actual callers/tests after LSP/reference inventory and explicit approval. Compare displayed lengths OR deletion touching prior-visible prefix to prevent stale same-length content. Recompute legacy withholding with sufficient context (or reuse the cutoff when metadata proves the entire raw buffer unchanged); do not blindly scan only the old-visible suffix. Maintain every constructor/append/harvest/raw-take metadata path, preserve raw flush/receipt order/id-first fallback/queue effects and child-stream semantics.

This invalidation is conservative for deletion/reappend-identical-content, not an exact content-equality claim. It avoids cloning/comparing/hashing the large live body to detect change, but does NOT eliminate legacy harvest's existing whole-buffer search; a zero-whole-body-decoder-read guarantee would require separately approved parser progress. A displayed-length-only patch is unsafe: `A[STEERING steer-older-receipt: [x` followed by `]` and enough z bytes changes content without length change. Deleting an inner receipt from `[STEER[` can expose an earlier `[STEER` before the old cutoff. Parent artifact605 executes and passes both semantic controls before approving this repair.

**Historical gate, now satisfied:** Main retained genuine F6/F8 RED in artifact605 and explicitly authorized the bounded owner repair below. That is not blanket permission for other production changes. All original repository/behavior/work/source fences, benchmark/native smoke, final independent re-review and issue PR remain required.

### Exact tests-only artifact

Retained pre-repair `review-test-only.patch`: 22116 bytes, three files, SHA256 `9b7aeba7f91b651247bb18659517bab082e1766a4f7e8e8705b05af19a9dec1a`; applies after b84/tree51dd940ecd4e033b9eeb4e967ea2199596a1d378. It contains only actual cfg(test) predicate instrumentation, five new tests and five valid-ID fixture corrections. Hidden-Diff fixtures are Idle and receipt fixtures pin elapsed3s. Main subsequently executed the intended four work failures/one positive in artifact605 and explicitly authorized the bounded product repair; no worker execution is claimed.

## Observed review RED and explicit production GO

Main executed the five-review filter on product b84 plus test-only9b7 in the original linked worktree. Retained [artifact605](artifact://605) shows five executed tests, four intended operation failures and the nested-prefix semantic positive PASS. All visible/style/source/status/cold controls executed before target failures. This is actual parent evidence, not a worker run or a compile/fixture/zero-test RED.

Observed 100-draw hidden-Diff visits: 200 / 51400 / 819400 for 0 / 256 / 4096 hidden blocks; identical visible buffers/styles and zero other warm derivation passed. Valid complete/withheld receipt source sizes were 413773–413777 bytes; unchanged chunks still prepared live1, hashed1 and wrapped2, while complete receipt also did its legitimate one-chip work (wraps3 total). Nested equal-length changed-content and earlier-prefix exposure controls passed.

**Main explicitly issued PRODUCTION REPAIR/DG GO** after that RED: capture selected Diff ordinal during preparation/direct indexed paint, preserve first-Diff and equivalent-source cache; cache visible cutoff/revision in UiState using conservative parser removal metadata, preserving recognition/deletion/raw flush/chronology/child semantics. Earliest affected offset must conservatively min across ALL removals. No body clone/hash/equality scan, per-frame getter rescan, length-only/suffix-only shortcut, new framework or public UI owner is authorized. Legacy harvest's existing whole-buffer scan is explicitly outside the eliminated-work claim. Exported-function references/caller migration are required before API edits, with no lazy compatibility fallback.

The two disjoint approved source slices are traits.rs/presentation.rs indexed Diff ownership and Core receipt metadata/state.rs visible-prefix ownership. Frozen five review tests and original behavior/work/source fences remain unchanged. Parent owns post-repair GREEN, full qualification and independent re-review; no worker gate is authorized.

## Frozen post-review repair handoff

F6/F8 and F9 implementation and scoped proof retain their historical boundaries. The common prerequisite merge, initial final format and first Clippy failure are preserved separately. Bounded F10 repair is complete; Main subsequently reports current-source workspace/all-target Clippy GREEN and independent repair review correct .99. The next full tests at formatted tree830b4673 fail three F11 reference-wrapping cases. Route.md owns exact current/historical identities and review-decisions.md owns actual receipts and the reserved design gate.

Historical production-after-f9.patch and prior b84/9b7/f51/095/F9 artifacts remain unchanged for their original source/proof boundaries; they are not the current formatted/prerequisite-qualified assembly. Use the separate review-clippy-repair.patch only after the exact parent post-format tree, never reapply it to the already-edited original worktree. Parent private-index/manifests preserve pre/post-format provenance. No auth fixture or frozen harness/snapshot is overwritten, and no historical feature patch is regenerated.

F10 preserves live source identity, dirty-only borrowed retrieval, range/invariant failures, source-affix bytes and header/label/body ordering; all nine production expect calls are replaced without allocations/defaults/new scans. The cold-control comparison and three periodic predicates are equivalent mechanical lint changes, not narrowed workloads. Main owns remaining full qualification, benchmarks/native consumer and issue PR. Retained bg116 targeted GREEN/F9 proof is not relabeled as current full qualification.

Common auth fixture and later PR183 alias stack propagation remain separately parent-owned. No actual zj0m edits were made. Source ready: seven-file bounded F10 delta and JSON identity are in the current route handoff; no worker Cargo/formatter/build/test/bench/smoke was run. Edit-tool client-normalization warnings were reported and actual saved bytes exported, without invoking a formatter.

## Approved F11/F12 affected repair plan

**F11 outcome (C2):** correct only carried-prefix unsafe positive starts, preserve every defined reference comparison and the legal final-wide overhang. The requester-selected narrow parity exception is recorded once in design.md F11; no vendor/dependency change is selected.

**F11 changes:** private wrap.rs root carry normalization and bounded original-reference classifier/test policy; presentation.rs generated-affix comparison follows the same exact exception. Existing fixtures remain. Literal normative rows/cells plus content/style/bounds checks replace ONLY the undefined reference comparisons. No painter clipping, padded oracle, blanket panic catch or broad upstream semantics.

**F11 verification — Main:** retain original tree830b4673 actual RED, then qualify unchanged original workloads with the approved comparison and new literal/safe-start/classifier controls, exact safe Paragraph/affix/overhang/deep-window cases and affected/full repository gates. New root-correction fence must demonstrate sensitivity to removal of the carry repair without disabling classification or assertions; do not claim the old reference-first panic alone proves the new normative fence. Fresh source review precedes parent qualification; final repair review follows applicable checks.

**F12 outcome (C1/C2/C5):** reserve a geometry-only fixed right1 scrollbar gutter for outer width>1; width0/1 retains full content without indicator. Offset/follow/overflow/focus never changes content width or causes hidden-history wrapping.

**F12 changes:** retained.rs uses identical contentRect for all synchronization/live/activity/preparation/painting and separate gutter for the thumb. Root consumer tests in conversation_performance_tests.rs retain full opaque URL bytes at bottom/browse, actual zero-derivation warm-scroll controls and overflow/tiny-width boundaries. conversation_render.rs pads its80,000 unbroken segments to content width rather than outer width, retaining all literal markers/old-scroll-ceiling checks and unchanged timing/backend geometry. No captured snapshot/TSV change is justified by the affected-fixture census. No root geometry or URL/protocol/auth changes.

**F12 verification — Main:** actual C6 full-URL regression must fail with old overlaid geometry and pass with the fixed gutter; counters forbid unchanged-source preparation/hash/parse/wrap/Myers on PageUp/offset/overflow transitions. Run affected/full tests, width/theme/source-policy gates, production-shaped benchmark and remaining smoke. Tqr's independent actual controlled native RED supports the diagnosed class, not a substitute C6 test receipt.

These are disjoint existing-owner repairs within the same C6 issue/PR, not new issue placeholders or invented budgets. Starting full-workspace state is RED; do not commit or claim an independently green intermediate assembly. Parent owns all checks, combined final checkpoint/review, post-smoke permanent docs/changelog and publication. User-native Kiro checks remain separately paused, not waived. Historical artifact boundaries remain immutable.

## F11/F12 source delivery checkpoint — qualification pending

Both approved repairs and their affected test/benchmark consumers are implemented; five-file frozen delta14945de1 after830b4673 reconstructs43db6bd5. Route.md and carry-gutter-repair.source.json own exact identities; original worktree already contains the source and all historical artifacts remain unchanged. Disjoint independent pre-qualification source reviews report correct/no findings.93 each. No worker checks, formatting, commits or publication.

Review-decisions.md owns the current delivered behavior, new test names, source-review scope and precise required RED/restored-GREEN mutations. Main now owns actual format/Clippy, affected/full/source-policy/build, sensitivity, benchmark/remaining smoke and post-check repair review. The previous830b4673 Clippy/three-test RED is not relabeled current proof. No dependency/vendor change, universal budget, snapshot regeneration or native-Kiro waiver is introduced. Existing C6 issue/PR completion still requires those gates and post-smoke permanent docs/changelog.
