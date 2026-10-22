# Final design-conformance report

**FAIL — the implementation leaves one unresolved physical-layout ownership discrepancy and two ledger-completeness discrepancies; this is a source/design comparison, not a runtime or full-feature verdict.**

## Scope and method

Compared `03838a501dad8be140acd78dc3fff4cae35fafef..c0846f994994575683dd9d1568ed08f003859886` in the assigned worktree.

Initially read only:

- `.cyril-c6la/reconstruction-final-c0846f99.md`
- `.cyril-c6la/design.md`

Then verified load-bearing reconstruction claims against production source and the patch. No other change artifacts, plans, specifications, checkpoints, routes, review decisions, or evidence archives were read. Two independent read-only research slices checked history/state/Core and root/shared-output ownership.

The scoped patch contains **31 changed files under `crates/`**, of which **16 contain production changes**. The reconstruction’s production inventory is supported by the inspected diffs. In particular, the changes to `theme.rs`, `workflow_ui.rs`, and `widgets/powers_panel.rs` are test-only; the benchmark is supporting qualification code.

**No builds, tests, linters, formatters, native sessions, or runtime checks were performed. No files were changed.**

For compact citations below:

- **Design** = `.cyril-c6la/design.md`
- **UI/** = `crates/cyril-ui/src/`

## 1. Approved-ledger conformance

Each approved ledger row receives one verdict. The three discrepancies below have dispositions in §3.

| Approved row | Verdict | Design versus actual production |
|---|---|---|
| **L1 — `message_history.rs`** | **MATCH** | Design: `:43-49,101,194-217`. Actual: `UI/message_history.rs:51-150,164-239`. The concrete owner holds messages, identity/epoch, stable retained positions, per-message revisions, and a borrowed latest-change iterator. Mutations are crate-private; no mutable Vec escapes; there is no layout/event-routing ownership. |
| **L2 — `state.rs`, `subagent_ui.rs`** | **MATCH** | Design: `:45-51,102,259-261`. Actual: `UI/state.rs:1266-1352,1484-1544,3575-3591`; `UI/subagent_ui.rs:79-172`. Parents retain notification, tool-ID, receipt, chronology, retention, and adoption policy while delegating committed mutation bookkeeping to `MessageHistory`. No physical-layout/index algorithm was added to these protected parents. |
| **L3 — `widgets/chat_layout`** | **MISMATCH — D1** | Design: `:77,103,110,221` assigns prepared blocks and physical preparation/index/seek/painting to layout. Actual layout owns the numeric index, wrapper and single-row painter, but aggregate prepared-row storage, preparation, index orchestration and visible-row traversal remain in chat children. |
| **L4 — `widgets/chat.rs`** | **MISMATCH — D1** | Design: `:104,110,221` limits chat to presentation, transcript selection and wiring into the layout owner. Actual `chat/presentation.rs:45-50,736-757` and `chat/retained.rs:47-64,244-349` additionally own physical-layout aggregation and traversal. The presentation/selection responsibilities themselves are present. |
| **L5 — `widgets/markdown.rs`** | **MATCH** | Design: `:67-73,105`. Actual: `UI/widgets/markdown.rs:17-18,39-76`. Cache storage and public results are immutable `Arc<[Line<'static>]>`; insert/hit paths clone Arc handles, not line collections. Markdown parsing remains here. |
| **L6 — `render.rs`** | **MATCH** | Design: `:55-63,106,219,223`. Actual: `UI/render.rs:8-53,89-171,187-225`. Root owns the concrete retained-state interface, metrics surface, frame composition and panic reset. The production diff preserves geometry arithmetic and forwards the actual chat rectangle. |
| **L7 — `cyril/src/app.rs`** | **MATCH** | Design: `:61,107`. Actual: `crates/cyril/src/app.rs:76-78,465-467,950-978`. One `RenderState` lives beside `UiState`, is initialized once, and is passed to root drawing. No row selection, derivation or history bookkeeping was introduced here. |
| **L8 — `cyril-core`** | **MISMATCH — D2, ledger wording only** | Design ledger `:108` says Core is unchanged, but the explicit amendment at `:257` authorizes `HarvestedReceipts` and splice metadata. Actual: `crates/cyril-core/src/types/steer_receipt.rs:77-93,164-200`. Implementation follows the amendment; the ledger does not reflect it. |

**MISSING — ledger-not-code:** none established. Every approved responsibility has some implementation; L3 is a misplaced/incomplete ownership boundary, not a wholly absent implementation.

**UNCOVERED — code-not-ledger:** `traits.rs`, detailed as D3. Its interfaces are explicitly authorized elsewhere in the design, but no ledger row covers that production module.

## 2. Every changed production module: single-row map

Supporting registration changes are mapped to the owner they register; they are not treated as new behavioral owners.

| Changed production module | Assigned approved row | Result |
|---|---|---|
| `crates/cyril-core/src/types/steer_receipt.rs` | L8 | MISMATCH D2 |
| `UI/lib.rs` | L1 | MATCH — production addition only exports `message_history` (`:15`); other additions are test registrations |
| `UI/message_history.rs` | L1 | MATCH |
| `UI/render.rs` | L6 | MATCH |
| `UI/state.rs` | L2 | MATCH |
| `UI/subagent_ui.rs` | L2 | MATCH |
| `UI/traits.rs` | **No approved ledger row** | UNCOVERED D3 |
| `UI/widgets/chat.rs` | L4 | MISMATCH D1 at its aggregate child boundary |
| `UI/widgets/chat/presentation.rs` | L4 | MISMATCH D1 — presentation is appropriate here, physical-row aggregate ownership is disputed |
| `UI/widgets/chat/retained.rs` | L4 | MISMATCH D1 — selection/invalidation/gutter are appropriate here, physical preparation/seek/traversal exceed the stated boundary |
| `UI/widgets/chat_layout/index.rs` | L3 | MATCH as a numeric layout primitive |
| `UI/widgets/chat_layout/mod.rs` | L3 | MATCH as the physical row painter |
| `UI/widgets/chat_layout/wrap.rs` | L3 | MATCH as the attributed source-range wrapper |
| `UI/widgets/markdown.rs` | L5 | MATCH |
| `UI/widgets/mod.rs` | L3 | MATCH — registers the required private layout module (`:3`) |
| `crates/cyril/src/app.rs` | L7 | MATCH |

Thus the requested exact-coverage condition is **not yet satisfied**: `traits.rs` has no ledger row, and the assigned chat/layout rows do not accurately describe aggregate ownership.

## 3. Evidence-backed discrepancies and disposition proposals

### D1 — Reconcile aggregate physical-layout ownership

**Classification:** unresolved responsibility/interface discrepancy, not an established functional rendering bug.  
**Patch anchor:** `crates/cyril-ui/src/widgets/chat/presentation.rs:45-50`.

The controlling design assigns prepared blocks and physical preparation/index/seek to `widgets/chat_layout`, expressly hiding those mechanics from chat callers (`Design:77,103,110`). Production instead places `PreparedBlock` and its `Vec<WrappedRow>` in chat presentation (`presentation.rs:45-50`), performs source-to-wrapper input construction and physical wrapping in `Preparation::finish` (`:736-757`), and places the prepared deque, height-index updates, indexed seek, visible-block iteration and row slicing in chat retained state (`retained.rs:47-64,116-165,244-349`). Layout therefore supplies primitives rather than the deeper aggregate owner described by the design.

The later amendments do **not conclusively resolve this discrepancy**:

- `Design:221` authorizes chat’s selectors/generated/shared backing, but repeats that layout owns compact rows/index/painting.
- `Design:249` acknowledges `PreparedBlock::source_text` and the existing files while authorizing a naming-only correction with unchanged semantics.
- `Design:305-307` explicitly assigns the **geometry-only F12 gutter** to retained. That settles gutter placement, not the entire physical-layout aggregate.

**Proposed disposition for Main:**

1. **Restore the approved boundary:** move source-agnostic prepared-row aggregation and physical seek/visible-row traversal behind the concrete layout owner, while leaving domain presentation/selectors and transcript policy in chat; or
2. **Approve the observed ownership split explicitly:** record the actual boundary and residual coupling, and amend `Design:77,103-104,110,221` consistently.

The second option changes the stated responsibility/interface boundary; it must not be silently classified as a clerical ledger correction.

### D2 — Amend the Core ledger to reflect the approved parser metadata change

**Classification:** design-record inconsistency; implementation matches an explicit approved amendment.  
**Affected ledger anchor:** `Design:108`.  
**Production patch anchor:** `crates/cyril-core/src/types/steer_receipt.rs:77-82`.

The ledger says Core is unchanged, while `Design:257` explicitly authorizes `harvest -> HarvestedReceipts`, ordered receipt ownership, `into_parts`, and conservative earliest-removal metadata. Production implements exactly that parser-transform boundary (`steer_receipt.rs:77-93,164-200`); UI interprets the metadata for displayed-source invalidation (`UI/state.rs:1518-1544`). Core does not acquire display revisions, Theme, ratatui or rendering ownership.

**Proposed disposition:** update L8 to name the already-approved receipt-transform metadata exception and preserve its existing forbidden dependencies. This is documentation reconciliation under the existing amendment, not a request for new behavior or architecture approval.

### D3 — Add the explicitly approved `traits.rs` seam to the ledger

**Classification:** **UNCOVERED — code-not-ledger**, not unapproved interface implementation.  
**Production patch anchor:** `crates/cyril-ui/src/traits.rs:48-56`.

`traits.rs` changes the read-only history/live contract and supplies authoritative borrowed/derived tool-text projections, public Cow adaptation, direct recorded-source resolution, and indexed diff views (`traits.rs:48-56,374-414,423-495,576-738`). These interfaces are explicitly authorized by `Design:217,225,255`, but the ledger at `Design:99-108` contains no row for their owning module. Forcing this module into either the history or chat row would obscure its existing-owner responsibilities rather than provide an exact map.

**Proposed disposition:** add one `traits.rs` ledger row covering the read-only state contract and authoritative tracked-tool projections, forbidding physical-layout/index algorithms and event routing. Cite the existing interface amendments and record that their behavior, ownership and risk are unchanged.

## 4. Interface and cross-boundary verification

The inspected new interfaces are connected to actual consumers; no silent-discard integration defect was established.

| Produced interface/value | Consuming dispatch verified |
|---|---|
| `HistoryChanges`, stable positions and history cursor | `UI/widgets/chat/retained.rs:116-165` handles reset, prefix retirement, chronological append and changed existing positions separately. Revision order is not mistaken for transcript order. |
| Independent live text/thought revisions | `retained.rs:167-236,399-419` uses separate keys and requests live text only after a changed revision/identity guard. |
| `ToolText::{Borrowed, Derived}` and `ToolTextSource` | `UI/widgets/chat/presentation.rs:669-733` handles both variants; `:72-112` and `UI/traits.rs:697-715` resolve recorded authoritative branches. |
| Indexed diff `{ content, row }` | Preparation records the current ordinal (`presentation.rs:582-622`); painting indexes that content branch and cached preview row (`:93-106`). |
| Shared Markdown result | Preparation retains the Arc and records backing/line/span selectors (`presentation.rs:286-308`); visible resolution borrows the selected span (`:113-129`). |
| `SourceLine` / `WrappedRow` / `RowRun` | Wrapper inputs are consumed by wrapping (`presentation.rs:736-757`); selected rows reach `paint_row` (`retained.rs:325-349`), which consumes each run (`chat_layout/mod.rs:28-55`). |
| `HarvestedReceipts` | `UI/state.rs:1518-1544` consumes `into_parts`, updates cutoff/revision, then applies receipts in order. |
| `RenderState` / `LayoutMetrics` | App retains the former (`app.rs:76-78,465-467,965-969`); the benchmark reads metrics outside timed frame drawing (`benches/conversation_render.rs:520-529`). |

The **interface mismatch is D1**: direct concrete layout primitives and raw aggregate row storage are exposed to chat where the design promises a deeper physical-layout seam. The public root/history/tool-text interfaces themselves match the explicit handoff.

Existing approval diff consumers remain real consumers: `UI/widgets/approval.rs:580-584` routes diff previews into `UI/widgets/chat.rs:29-63`. Therefore `diff_source` and `diff_preview` are not obsolete compatibility aliases.

## 5. Dependency and protected-parent comparison

### Dependency/seam result

No additional unapproved external dependency, generic renderer family, global transcript registry, Markdown parser or Myers derivation path was established.

The observed flow is:

**App → root retained state/draw → chat coordination → presentation/tool projections/Markdown + physical-layout primitives → Buffer.**

That flow differs from the approved deeper aggregate seam only as recorded in **D1**.

Two concrete internal dependencies should be recorded rather than describing the implementation as strictly one-way:

- `traits` imports/returns `MessageHistory`, while history imports `traits::ChatMessage` (`UI/traits.rs:10,53`; `UI/message_history.rs:11`). The approved concrete history interface implies this coupling.
- Chat retained constructs the public parent’s metrics type (`UI/widgets/chat/retained.rs:71-105`; `UI/render.rs:8-16`). The metrics surface is explicitly authorized by `Design:223`.

### Protected-parent result

**No additional protected-parent mismatch found.**

| Protected parent | Both sides of comparison |
|---|---|
| `state.rs`, `subagent_ui.rs` | `Design:102` permits domain policy but forbids new physical index/layout bodies. Actual tool validation, stream flush, receipt cutoff and adoption logic remain domain policy; committed mutation bookkeeping is delegated (`state.rs:1266-1352,1518-1544,3575-3591`; `subagent_ui.rs:84-172`). |
| `render.rs` | `Design:106` permits root interface/geometry/recovery, not layout algorithms. Actual additions are retained state, metrics, reset-before-fallback, signature migration and chat forwarding (`render.rs:8-53,187-225`); existing frame budgeting remains at `:89-171`. |
| `cyril/src/app.rs` | `Design:107` requires lifetime/wiring only. Actual additions are the field, initialization and draw argument (`app.rs:76-78,465-467,965-969`). |

D1 should not be misattributed to App or root-render growth; it is the boundary between chat children and the physical-layout owner.

## 6. Explicit candidate tradeoffs for Main adjudication

These are not additional proven functional bugs, and no source repair is presumed.

| Candidate | Evidence and residual consideration |
|---|---|
| **Aggregate chat/layout split** | D1 is the principal architectural candidate. It must receive an explicit disposition, not silence or a broad “implementation matches” statement. |
| **Tool-label ownership and cloning** | Tool title fallbacks become owned labels during preparation (`UI/widgets/chat/presentation.rs:539-560`); `"Thinking..."` is also converted to owned text. These are preparation-time costs, not warm per-frame history clones. Main should explicitly retain or simplify this label-ownership tradeoff rather than implying every header byte is authoritative-source-backed. |
| **Separate lifecycle tool copies** | Main insertion creates owned tracked calls for history and active display (`UI/state.rs:1287-1303`); child insertion clones the incoming tool (`UI/subagent_ui.rs:134-136`). This is lifecycle ownership cost, not retained warm-render copying. |
| **Repeated presentation constants/policy** | The five-line output limit is repeated across borrowed/derived branches (`presentation.rs:687-729`); transcript and approval paths separately style diff gutters (`presentation.rs:597-627`; `UI/widgets/chat.rs:29-63`). No divergent behavior was established. Retaining duplication versus small local consolidation merits an explicit ruling, not a new policy framework. |
| **Internal dependency direction** | The history/traits type cycle and retained-to-root metrics dependency above are concrete coupling tradeoffs, not evidence of a generic service abstraction or runtime defect. |
| **Representation-coupled tests** | History tests inspect private metadata; state/child tests corrupt private tool maps; retained tests inspect deque/index/row allocation identity (`UI/message_history.rs:269-293`; `UI/state.rs:3768-3797`; `UI/subagent_ui.rs:341-369`; `UI/widgets/chat/retained.rs:535-665`). These complement authored root-consumer tests but are not interchangeable with them. |
| **Panic-test seam** | Root production code invalidates before fallback (`UI/render.rs:43-53`), but the inspected regression directly calls `invalidate` rather than injecting a preparation panic (`:669-678`). It supports reset/rebuild behavior, not a claim that panic delivery was exercised. |
| **Memory accounting scope** | History Vec capacity can retain a high-water allocation after drain/clear (`UI/message_history.rs:188-205`). Layout metrics count retained descriptors/backing, not allocator capacity or process RSS; the benchmark explicitly states that distinction (`benches/conversation_render.rs:520-529`). |
| **Reference-version wording** | Design rationale still names widgets 0.3.0 (`Design:85,277`), while current source documents the released 0.3.2/core 0.1.2 reference (`chat_layout/wrap.rs:6-17`; `Cargo.lock:2411-2414,2464-2468`). No dependency change occurs in this comparison. Main should keep historical derivation provenance distinct from the current fidelity reference; this is not a new parity exception. |

## 7. Qualification limits

Source inspection supports the reconstructed mechanisms:

- selected-transcript-only retained state;
- borrowed/coalesced history changes;
- independent live invalidation;
- direct indexed physical-row access;
- shared Markdown backing;
- source selectors instead of retained plain/diff body copies;
- the narrow carry correction in wrapping and the downstream fixed gutter.

It does **not** establish executed C1–C6 outcomes, benchmark performance, native appearance, platform qualification or full-feature correctness. Authored assertions are not passing receipts.

Linked-worktree LSP reference queries returned false-empty results for known production references. This was reported as tool QA; consumer verification used actual source and scoped structural inspection instead. No conclusion relies on those empty results.

## Final verdict

**FAIL — D1 requires an explicit ownership/interface disposition; D2 and D3 require ledger reconciliation before the requested exact module-to-approved-row conformance can pass.**

**Overall correctness:** incorrect **for this design-conformance gate**. No evidence-backed introduced functional bug was established by this review.

**Confidence:** 0.95.