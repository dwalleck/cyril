# cyril-c6la: persistent physical-row layout and visible-only painting

## Route and inputs

Structural; [route.md](route.md) owns requested outcomes and source-backed routing. Proposed stack order is `cyril-x5xi` → `cyril-zj0m` → `cyril-c6la`, with one substantive independently reviewed PR per issue. This design consumes structural Theme identity from x5xi and coherent tool diff views from zj0m. Neither is an excuse to omit c6la's own viewport regression, workload, benchmark, smoke or review.

Source implementation is authored under DG approval, including Main's necessary crossing-grapheme clarification and the requester-approved fully recursive widget inventory limit below. Reviewed1v25/fiv1 are integrated; no c6la build/test/benchmark/smoke evidence exists yet. No feature budget, frame-time threshold, history cap or implementer risk waiver has been invented.

## Outcomes and preserved behavior

Steady redraws must not walk, hash, clone, derive, measure, or wrap all historical content. They locate the physical-row window and paint that window. Live changes rederive only changed live blocks; committed edits rederive only changed messages. Width/theme changes and selecting a previously unprepared transcript are explicit cold/bulk-invalidation transitions, not warm-render measurements.

Preserve current cell symbols/styles and chronology, subject only to the explicitly approved F11 unsafe-reference exception and F12 stable scrollbar gutter below, including:

- Main history separators, user/agent labels, tools, plans, system/command output, steer status/notes, live Markdown, live thought and activity/spinner.
- Main follow mode and `Some(back)` as physical rows back from the bottom, **not a newly invented fixed top anchor**. Preserve oversized-back clamp and input-submit reset; F12 reserves a fixed rightmost scrollbar gutter whenever outer width exceeds1.
- Use the actual chat rectangle from `frame_rows`/`frame_areas`, including three-row chat floor, growing input, crew/voice and autocomplete. Height-only changes select another window without rewrapping history.
- Focused child header/name plus child history/live agent text; continue bottom-follow and ignore main scroll-back. Workflow streams remain storage-only. Focus clearing/adoption must not leak the previous transcript's layout.
- Select rows in `usize` before drawing, removing the existing `u16` Paragraph-scroll ceiling. A valid bottom or browse position beyond 65,535 physical rows must display the right content, not silently clamp to an earlier region.
- Width zero/height zero and empty histories render safely without losing later invalidation.

## Actual interface changes

### 1. Concrete history owner records the information only mutators know

Add a concrete UI-owned `MessageHistory` module; do not put layout or ratatui types in core. It owns the existing message Vec, immutable-history identity/epoch, stable monotonic message positions, revision metadata, and the latest edit revision for each retained message. It exposes borrowed source and borrowed changes; no raw mutable Vec/slice escapes.

Illustrative public read interface (final Rust spelling may follow existing visibility conventions without changing meaning):

```rust
impl MessageHistory {
    pub fn messages(&self) -> &[ChatMessage];
    pub fn revision(&self) -> HistoryRevision;
    pub fn changed_since(&self, revision: HistoryRevision)
        -> HistoryChanges<'_>;
}
trait TuiState {
    fn history(&self) -> &MessageHistory; // replaces messages + messages_version
    fn live_revision(&self) -> LiveRevision;
    // existing read-only live text/thought and other frame inputs remain
}
```

`HistoryChanges` distinguishes a different identity/epoch (reset) from incremental changes, and exposes current retained start/end plus a borrowed iterator of changed stable message positions. It does not allocate a copied change Vec. Implementation uses a `BTreeMap<revision, stable_position>` containing **only the latest edit per retained message**, plus parallel last-revision metadata to remove a replaced entry. Consequently rapid updates before a frame coalesce, history mutation discovery costs `O(log N + K)`, metadata is bounded by retained messages, and there is no arbitrary journal overflow that suddenly forces an avoidable history rescan. Prefix pruning removes only retired metadata; positions of retained messages remain stable. Clear/reorder/prepend-adoption changes epoch and rebuilds from the new authoritative order once.

Mutation methods own append/edit/clear/drop-prefix/adopt and revision bookkeeping. `UiState` and `SubagentStream` retain domain notification/merge/receipt policy and tool-ID lookup responsibility, but lookup values migrate from movable Vec indices to the history's stable message positions. Drop-prefix retires removed IDs, clear empties the index, and adoption rebuilds/remaps lookup values for the new epoch. A tool update must verify that the resolved retained message has the requested tool ID before mutating it. Retaining the old bare Vec-index map would be incorrect after pruning: current source drains without rebasing (`state.rs:3545-3549`) and clears messages without clearing the index (`:1862-1865`). Preserve the peer's repair of this behavior rather than resurrecting it during migration.

Main's 17 and child's six logical mutation sites are inventoried in route.md. ID-only steer binding need not invalidate display; rendered status/note/tool edits must. Live text/thought revisions are bumped by their real mutation/flush/reset sites, independently of committed history, so unchanged spinner/input redraws do not rehash the live source. A changed Markdown live block may need full reparse because closing fences/tables can change prior structure; that work is proportional to the changed block, never prior committed history.

Migrate every old trait/accessor consumer and fixture to `history().messages()`; remove obsolete `messages_version` state/method/increments, not a compatibility shim. `SubagentStream` shares the same history owner, including workflow storage/adoption. Peer MissingToolResults' unknown-update/index/clear/prune behavior must survive this migration; its version invalidations become equivalent history-edit records, not dropped fixes. Existing read-only `TuiState` semantics remain intact.

The prune/clear regression must distinguish wrong-row mutation from merely missing an update: retain tool B after evicting A, apply a late A update, and prove B's ID/content/status and prepared rows stay unchanged while A follows the approved unknown-update policy. Then update B and prove its source/visible rows change exactly once. Also cover clear followed by reused IDs and occupied adoption. This is required consumer correctness, not an optional cache test.

### 2. Persistent render state belongs beside UiState, not inside it

Add exported concrete `render::RenderState` with private chat-layout state. `App` owns one instance separately from `UiState`. Change:

```rust
render::draw(frame, &dyn TuiState, &mut RenderState)
chat::render(frame, area, &dyn TuiState, &Theme, &mut ChatRenderState)
```

`App::draw_frame` is the only production root caller (`app.rs:946-974`) and already has `&mut self`; it passes immutable UI state and mutable rendering state separately. Migrate all test callers atomically. No App rendering logic, mutable TuiState, hidden global per-app cache, or generic renderer strategy trait is introduced. Root panic fallback invalidates any partially updated render state before a future draw can reuse it.

The chat state retains **only the selected transcript's** prepared layout, not an unbounded map of every focused stream. History identity/epoch prevents collisions across streams, resets, distinct UiState fixtures and storage moves. Switching away/back can be a cold layout transition; process-global parsed-Markdown sharing still applies where useful.

### 3. Prepare immutable blocks once; never deep-clone Markdown hits

Change the Markdown cache and its real public result to shared immutable `Arc<[Line<'static>]>` (or equivalently one shared immutable line collection). Cache insert/hit clones an Arc handle only. Update every caller/test to borrow or retain the returned lines; do not retain the old Vec-returning clone shim. Markdown source hashing happens only when a block actually needs derivation, not to rediscover unchanged historical content every frame.

Each prepared message block retains compact physical-row descriptors and only genuinely derived/shared backing. Markdown rows reference the shared parsed allocation by line/span/byte range. Plain message text, thought segments, unchanged output and zj0m diff rows reference the authoritative current history/live source through stable message position plus a concrete presentation-source selector and byte range; they are not copied into `Line<'static>` merely to retain them. Generated labels/gutters/counts own their necessarily generated text once. Source-run resolution is a private concrete presentation operation in chat, not a generic source-provider trait or core responsibility. The layout routine handles source tokens/ranges and styles, not ChatMessage interpretation.

Preparation may borrow the message while producing descriptors, but retained state contains **no pointers/references into movable UiState strings** and no self-referential struct. Before visible painting, source selectors resolve against the current immutable history view; revision handling replaces descriptors before any changed source is used. Shared Markdown allocations are the other stable backing. This makes source ownership/lifetimes explicit, avoids a hidden second copy of a large Diff/plain line, and lets row painting borrow only the visible runs. Live Markdown/thought and activity/header are separate tail/prefix blocks so a spinner or child name change does not invalidate committed content or unchanged live Markdown. No new Myers path is created.

### 4. Index physical row heights; paint references directly

Private `widgets/chat_layout` owns prepared blocks, physical-row indexing, source-run descriptors and visible painting. A vector-backed cumulative-height/Fenwick index supports total height, point-height changes, append and row seek without updating every later prefix. Stable positions let prefix pruning retire only dropped blocks. Retired slots can be compacted amortized when the retired prefix dominates retained slots; compaction rewrites **metadata only**, not Markdown, diffs, text or wrapped rows. Storage remains proportional to retained prepared history rather than total lifetime appends.

Seek the first intersecting block/row in `O(log N)`, then visit exactly intersecting rows. **Overscan is zero physical rows.** There is no whole-message overscan exception hiding unbounded work. Paint source-run slices into the frame Buffer without constructing a whole-history Paragraph, per-frame line Vec, or per-frame owning String copy. The scrollbar uses the same indexed total/window and F12's scroll-independent content width. A single enormous line/message is not allowed to evade the visible-work guarantee: cached row descriptors permit direct access to its middle/bottom.

For `N` retained blocks, `R` visible rows and visible styled runs `S_v`, warm painting is `O(log N + R + S_v)` plus existing viewport-cell drawing. With `K` changed messages, update adds changed-block derivation/wrapping plus `O(K log N)` metadata, not all-history source traversal. Cold load and true global width/theme invalidation may prepare all retained messages once; benchmark those separately. No new input or history limit makes these claims vacuous.

## Wrapping tradeoff: explicit approval required

Current conversation wrapping is Ratatui `Paragraph` word wrapping, `Wrap { trim: false }`, not Cyril's input/path character wrappers. Locked widgets 0.3.0 source shows public Paragraph always scans a wrapped scroll prefix; its `reflow` module is private (`lib.rs:131`; `paragraph.rs:418-445`). Therefore merely retaining a Paragraph, using per-message Paragraphs, or indexing only message heights **does not meet** direct access to a deep row in one large message.

**Approved:** implement a small private styled-grapheme row-layout routine in `widgets/chat_layout`, source-derived from pinned Ratatui WordWrapper with required attribution/license notice, adapted to emit borrowed-source run descriptors. Use public styled graphemes/Unicode widths. Preserve whitespace, word-break, overwide-symbol and zero-width behavior except the requester-approved F11 unsafe carry correction; do not introduce input/approval character wrapping or upstream2680's broader semantics. Store final row/run descriptors, not per-character history metadata. This is production layout ownership, not a second Markdown parser or test-only renderer.

Alternatives and approval choice:

- Full offscreen Buffer per history/message: reuses exact Paragraph behavior but retains a cell grid proportional to width × all historical physical rows, including blank padding; materially worse memory than compact text/run layout.
- Textwrap/new generic wrapping dependency: would need fidelity work for styled graphemes and can silently change existing wrap semantics; no demonstrated reuse win over the small pinned routine.
- Smaller per-message Paragraph alternative: retain per-message styled text/height and use the existing exact Paragraph wrapper only for intersecting messages, with overscan explicitly defined as the hidden wrapped prefix of the first partially visible message. This avoids owning wrapping behavior and has lower migration risk. Its warm cost is viewport work **plus the size of that partial-message prefix**, which is not uniformly bounded because Cyril accepts large messages/lines. It does not meet the strict zero-overscan guarantee proposed above, but the issue itself says “documented overscan,” not “zero.” Parent approval may select this less intrusive interpretation explicitly; then the design must amend C1/deep-message qualification to disclose and measure that prefix work rather than claim strict row proportionality. Do not silently declare either interpretation requester-mandated.

Pinned upstream Paragraph remains the independent existing cell/style/count reference wherever its wrapped positive glyph starts are inside the original viewport. Differential fixtures retain all defined comparisons and deep windows. F11 precisely bounds the approved exception for independently classified unsafe reference rows, replacing that undefined comparison with literal normative rows/cells plus conservation/style/bounds checks. Visual similarity or simply skipping the broken case cannot establish correctness. This introduced ownership remains the principal design tradeoff.

## Module shape / ownership ledger

| Module | Disposition and owned responsibility | Forbidden responsibilities / interface discipline | Verification |
|---|---|---|---|
| `message_history.rs` (new) | Create concrete mutation/revision owner shared by main and child histories. | No ratatui/layout/theme/core protocol or event routing. No exposed mutable Vec. No configurable global journal service. | Revision/order/coalescing/pruning/adoption tests through its real interface; independent review. |
| `state.rs`, `subagent_ui.rs` | Retain notification, receipt/steer/tool lifecycle and history policy. | Protected parents for new index/layout bodies: only call the history owner's mutation methods and expose immutable views. Preserve peer repairs and tool indices. | Lifecycle tests, caller migration and review. |
| `widgets/chat_layout` (new private module) | Own prepared immutable blocks, physical rows, height index, exact wrapped layout and borrowed visible painting. | No input handling, retention policy, bridge/IO, global transcript cache, Markdown parser or Myers derivation. | Work/row-fidelity/scroll tests and production benchmark. |
| `widgets/chat.rs` | Retain message presentation and transcript selection. | Wire selected history/live/header/activity blocks into layout; no whole-history per-frame Vec/Paragraph traversal, duplicate layout engine or diff derivation. | Existing conversation fixtures plus visible-work checks. |
| `widgets/markdown.rs` | Retain Markdown derivation/cache; share immutable outputs. | No transcript versioning or App ownership. Existing style-input identity from x5xi retained. | Existing Markdown/cache/theme tests and Arc-backed caller migration. |
| `render.rs` | Own concrete RenderState interface and root geometry/render wiring. | Protected parent for chat layout: no layout/index algorithms. Retain fallback and exact frame geometry. | Existing floor/overlay tests and independent review. |
| `cyril/src/app.rs` | Own long-lived RenderState and pass it to draw. | Wiring only; no row selection, theme/cache derivation or history bookkeeping. | Compiler/caller checks and real App draw smoke. |
| `cyril-core` | Retain typed source domain; the existing approved receipt-transform exception returns `HarvestedReceipts` with ordered receipts and conservative earliest-removal splice metadata, consumed via `into_parts` (see Receipt transform metadata below). | No history display revisions, Theme, ratatui or rendering dependency. Preserve recognition, surviving bytes, receipt order and UI-owned metadata interpretation. | Existing parser metadata/receipt tests, dependency/compiler checks and review. |
| `traits.rs` | Retain the read-only `TuiState` history/live contract and authoritative tracked-tool borrowed/derived text projections, public Cow adaptations, direct recorded-source resolution and indexed first-Diff/cached preview views, as already approved in the Concrete implementation handoff and First-Diff source ownership amendments below. | No physical prepared-row/index/wrap algorithms, retained transcript layout owner, event routing, mutable history escape, generic source-provider service or new diff-derivation path. | Existing history/live/tool-text/diff projection tests, public-root caller/compiler checks and independent review. |

The history owner earns depth by making missed mutation invalidation impossible at the Vec seam, rather than relying on all 22 sites to remember independent counters. The layout owner earns depth by hiding physical row preparation/index/seek from chat/root callers. No generic cache, scheduler, event bus or renderer adapter family is created. No custom AST/source-policy gate is warranted; compiler visibility and ordinary independent ownership review suffice.

## Claims / falsification

| ID | Claim and covered cases | Falsifier / verification | Regression coverage | Status |
|---|---|---|---|---|
| C1 | Warm render work is visible-only, including one giant message/line and cache-size-exceeding unique history. | Test-only counters at actual block/Markdown/diff/wrap/hash operations and row/index visits. After preparation, 100 unchanged draws at 160×48 must do zero historical block derivations, complete-source hashing or wrapping; source reads and painting are limited to selected visible runs, with indexed row lookup. Compare 100/1,000/10,000/50,000 retained messages with the same visible suffix. A deep-row giant message must not traverse its hidden prefix. Assert visible positive-control text/styles. | New consumer performance tests through real root draw, not a standalone helper. | PENDING — implementation + parent pre-fix RED/post-fix GREEN. |
| C2 | Main scrolling and wrapped output remain correct, including >65,535 rows and constrained geometry, with only approved F11/F12 exceptions. | Existing Paragraph exact cells/styles/counts wherever reference positive starts are valid; independently bounded F11 classification plus literal normative row/cell/conservation/style checks for unsafe cases. Preserve first/middle/last windows, follow/rows-back/overlarge-back, zero-size, 3-row floor, input and resize; F12 scrollbar never overwrites content or changes width on scroll. | Existing conversation/floor/deep-history/Unicode cases plus unchanged positive overhang, narrow carry and fixed-gutter full-URL/work regressions. | PASS — current post-repair deterministic qualification and [required supported native outcomes](#main-technical-native-gate-judgment), with separate evidence layers; later integrated-tree qualification remains Main-owned/PENDING. |
| C3 | Committed edits/append/prune/reset/adopt invalidate correctly without rescanning unaffected sources. | After warm layout: append; update an old tool without count change; mutate an old receipt note/status while live text stays unchanged; multiple edits before draw; prefix trim at max retention; clear/reload; child adoption/prepend. On incremental changes exactly changed blocks are rederived, removed blocks disappear, and untouched sources are not revisited for preparation; epoch-reset operations rebuild the newly authoritative history once. Metadata-only compaction must preserve prepared blocks. | Existing main/child/workflow lifecycle and peer missing-result tests; new revision/index consumer tests. | PENDING — parent lifecycle/work run. |
| C4 | Streaming, thought, activity and focus remain correct with independent invalidation. | 1,000 live deltas over a large committed history prepare live blocks only; no change redraw reuses them. Flush text/thought at tool boundaries preserves order/no duplication. Spinner/input-only redraws do not reparse live Markdown. Focus main↔child, child completion/removal and workflow adoption never show another transcript's content; child remains bottom-follow. | Existing chronological render, thought, drill-in and workflow-adoption tests; extend root consumer scenarios. Main↔child focus/back uses the existing state/render/root qualification under the [supported-controls user approval](#supported-controls-user-approval), not native focus entry or a new UI control. | PASS — current state/render/root qualification and [native streaming/thought/activity outcomes](#main-technical-native-gate-judgment); later combined-feature/integrated-tree qualification remains Main-owned/PENDING. |
| C5 | Shared cache ownership and theme/width invalidation are coherent. | Repeated Markdown result obtains the same shared immutable allocation; no deep-clone cache hit. Change width and relevant style, verify correct tables/code/wrap/cells then warm reuse; change height alone, verify no block rebuild. Theme identity is structurally derived; x5xi narrow highlighting behavior remains green. | Existing Markdown/theme/palette/cache fixtures plus new root invalidation tests. | PENDING — parent regression/review. |
| C6 | Practical production-shaped long-history rendering is qualified, with memory tradeoff visible. | Run benchmark below on one pinned runner with cold/warm/stream/scroll/invalidation cases and recorded samples. Smoke uses real App draw and actual native scroll/resize/palette changes; color modes use separate launches and main↔child focus/back uses existing state/render tests under the [supported-controls user approval](#supported-controls-user-approval). Review timings and retained storage against source/work results; do not fabricate an uncalibrated hard latency PASS. | Benchmark target and retained receipts; independent review of assembled stack. | PASS — current bounded-source TestBackend benchmark/source review and [required supported native outcomes](#main-technical-native-gate-judgment), not native performance or final publication approval; later integration/new-PR CI remains Main-owned/PENDING. |

For the wrapping gap, use existing pinned Paragraph as the independent exact Buffer cell/style/count reference wherever defined, not a new test renderer. Retain all whitespace/indentation/trailing-space, unbroken-word, wide/emoji, combining/zero-width, span/style-boundary, blank-line and widths0/1/2 fixtures and arbitrary windows. Only F11's actual unsafe positive-start condition changes the comparison obligation; its independent normative goldens and conservation/style/bounds checks replace, rather than silently discard, that coverage. Preserve captured fixture provenance, never regenerate expected output from current implementation.

Bug sensitivity: parent retains pre-fix RED from the long-history/deep-row work and reachability checks, and post-fix GREEN. Positive-control visible-content assertions ensure skipping history or rendering nothing cannot satisfy counters. Additional arbitrary mutations/custom parsers are unnecessary. If implementation reveals a real reference mismatch or missing premise, return that affected decision to design rather than changing expected output to match the new algorithm.

## Production-shaped benchmark and smoke protocol

Extend zj0m's `harness = false` `conversation_render` benchmark; no new production or benchmark framework dependency. Entry is public UiState + typed notifications + the actual retained RenderState + root `render::draw` + one Ratatui Terminal/TestBackend. Do not benchmark a preclipped synthetic list or only Markdown cache lookup.

Fixtures:

1. 100, 1,000, 10,000 and 50,000 retained chronological messages, with explicit UiState retention set high enough and an assertion that none were silently pruned. Unique sources exceed the 256-entry global Markdown cache. Mix user paragraphs/paths, assistant lists/code/tables, thought, read/execute output, completed Writes and steer status; keep the same latest visible suffix across sizes.
2. A single multi-megabyte Markdown message and a giant single logical line, each sampled near top/middle/bottom. This distinguishes true row access from whole-message overscan.
3. The same long history receiving live deltas, an older tool edit, a receipt status/note update, append+prefix prune, and focus swaps. Separate mutation/setup from the measured subsequent warm frame; also report update cost.
4. Width 160→80→160, height-only 48→12→48, and truecolor/no-color/theme changes. Bulk invalidation/cold preparation is reported separately from steady redraw.

Protocol: one reference machine/process, release bench profile, 160×48 baseline backend, fixture construction outside timing. Record source revision, Rust/compiler/target/features, CPU/environment, raw samples, cold preparation time and prepared row/run/storage counts. Warm up 100 draws, then 30 samples of 100 stable draws; report median/p95 per frame for each history size and case. Keep CPU-intensive gates/agents serialized. Deterministic work assertions are the automated scaling fence; benchmark samples establish practical cost without a fabricated universal milliseconds/line/allocation cap. If a hard latency requirement is later requested, it needs runner calibration and approval under the performance-fence policy, not a convenient new threshold.

Planned command: `env -u CARGO_TARGET_DIR cargo bench -p cyril-ui --bench conversation_render -- long-history` (final harness flags recorded with actual results). Parent runs focused tests, full UI tests, affected binary tests and required workspace compiler/lint/format checks after integration; no checks were run here.

Smoke must exercise the production App draw lifetime, not create a fresh RenderState per frame. With a safe owned fixture, stream text/thought/tool updates, scroll oldest/middle/newest and back to follow, resize during streaming and change palettes natively. Under the [supported-controls user approval](#supported-controls-user-approval), verify the truecolor/none pair in separate native launches and main↔child focus/back through existing state/render tests instead of the two unreachable native interactions; add no UI controls. Retain actual terminal screen/text evidence plus source/revision for native interactions, separately attributed startup-mode receipts and existing state/render results for the replacements. Exact deterministic root cells/styles/conservation, native-export/source correlation and positive actual-PNG appearance have separate roles in the [current Main technical judgment](#main-technical-native-gate-judgment). The supported native wide/combining/full-frame-right-edge, opaque-URL, scroll/follow, streaming, resize and palette outcomes now PASS. The benchmark/deterministic retained-history and giant/>u16 obligations above remain distinct; no native50,000-history claim is made. Existing workbench/cross-feature and final integration obligations retain their separately bounded proof and Main ownership. No external account/state mutation is necessary. After smoke, update existing architecture/changelog documentation at its discovered owner; workflow artifacts are not a substitute for describing the shipped renderer seam.

### Main technical native-gate judgment

**PASS — required supported native outcomes on the qualified current source.** Main, not the user or either visual reviewer, makes this technical combined-layer judgment under the unchanged approved claims. The owning [native supported-controls evidence record](current-width-e3f2a7c7/native-supported-controls-evidence.json) binds the actual owned Ghostty/Xvfb `truecolor`/`none` pair, source/environment, content assertions, complete PNG reviews, natural exits, cleanup and source-preservation receipts; its [native evidence archive](current-width-e3f2a7c7/native-supported-controls-evidence.tar.gz) preserves the supporting captures. Figures and hashes belong to that record, not duplicate editable totals here.

The required outcomes are established by three complementary layers, not by treating a screenshot as a cell oracle:

1. **Exact deterministic current root/layout proof:** the qualified wrap/presentation/root tests retain defined Paragraph cell/style comparisons, the [F11 normative rows/cells and eligible-content/style conservation](#approved-root-correction-and-replacement-coverage), and [safe-start/golden/classification controls](#deterministic-and-production-shaped-proof). The complete opaque-URL byte/style, fixed-gutter and warm-scroll root controls remain exact deterministic proof. Current qualification and retained-versus-fresh sensitivity are owned by [review-decisions.md](review-decisions.md#retained-f11f12-applicability-versus-fresh-released-width-sensitivity) and the publication manifest, not re-executed by the native review.
2. **Native-export/source correlation:** source-bound native history/active exports and fixture/scene receipts correlate the actual launched application, content and supported interactions with those exact deterministic cases. Exports are not style-cell captures and do not independently establish every cell or byte-conservation claim.
3. **Positive actual full-window PNG appearance:** independent reviews inspect every indexed image and every named native appearance obligation in each mode, with no observed visual defect. They establish the native styling/no-color appearance, palette lifecycle, streaming/resize, scroll/follow, relative glyph placement, wide full-frame edge and opaque-URL/gutter usability required by the native coverage paragraph below. They do not establish exact literal bytes, cell ownership, style values or repeated-character conservation.

The visual reviews' **26 literal claims per mode remain NOT_PROVEN by visuals alone** even though every named appearance subcheck passes. Main discharges the combined native gate with the separately qualified exact and correlated native layers; neither a new pixel oracle nor a waiver is introduced. The original raw capture handoff and mode receipts retain their capture-time `PENDING_VISUAL_REVIEW` statuses unchanged: those historical statuses precede the later reviews and this judgment, and are not the current gate.

The native pair is two separate launches, not an all-four-mode native matrix or same-process mode switching. Existing all-four-mode state/render coverage, same-process theme/mode cache-invalidation tests and benchmark keep their own qualification; existing state/render/root tests own approved main↔child focus/back. No new controls or native50,000-history result is claimed. The original benchmark fixtures and deterministic workload paragraph require 50,000 histories and giant/>u16 coverage in their own layer; earlier blanket native-pending wording did not create an additional native50,000 fixture obligation. This is an attribution correction, not retirement or waiver of a controlling positive obligation.

This scoped PASS leaves current-main/held93 integration, retained workbench/cross-feature limits, final integrated-tree/new-PR CI/platform qualification and post-smoke permanent architecture/changelog delivery with Main as recorded in [plan.md](plan.md#current-slice-4-evidence-disposition-and-remaining-obligations-2026-10-21). User-native Kiro/account/model turns remain separately paused, not waived. It is not Structural terminal completion, whole-issue acceptance or merge approval; PR187 remains untouched/unmerged.

## Delivery boundaries and completion

Third substantive issue PR: history-owner migration, retained RenderState/root caller migration, shared Markdown output, exact indexed row layout/painting, behavior/work tests, long-history benchmark and post-smoke documentation. It can be sliced internally into independently green changes after approval, but no half-migrated public interface is a delivery boundary. No commit/push is authorized in this worker assignment.

Permanent non-goals: no fixed-top scroll redesign, input/path wrapping replacement, new workflow renderer, retention-limit change, globally cached transcript registry, incremental Markdown grammar, protocol/backend behavior, or generic feature budget. These are not deferred acceptance. Cold preparation/global invalidation remain explicit costs because the requested guarantee is steady-state visible-window work; they are measured rather than hidden.

## Approval

**DG — 2026-10-19, approved by parent Main under delegated routine sign-off.** User delegation reported verbatim by Main: “Delegate routine sign-offs” — “I select and record evidence-backed designs. I still ask before scope changes, relaxed acceptance, architecture changes outside the existing boundaries, or explicit risk acceptance.” Main approved concrete history revisions/stable IDs, App-owned separate RenderState, Arc backing and compact row indexing, and selected strict visible-only rather than unbounded partial-message overscan. That approval required exact pinned reference cells/styles and attribution; only the later explicit F11 requester exception below changes its fidelity scope. Keep a concrete implementation, not a general indexing framework. zj0m first-Diff behavior is user-approved in its owning design. Reviewed1v25 a5d2c4b5 is integrated as1ee1d92f (PR183); reviewed fiv1 7eefe275 is integrated as75aaa6f1 (PR184 based on183), preserving dirty history/cache work. No unreviewed hunks are copied. Prerequisite local receipts and the separately accepted Windows limit do not establish platform-green or C6 completion. Main owns remaining C6 benchmark/smoke/regression/repository/review gates.

### Supported-controls user approval

**User approval — 2026-10-22 UTC, reported by Main from the explicit ask result.** Selected option: **Test supported controls**.

> Native-proof scrolling, resizing and palette changes; test color modes in separate launches and child focus/back through existing state/render tests. Explicitly replace the two unreachable native interactions; no new UI controls.

This changes only the two native interaction instruments: same-process color-mode switching becomes separate-launch mode verification, and main↔child focus/back becomes existing state/render verification. C4 focus behavior, completion/removal/adoption isolation and bottom-follow are unchanged. Existing deterministic same-process theme/color-mode cache-invalidation tests and the production-shaped benchmark remain required; separate launches do not supply that invalidation evidence.

**Main's roster operationalization:** retain the original truecolor/none native pair in separate launches. The existing all-four-mode state/render matrix remains distinct; two startup modes are not claimed to cover all four supported modes, and this approval adds no four-launch native matrix.

**Current supported-native result: PASS**, by [Main's technical combined-layer judgment](#main-technical-native-gate-judgment) and the owning [source/environment-bound evidence record](current-width-e3f2a7c7/native-supported-controls-evidence.json), not by extending the user's words. Native scrolling, resizing, palette changes, streaming, wide/combining/full-frame-right-edge and opaque-URL usability are discharged with their exact deterministic, correlated native-export and positive PNG appearance layers kept distinct. The 50,000-history and giant/>u16 benchmark/deterministic proof, retained workbench/cross-feature limits and final integration obligations remain in their own layers; no native50,000 claim or new native fixture requirement follows from the earlier overbroad pending wording. Existing PTY/TestBackend receipts alone still do not prove native glyph or interaction behavior.

This explicit user replacement is separate from Main's earlier technical adoption of bounded Page-key batching for ordinary scroll/follow outcomes, owned by `current-width-e3f2a7c7/scroll-obligation-adjudication.json`. That record's original burst FAIL and qualified pre-C6 ASSERTFAIL remain preserved, and no product/source, UI-control, or harness-hardening change is authorized here.

Parent's dependency instruction, verbatim: “Start c6la full approved migration now; do not wait for fiv1 for MessageHistory.” The same message requires integrating fiv1's exact reviewed commit when supplied and preserving zj0m's diff cache. This is explicit parent authorization, not an implementer waiver of a pending prerequisite.

## Concrete implementation handoff contract

The following names pin the approved concrete seam for disjoint implementation; they do not expand behavior or architecture:

```rust
// public message_history module; no ratatui or core event-routing dependency
pub struct MessagePosition(pub(crate) usize); // Copy, Eq, Ord
pub struct MessageRevision(/* private sequence */); // Copy, Eq
pub struct HistoryRevision(/* private identity/epoch + sequence */); // Copy, Eq, Default-invalid
pub struct LiveRevision { pub text: u64, pub thought: u64 } // Copy, Eq, Default
pub struct HistoryChanges<'a> {
    pub reset: bool,
    pub retained: std::ops::Range<MessagePosition>,
    pub changed: ChangedPositions<'a>, // borrowed Iterator<Item=MessagePosition>
}
impl MessageHistory {
    pub fn new() -> Self;
    pub fn messages(&self) -> &[ChatMessage];
    pub fn revision(&self) -> HistoryRevision;
    pub fn retained_range(&self) -> std::ops::Range<MessagePosition>;
    pub fn get(&self, position: MessagePosition) -> Option<(&ChatMessage, MessageRevision)>;
    pub fn changed_since(&self, revision: HistoryRevision) -> HistoryChanges<'_>;
    // concrete From<Vec<ChatMessage>> and Default produce distinct identities
    pub(crate) fn push(&mut self, message: ChatMessage) -> MessagePosition;
    pub(crate) fn edit(&mut self, position: MessagePosition, edit: impl FnOnce(&mut ChatMessage)) -> bool;
    pub(crate) fn drop_prefix(&mut self, count: usize);
    pub(crate) fn clear(&mut self);
    pub(crate) fn prepend(&mut self, earlier: MessageHistory);
}
```

`TuiState::history() -> &MessageHistory` and `live_revision() -> LiveRevision` replace the old message/version accessors; child streams expose the same history/live revision getters. No raw mutable history escape or constant mock revision substitutes for the real owner. The shared mock keeps the existing field name `messages` but its type becomes MessageHistory; fixture Vec initializers convert explicitly. Live mock edits during a retained-render test must update their corresponding revision or use real UiState notifications.

The public root seam is `render::draw(frame, &dyn TuiState, &mut render::RenderState)`. Its private chat field holds `chat::ChatRenderState`, which is crate-visible and Default; `chat::render(frame, area, state, theme, &mut ChatRenderState)` is crate-visible. External consumer fixtures use the public root, not exposed layout internals. `RenderState::invalidate` invalidates chat state after a panic. No old-signature compatibility shim remains.

Chat owns concrete presentation source selectors and resolution, including authoritative plain/history/live text, diff row ordinals, selected raw-output JSON strings, generated text and Arc Markdown backing. Layout owns compact rows/index/painting; no generic source-provider service, cloned plain/diff bodies or per-frame whole-history views. Test work counters live at actual operations and visible row/run visits; benchmark metrics are read outside timed draws.

Qualification hooks are concrete and tied to actual operations, not a generic telemetry system. `markdown::work_counts()` (cfg(test)) returns `(source_hashes, parses)`; chat's cfg(test) `work_counts()` returns cumulative `prepared_messages`, `prepared_live`, `painted_rows`, `resolved_runs` and `index_steps`. Tests compare snapshots, with separate live text/thought keys. `RenderState::layout_metrics()` exposes the prepared `messages`, `rows`, `runs`, `source_slots` and `owned_text_bytes` counts as public `render::LayoutMetrics`; the benchmark reads them outside timed draws.

The presentation borrowing gap uses one concrete existing-owner seam in traits.rs: crate-visible `ToolText::Borrowed { source: ToolTextSource, text: &str } | Derived(String)`, `TrackedToolCall::output_text_view()`, `error_text_view()` and `resolve_text_source(ToolTextSource) -> Option<&str>`. ToolTextSource is opaque Copy metadata to chat; resolution directly selects the recorded raw-output/content branch without rescanning/serialization. The exact reviewed fiv1 commit is integrated before this cutover; its first-nonblank Text fallback and raw-output precedence are preserved. Existing public `output_text` / `error_message` remain useful consumers (including fiv1 capture tests), but now return `Cow<'_, str>` by adapting the same authoritative view decoder: no string clone, duplicate decoder or legacy allocating implementation is retained. JSON pretty-printing is the only genuinely derived owned backing. Existing extraction expectations migrate to borrowed comparisons; new ownership tests check actual source allocation identity and recorded-branch resolution.

## Generated boundary-grapheme clarification

**DG APPROVED — Main explicitly approved necessary crossing-grapheme derivation.** Old plain/thought/steer presentation uses one formatted span containing generated affixes and source text. Leading combining/spacing marks or trailing Unicode Prepend can form a single grapheme across that virtual boundary; treating affix/body as separate spans changes pinned Paragraph cells.

Retain only genuinely crossing-cluster bytes with the generated affix, leaving remaining ordinary/plain/diff source selector-backed and fixed labels static. A pathological grapheme can contain arbitrarily many combining bytes; no fixed copy bound or size cap is claimed. Derived bytes are included in owned-text metrics and preparation cost; visible paint cost includes selected grapheme bytes, not a fictitious constant bytes-per-cell bound. There is no per-frame assembly allocation. Prefix detection now uses borrowed GraphemeCursor chunks/context before materializing a crossing cluster; suffix detection uses an eight-byte UTF-8 stack probe, not one String per scalar. Noncrossing first graphemes are never copied merely to test a boundary.

Production-preparation/Paragraph differential fixtures retain all original combining/spacing/Prepend/control cases at widths0/1/2/3/7/40 plus4096-element noncrossing ZWJ and pathological combining/Prepend controls for UserText/SteerEcho. Large plain-source and nonzero-range lifetime controls retain zero body bytes and authoritative source pointers. Parent's generated-affix test reached the actual F11 reference failure; its repaired comparison now follows only that explicit exception. No wider source cloning or unrelated fidelity deviation is authorized; post-repair results remain Main-owned.

## Nested theme-fence coverage clarification

**APPROVED — Main relayed the requester's explicit fully recursive inventory budget:** 22 widget Rust sources for this renderer tree, 23 only once the separately approved MCP widget is included; unchanged 300000-byte total production cap and all five theme prohibitions. The earlier split-group/17-cap proposal was NOT adopted. One explicit path/source inventory now contains all 22 files, recursive filesystem enumeration compares the exact full relative-path set, duplicate paths fail, and every production source contributes to the same total cap and same five checks. Failure diagnostics identify the offending path. No subgroup, exemption or omitted nested source preserves a misleading 17 total. Parent owns the final 23-source assembly when MCP lands and actual source-gate/mutation proof.

## Approved existing-package dependency edge

Main explicitly approved the shared workspace declaration `unicode-segmentation = { version = "1.12.0", default-features = false }`, UI `workspace = true`, and only the cyril-ui lock dependency membership edge. The already locked package remains 1.12.0; no new package/version or alternate exact-pin/default-feature convention is introduced. The approval renderer uses the same declaration independently. Preparation source-verifies `GraphemeCursor::is_boundary`/`provide_context` in registry 1.12.0 `src/grapheme.rs:394-434,565-654`: “Only a part of the string need be supplied”; requested PreContext is supplied and retried over borrowed chunks. Existing wrapping continues to use pinned Paragraph-compatible span segmentation.

## Requester-accepted Windows baseline qualification limit

Main relayed the requester's explicit **Accept documented baseline exception** choice for the five batches. PR 183's Windows Test job hit the three existing cyril-u62f memory-runtime initial-readiness failures; 23 tests passed, three failed, 2670 later tests were cancelled and 13 skipped. Cancelled coverage is not claimed green. Parent's read-only diagnosis established unchanged memory code/test/manifests/lock/workflow identities and all other substantive PR 183 jobs passed. The memory mechanism is unresolved; this is an accepted qualification limit, not a runtime repair or platform-green. No retries, timeout changes, skips or memory-runtime changes are authorized, and new/differently attributed failures remain blocking. Preserve the [public record](https://github.com/dwalleck/cyril/pull/183#issuecomment-6284441811) and [retained receipt](local://windows-baseline-exception.md); complete all other c6la proof.

## Source-accessor naming under the unchanged policy

The private PreparedBlock getter and physical-row painter callback are named `source_text`, not `resolve`: the existing theme fence deliberately prohibits the broad `resolve(` spelling in widget production. All fourteen declarations/callers in presentation.rs, retained.rs and chat_layout/mod.rs are migrated with unchanged selector, borrowing, layout and painting semantics. No fifth-prohibition exception or policy relaxation is introduced.

## Post-RED approved owner repairs: concrete interfaces and limits

Main's actual five-test artifact605 establishes F6/F8 work RED with every positive control and nested semantic PASS, then explicitly DG-authorizes the following bounded implementation. No new renderer framework or public UI owner is introduced.

**First-Diff source ownership:** `TrackedToolCall::diff_view(&self) -> Option<(usize, DiffSource<'_>, &DiffPreview)>` combines one real indexed first-Diff selection with the existing lazy preview, including cached absence. Existing diff_source/diff_preview consumers remain useful, not compatibility aliases. Prepared `Source::DiffRow { content, row }` retains authoritative content/row ordinals; paint directly indexes the current content, matches Diff and reads the cached preview row. Equivalent source invalidation deliberately excludes ordinal so added/removed preceding Text keeps the same preview Arc while re-preparation records the new index. First-Diff selection still costs one content walk when required; warm visible paint no longer reselects hidden content. Actual predicate-step instrumentation stays in that single selector. Summary/body appearance and the completion-independent approval adapter remain unchanged.

**Receipt transform metadata:** Core `harvest(&mut String) -> HarvestedReceipts`; its private ordered receipt Vec and optional earliest_removed_start consume through `into_parts()`. Every successful splice min-updates earliest start across ALL removals, in current-buffer coordinates at each splice. None means no removal. Exact recognition, first-close parsing, malformed handling, deletion, surviving bytes and emission order remain unchanged. No Vec shim/root re-export. Before changing the export, linked-worktree LSP references falsely reported none (tool QA recorded); scoped AST plus exact identifier inventory mapped all nine old callsites: one UI production and eight calls across seven existing Core tests. All are migrated, including nested assert use.

**Displayed source owner:** UiState adds private streaming_text_cutoff initialized0. The trait getter directly borrows raw[..cutoff]. Shared append_streaming_agent_text snapshots old raw length/cutoff, appends and harvests/applies ordered receipts. It reuses old cutoff only when final length equals old raw length and no removal precedes old raw end, proving the entire old raw is unchanged. Otherwise unchanged withholding runs with FULL raw context. Revision changes when cutoff length changes OR earliest removal touches the old displayed prefix. This conservatively invalidates deletion/reappend-identical content, but never compares, clones or hashes the large body to decide source identity. Legacy whole-buffer harvest search remains and is not a zero-total-work claim.

Every nonempty raw flush mem::takes raw, resets cutoff0, commits the original raw AgentText and bumps live text revision, including whitespace-only/wholly withheld raw; empty flush stays noninvalidating. Thought trimming is separate and unchanged. Constructor, both append branches, harvest, flush and the sole direct-raw unit fixture maintain the cache; no lazy fallback for invalid state. Child stream/adoption ownership, queue recovery, id-first/FIFO matching and raw chronology remain unchanged. Frozen five review regressions plus original tests stay intact; supplemental metadata/flush/ordinal tests are authored, not worker-executed. Main owns actual GREEN and independent conformance/review.

## F9: preserve the separate color-policy obligation

Main DG-authorizes bounded repair within the already approved fully recursive22-widget coverage: this integration checker also retains highlight.rs, so its explicit inventory has23 entries here (combined MCP has23widgets+highlight24). No new limit or exemption is approved. Its palette/raw Color rules and signed RGB/Reset comparison exceptions differ from theme.rs's five explicit-theme prohibitions; neither checker substitutes for the other, and theme.rs's300000-byte cap remains unchanged.

Keep the two established checks rather than inventing a shared policy framework or deleting unique obligations. widget_theme_sources.rs recursively compares its explicit path inventory with every widget Rust source and rejects duplicate entries. production_source_text normalizes CRLF first, then truncates only at the ordinary test-module boundary; counters never end production. Existing audit_source and its exemptions remain unchanged, with the existing CRLF offset regression plus a new LF/CRLF counter/cutoff regression. No renderer/core API, permanent production behavior or frozen performance regression changes.

Named proof gaps use the real checker: nested raw color, post-counter LF/CRLF and unlisted nested source change falsePASS to FAIL; unchanged/signed conversion stay PASS, existing color stays FAIL. Frozen before/after checkers and fresh-copy harness remain unchanged. Exact inputs/commands, parent bg116 PASS receipts, independent F9 review and remaining integration scope are owned by review-decisions.md. Copies preserve captures and invalidate Core/UI inputs at shared-target switches. No new design decision or code change is made by recording evidence; full C6/merged qualification remains Main-owned.

## F11: approved narrow parity exception for unsafe carried-prefix starts

**Requester approval relayed explicitly by Main:** selected **“Approve a narrow parity exception”**. Main specifies: repair carried-prefix overflow in Cyril, retain existing parity for valid reference behavior, replace broken-reference comparisons with normative row/cell and content/style checks, and make NO vendoring/dependency change. Positive width3界界 stays one row of width4. No broader PR2680 semantics, blanket panic catch/skip, padded oracle or clipping. This is the reserved C2 amendment, not an implementer waiver. Source repair is now applied and independently source-reviewed; route.md/review-decisions.md own the frozen boundary and pending actual qualification.

### Evidence and the actual contract conflict

Both differential helpers render the real pinned Paragraph before our candidate. In widgets0.3.0, reflow107–164 can emit only the existing line, retain a full-width pending word and append another positive glyph to that word. Width2 `a界a` manually yields `a`, `界a`; its last `a` starts at2. Paragraph460–471 and our painter both use that unchecked start. This is not a valid final-wide-symbol overhang: width3 `界界` starts at0 and2, both inside the viewport, and its explicit one-row/width4 cell assertions are approved positive controls.

The supplied log lacks a backtrace. Static call order and source-derived exact coordinates establish a defect in the independent reference and an inherited defect in our routine; they do not fabricate the actual failing stack frame. Two independent source scouts agree. Rewriting run merging, grapheme source offsets or generated affixes has no causal support.

Primary upstream [2679](https://github.com/ratatui/ratatui/issues/2679) reports `a🌞b` width2. [2680](https://github.com/ratatui/ratatui/pull/2680), OPEN/unmerged at `6e25eca2ecda0d2dcec13cfb6f754208a315b5b7`, proposes incoming-inclusive root overflow checks and zero-width handling; the [exact diff](https://github.com/ratatui/ratatui/commit/6e25eca2ecda0d2dcec13cfb6f754208a315b5b7.diff) is not a released fix. That broader rule changes even `界界` width3 into two rows. Adopting it would intentionally change an existing positive fidelity obligation, not merely repair a buffer bound.

The original unconditional reference-first comparison was impossible for these unsafe inputs. The requester selected a bounded semantic exception, not a modified dependency. The previous vendor proposal is NOT selected and must not be implemented; no Cargo/source-identity/workspace-membership change is in scope.

### Approved root correction and replacement coverage

Correct the carry rule, not the paint boundary. After existing row emission and whitespace handling, before appending a positive glyph, if the retained pending prefix already occupies at least the viewport width, emit that prefix first. Use existing measured widths and an overflow-safe remaining-capacity comparison, not a new glyph/body pass. Earlier accepted positive starts remain inside their row; the new glyph starts in a fresh row. Preserve zero-width/control handling, original source/style boundaries, whitespace policy and legal final-wide overhang. This is a class-wide safe-start correction, not a fixture special case.

Production ownership remains `chat_layout/wrap.rs`; presentation changes only its test comparison. No public runtime API, painter, Buffer bound, source selector, history/cache owner or dependency changes. Preparation alone pays necessary extra row emission; warm frames gain no body clone/hash/all-history work or overscan.

Classify the ORIGINAL pinned behavior, never the corrected rows, by whether any positive glyph would start at or beyond the viewport's right edge under its original alignment/span segmentation. Because upstream reflow is private, a test-only legacy mode of the existing source-derived routine may supply those original rows. It is a narrowly named classification instrument, NOT an independent correctness oracle or second wrapper implementation. Cross-check its row count against actual Paragraph; valid cases still use real Paragraph exact cells/styles/counts.

Retain every original fixture/width/alignment/window/affix/control/style workload. Unsafe cases must still execute corrected row/window painting and bound/content/style assertions, not return or swallow panic. Preserve exact eligible non-whitespace graphemes/order/effective styles, including zero-width non-whitespace. Pinned whitespace classification includes U+200B; whitespace, including zero-width whitespace, retains its existing discard/order policy without invention or new retention. Independent literal normative rows/cells cover `a界a`, `a🌞b`, carried whitespace and leading-zero/spacing marks, including the actual affix path. Keep positive `界界` width3 one-row/width4 unchanged. Goldens derive from the approved rule, never repaired output.

No broader upstream incoming-inclusive wrapping, reference padding, dropped fixture, weakened Buffer, runtime clipping or all-Unicode exception is authorized. The named verification gap is ONLY that real Paragraph cannot paint its unsafe positive-start rows. The rest of C2 and all C1/C3–C6 obligations remain unchanged, subject only to the later explicit [supported-controls user approval](#supported-controls-user-approval) replacing the two named native interaction instruments; F11 itself grants no such replacement.

### Deterministic and production-shaped proof

Main retains the real three-test RED at830b4673; no rerun merely to confirm it. After repair, run the same original workloads with the approved bounded comparison plus new literal safe-start regressions and all exact defined Paragraph/affix/overhang/deep-window checks. Require safe positive starts, full eligible content, styles, row/cell goldens and classification positive/negative controls. There is no dependency patch to qualify.

Re-run actual C1–C5 workloads at100/1000/10000/50000 histories, large Markdown/single line, deep70000,1000 streaming deltas and width/theme mutations. Existing zero warm historical/hash/parse/wrap/Myers expectations remain; only legitimately invalid narrow-source row counts may change. Existing production benchmark timing/memory reporting and native consumer smoke remain required, not replaced by literal fixture success or a timing budget. Native coverage must include the problematic wide/combining content at the full-frame right edge as well as durable opaque URL/scroll/theme/resize behavior. Main owns all checks, fresh independent review, final docs/changelog after smoke and the issue PR.

## F12: DG-approved stable scrollbar gutter

Main DG authorizes geometry-only repair under the existing delegated routine approval. For outer width>1, always reserve its rightmost1 column; use the resulting contentRect.width for history synchronization and ALL live/activity preparation and contentRect for painting/viewport metrics. Render the scrollbar ONLY in the separate gutter. The content width must not depend on follow mode, overflow, focus or scroll offset; never subtract twice. For width0/1 retain full content width and omit the indicator.

Owner is `chat/retained.rs`, with deterministic root regressions in conversation_performance_tests.rs. The existing giant-unbroken benchmark's80,000 newline-free segments use the approved content width (outer160 minus fixedgutter), preserving its literal top/middle/70000/bottom markers and multi-megabyte/>u16 workload; outer dimensions/timing protocol stay unchanged. Read-only affected-fixture census found no justified TSV/snapshot replacement, so all captured bytes/provenance remain. No root-layout, URL bytes, protocol/auth, source-selector or dependency change. Tqr's owner implements the identical contract in its independent tree. Main owns full-URL/bounds/no-rewrap/tiny-width checks, source-policy/bench/smoke and publication. User-native Kiro checks remain separately paused, not waived.


## F11/F12 implementation status

Approved existing-owner source/test/benchmark repair is frozen as the five-file14945de1 delta/tree43db6bd5. Test-only original-mode classification is not an independent correctness oracle; real safe full Paragraph comparisons, real wrapped per-row cells/styles and independently specified literal wrapping expectations keep their separate evidentiary roles. Both scoped source reviews report correct/no findings; no worker executable checks or sensitivity runs occurred. Current source still requires Main's actual RED/restored-GREEN, repository/consumer/performance/final-review qualification. Exact identities/receipts are owned by route.md and review-decisions.md, not duplicated as test PASS here.

## Current Main038 integration evidence boundary (2026-10-22)

The [new integration manifest](current-main038-b5d45fe2/publication-manifest.json)
and [route binding table](route.md#held-main038-integration-and-current-app-consumer-2026-10-22)
own the actual current506 qualification and one fresh actual-App `truecolor`
consumer. All17 assigned local gates passed, including current source-derived
approval/App/state/cache/workbench controls, and the unchanged C6 consumer
completed its full predicate/capture inventory and independent owned cleanup.
Earlier authored or then-pending checkpoints are historical, not a claim that
their old source pins were freshly exercised.

No production design or approved C1–C6 obligation changes. App-owned
`RenderState`, original valid Paragraph parity and the narrow F11 replacement
coverage, F12's sole fixed-gutter owner, source/history/cache ownership and
giant/long-history deterministic workloads remain intact.

The native judgment above belongs to the original505/e3f2a7c7/3efb0b67 source
binding. Main retains that192-PNG/two-launch component under bounded unchanged
renderer and changed-approval applicability, corroborated by fresh current
ordinary and actual-App pending/resize/deny controls. This is not current506
native execution, an expanded native roster, a cell/pixel oracle or a waiver of
the visual-only literal limits. Existing focus/back and all-four-mode state/
render evidence remain distinct. Benchmark retention likewise does not create
new current506 timings; pending-preview timing is historical only.

The current descriptor names the qualification/execution snapshot, not README's
later documentation overlay or the final Git tree. Main still owns held038
merge/publication, NEW C6 PR and remaining CI/platform/delivery approval.
Original burst failures and paused user-native Kiro turns remain unchanged;
PR187 is untouched/unmerged. No Structural terminal completion is declared.

## Restored layout-owner evidence boundary (2026-10-22)

The [restored-owner manifest](current-restored-layout-owner-7d88498e/publication-manifest.json)
owns current qualification/execution source `7d88498e…` (506 inputs), its full
manifest/default-App pins and evidence inventories. Earlier e3 native judgments,
including the then-current PASS above, and b5 native/benchmark carry-forward
remain source-bound history; they are not current D1 producer acceptance.

No approved responsibility, interface, C1–C6 obligation, F11/F12 exception or
user approval changes. Main restored D1's approved separation: layout owns
physical preparation/aggregation/index/seek/traversal/painting; chat owns
presentation/source resolution and transcript policy. The existing Core row108
and `traits.rs` row109 already reconcile D2/D3 under their approved amendments,
without new behavior or dependencies. The [finding dispositions](review-decisions.md#restored-layout-owner-findings-and-current-evidence-2026-10-22)
preserve the original c084 reconstruction and conformance FAIL.

All 17 assigned restored-source local gates PASS. The fresh full benchmark
contains 79 ordered executions / 2370 samples, including pending preview, not
the retained78 subset. The [actual consumer aggregate](current-restored-layout-owner-7d88498e/c6la-restored-layout-owner-consumer-result.json)
owns first-attempt PASS and complete independent cleanup for both separate PTY
modes. Exact state/render/work tests, TestBackend timing, API/action/wire
observations and native appearance remain distinct evidence layers; submitted
input counts are not internal consumption, retained counts or performance proof.

### Current supported-native judgment and limits

**PASS — Main's combined approved supported-native judgment, with explicit
limits, for restored source `7d88498e…` / default App `e31ef685…`.** The manifest
binds fresh Ghostty/Xvfb captures, both complete independent reviews and Main's
later disposition. Each mode recorded 79/0 content assertions, 96 PNGs and 40
obligations with natural exit and complete owned cleanup. Main matched the
complete path/hash sets and source/binary identities. All 192 images / 80
obligations were reviewed: truecolor has nine visual and 31 qualified passes;
none has 39 qualified passes and one clean baseline/no-op, with no demonstrated
render regression. This is not all80 unqualified visual PASS.

- Five resize capture states comprise baseline/no-op obligation10 at 160×48
  cells / 1446×960 pixels and **four actual transitions**, obligations11–14.
- Clipboard toasts obscure the lower-center composer/footer. Required
  conversation content remains visible; no full-window unobscured claim follows.
- Exact deterministic cells/styles/conservation, native-export/source
  correlation and PNG appearance retain separate attribution. Screenshots do
  not independently establish exact URL/scalar/RGB/process claims.
- Native exports omit initial unattached U+0301 and U+200B: no exact source/native
  scalar equality or bare-mark visibility is claimed there. Positive-width
  suffix/repeated-run evidence keeps its separately recorded scope.
- Separate `truecolor`/`none` launches do not establish same-process native mode
  switching or all-four-mode native coverage. Existing state/render/root tests
  own approved focus/back and all-four-mode coverage; same-process invalidation
  tests and benchmark keep their own scope. No unsampled-transient, native
  child-focus/back, native50,000-history or native internal-performance claim.

Raw capture-time pending-visual statuses remain unchanged beside the later
qualified reviews and Main judgment. This is a source-bound combined native
result, not a pixel oracle, new control or acceptance waiver.

Fresh isolated reconstruction/design conformance, restored-head CI/platform,
applicable all-feature/release/doc gates and final combined acceptance remain
**PENDING**. Main owns final publication/index/commit/tracker work on existing
draft **PR197**; earlier NEW-PR/held-merge wording is historical. These five-doc
postimages are a publication overlay outside compiled506, not tested final-tree
identity. Original burst FAIL/pre-C6 ASSERTFAIL and ordinary-cadence limits,
remaining Structural/workbench/cross-feature obligations and separately paused
user-native Kiro turns remain unchanged. PR187 remains untouched/unmerged.

