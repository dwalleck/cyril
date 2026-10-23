# Fresh-context design-conformance comparison

## Scope and source binding

This is a **source-level design-conformance review**, not a native, CI, performance-qualification, or overall-completion verdict.

The reviewed worktree was `/home/dwalleck/repos/cyril-wt-perf-cyril-c6la`. Git reported HEAD `c0846f994994575683dd9d1568ed08f003859886`, but the reviewed implementation was the **current working production**, including the uncommitted repairs to `chat/presentation.rs`, `chat/retained.rs`, and `chat_layout/mod.rs`. HEAD’s earlier implementation was not substituted for those files.

The two authorized inputs had the requested hashes:

| Input | Observed SHA-256 |
|---|---|
| `.cyril-c6la/reconstruction-restored-7d88498e.md` | `97c8f8c7baf33453eaeb0a38eec052c2227925e276682e956ea89f0cc4881fad` |
| `.cyril-c6la/design.md` | `219f959520dd99b75d14a6b93ee2850e42bab80c62c3f1cd99118ac3e993a8e9` |

The supplied `7d88498e…` descriptor identifies the assignment’s qualification snapshot; I did not open its manifest or independently attest its qualification results. I read no other change artifacts, plans, checkpoints, prior reviews, PR descriptions, or issue descriptions. Reconstruction claims were checked against working production rather than accepted as authority.

No edits, builds, tests, benchmarks, linters, formatters, native runs, or Git mutations were performed. Linked-worktree LSP reference queries returned empty results despite directly observable consumers; that tool inconsistency was reported, and those empty results were not treated as evidence of absent callers.

### Citation notation

All repository citations are relative to the reviewed worktree:

- **D** = `.cyril-c6la/design.md`
- **R** = `.cyril-c6la/reconstruction-restored-7d88498e.md`
- **UI** = `crates/cyril-ui/src`
- **APP** = `crates/cyril/src/app.rs`
- **CORE** = `crates/cyril-core/src/types/steer_receipt.rs`

## 1. Approved-ledger conformance

The crate diff contains **16 files with production changes**. Each is assigned to exactly one approved ledger row below. Registration-only changes belong to the owner whose required interface they register; they do not constitute additional behavioral owners.

| Approved ledger row | Changed production modules assigned to this row | Verdict | Approved requirement versus verified working production |
|---|---|---|---|
| **L1 — Concrete history owner** — D:101 | `UI/message_history.rs`; `UI/lib.rs` | **MATCH** | D:26–52 and D:191–214 require owned messages, opaque owner/epoch revisions, stable retained positions, borrowed changes, and mutation bookkeeping without mutable collection escape. The implementation owns the message/revision vectors and latest-change `BTreeMap`, exposes immutable reads, coalesces edits, preserves positions across prefix removal, and resets epochs for clear/prepend. Public module registration is the required export wiring. **Evidence:** `UI/message_history.rs:19–32,51–85,88–146,156–255`; `UI/lib.rs:13–16`. |
| **L2 — Main/child notification and history policy** — D:102 | `UI/state.rs`; `UI/subagent_ui.rs` | **MATCH** | Domain notification, receipt, tool, and adoption policy remain here; physical layout does not. Both tool maps contain stable positions, and updates verify the resolved message’s tool ID. Main pruning retires only matching removed lookups; clear empties the map. Occupied child adoption prepends through the owner and rebuilds positions with later IDs winning. Live revisions and the displayed cutoff are mutation-owned as amended in D:262–264. **Evidence:** `UI/state.rs:209–225,1266–1353,1518–1544,1893–1896,3575–3591`; `UI/subagent_ui.rs:8–38,79–172`. |
| **L3 — Private physical layout owner** — D:103 | `UI/widgets/chat_layout/mod.rs`; `UI/widgets/chat_layout/index.rs`; `UI/widgets/chat_layout/wrap.rs`; `UI/widgets/mod.rs` | **MATCH** | D:74–81 requires layout to own prepared blocks, physical rows, indexing, seeking, traversal, and borrowed painting. The current owner does all of these: `PreparedBlock<P>` binds payload and rows; `Layout<P>` owns indexed/edge blocks and `RowHeights`; preparation and visible painting reside here. The wrapper retains source-range descriptors and contains the approved carry repair and attribution. Private registration is in `widgets/mod.rs`. **Evidence:** `chat_layout/mod.rs:27–85,130–205,209–341`; `index.rs:9–160`; `wrap.rs:1–45,65–118,122–270`; `UI/widgets/mod.rs:1–4`. |
| **L4 — Chat presentation and transcript selection** — D:104 | `UI/widgets/chat.rs`; `UI/widgets/chat/presentation.rs`; `UI/widgets/chat/retained.rs` | **MATCH** | Chat owns semantic presentation, concrete source selectors, selected-transcript policy, invalidation coordination, rows-back policy, and F12 gutter composition—not physical aggregation/index traversal. `Presentation` owns selectors/generated/shared backing, while `Preparation::finish` delegates physical preparation to layout. Retention owns one `Layout<Presentation>`, not parallel payload/row/index collections. **Evidence:** `chat.rs:1–63`; `presentation.rs:18–129,148–190,303–733`; `retained.rs:70–81,121–235,274–375`; approved seams at D:70–72,216–226 and D:304–308. |
| **L5 — Markdown derivation and immutable sharing** — D:105 | `UI/widgets/markdown.rs` | **MATCH** | The real public result and cache values are `Arc<[Line<'static>]>`; cache hits/insertion clone handles rather than line trees. Content/width/consumed-style hashing stays in derivation, and the production chat consumer retains that allocation during preparation. No transcript owner or App lifetime migrated into Markdown. **Evidence:** `markdown.rs:17–18,33–85`; `presentation.rs:278–300`; D:66–72. |
| **L6 — Root retained-render interface and geometry wiring** — D:106 | `UI/render.rs` | **MATCH** | `RenderState` privately owns chat state, the public draw seam receives immutable UI state plus mutable render state, and caught panics invalidate retained state before fallback. Existing `frame_rows`/`frame_areas` remain the chat geometry source. No wrapping or height-index algorithm was added here. **Evidence:** `render.rs:8–53,56–170,191–220`; D:54–64,216–222. |
| **L7 — Application lifetime wiring** — D:107 | `APP` | **MATCH** | App stores one render state beside `UiState`, constructs it once, and passes it through `draw_frame`. The production call is not creating a fresh cache per frame. Row selection, source derivation, and revision bookkeeping remain below App. **Evidence:** `APP:76–78,465–467,949–978`; D:54–64. |
| **L8 — Typed Core source domain and approved receipt metadata exception** — D:108 | `CORE` | **MATCH** | `HarvestedReceipts` contains ordered receipts plus conservative earliest-removal metadata and consumes through `into_parts`. Successful splices update the minimum offset; Core does not interpret display revisions or depend on rendering types. The UI performs that interpretation. **Evidence:** `CORE:77–92,164–199`; `UI/state.rs:1518–1544`; explicit approval at D:260–264. |
| **L9 — Read-only renderer contract and authoritative tool projections** — D:109 | `UI/traits.rs` | **MATCH** | `TuiState` exposes history/live revisions without mutable history access. Borrowed/derived tool projections, public `Cow` adaptations, direct recorded-source resolution, and indexed first-Diff/cache views reside in the expressly approved existing owner. No prepared-row, index, or wrap implementation was placed here. **Evidence:** `traits.rs:48–60,373–395,407–489,576–731`; D:216–226,258. |

**Coverage:** 9 approved rows; 16 changed production files assigned once; **9 MATCH, 0 MISMATCH, 0 UNCOVERED, 0 MISSING**.

The changed files `UI/theme.rs`, `UI/workflow_ui.rs`, and `UI/widgets/powers_panel.rs` have test-side changes, not additional production ownership changes. Their hunks respectively begin inside the existing tests at approximately lines 1273, 135, and 261. Test and benchmark files are therefore not miscounted as additional production modules.

## 2. Responsibility mismatch assessment

### Physical ownership: no remaining mismatch

This was the most important claim to verify directly.

**Approved:** layout owns prepared blocks, row aggregation, height indexing, seek/traversal, and painting; chat supplies presentation and transcript policy (D:74–81,103–104,222).

**Observed:**

- `Presentation` contains source slots and shared Markdown, but no physical rows (`presentation.rs:45–49`).
- `Preparation::finish` invokes `chat_layout::prepare` and returns a layout-owned `PreparedBlock<Presentation>` (`presentation.rs:728–733`).
- `Layout<P>` owns the leading block, indexed deque, height index, and trailing blocks (`chat_layout/mod.rs:130–139`).
- Its mutation methods update physical storage and heights together; its paint method performs the indexed seek and visible traversal (`chat_layout/mod.rs:147–173,209–308`).
- Chat retention decides what changed and what window to request, then calls that owner (`retained.rs:121–154,337–365`).

The current implementation is not a painter-only layout module with the actual index/traversal retained in chat. The reconstruction’s central ownership claim is supported by the working source.

### Core and traits growth: approved responsibilities, not uncovered drift

Core’s splice-offset result is expressly covered by D:108,260. UI display-cutoff/revision decisions remain in `state.rs:1518–1544`; Core only reports parsing/buffer-transform facts.

Likewise, the substantive tool projections in `traits.rs` are covered by D:109,226,258. They would exceed a “trait declarations only” owner, but that is not the approved ledger. The actual forbidden work—physical row preparation, wrapping, retained layout ownership, or new Myers derivation—is not implemented there.

### Presentation versus derivation: no duplicate engine

Chat interprets message kinds and styles source-backed content (`presentation.rs:303–725`). Markdown parsing remains in `markdown.rs:79–425`. Diff derivation remains the existing `DiffPreview::new`/`TextDiff` path (`UI/tool_diff.rs:39–111`); presentation consumes its rows, and visible Diff resolution uses recorded content/row ordinals (`presentation.rs:89–103,554–586`). This matches D:70–72,104–105,258 rather than creating a second parser or Myers path.

## 3. Interface and consuming-side dispatch assessment

The following checks trace new or changed boundary values into their consumers, rather than stopping at their producers.

| Boundary value/interface | Verified consumer and disposition |
|---|---|
| `HistoryChanges { reset, retained, changed }` | `retained.rs:121–154` handles reset explicitly, retires removed prefixes, appends new stable positions chronologically, and replaces pre-existing edited positions. Latest-edit order is not incorrectly treated as transcript order. Matches D:44–52. |
| Stable `MessagePosition` tool-map values | Main and child lookup paths validate both retention and the requested tool ID before mutation (`state.rs:1266–1291`; `subagent_ui.rs:137–157`). Adoption rebuilds positions (`subagent_ui.rs:84–97`). Matches D:46. |
| Independent text/thought `LiveRevision` values | `retained.rs:157–235,308–333` has separate text/thought preparation keys and separate activity preparation. History/geometry reset clears these keys before reuse. Matches D:48,72,224. |
| `LogicalLine`, `PreparedRows`, `PreparedBlock<P>` | `chat_layout::prepare` consumes logical source tokens/styles, wraps them, and the returned block binds the physical rows to the opaque presentation payload (`chat_layout/mod.rs:27–85`). `Layout` consumes those blocks for append/replacement and painting. Matches D:70–81,222. |
| `BlockLocation::{Leading, Indexed, Trailing}` | The consuming match in `retained.rs:346–364` resolves indexed blocks against the selected history; thought/text tail slots and generated-only slots have explicit handling. There is no unmatched location silently discarded. |
| Concrete presentation `Source` variants | `Sources::external` and `Presentation::source_text` route static/generated/shared Markdown, message/plan/steer/tool/Diff, derived-slice, and live-thought sources (`presentation.rs:18–129`). No retained pointer into a movable history string is stored. Matches D:70–72,222. |
| `ToolText::{Borrowed, Derived}` and `ToolTextSource` | Both variants are consumed in presentation, and all recorded source variants resolve directly in `traits.rs:697–714`. Public output/error methods adapt the same decoder to `Cow`, rather than retaining a second allocating implementation (`traits.rs:576–589,666–714`; `presentation.rs:623–725`). Matches D:226. |
| First-Diff `(ordinal, source, preview)` | Preparation records the content ordinal; visible resolution directly indexes that content and cached preview row (`traits.rs:467–473`; `presentation.rs:89–103,520–586`). Ordinal changes do not unnecessarily redefine equivalent Diff source identity (`traits.rs:424–429`). Matches D:258. |
| `HarvestedReceipts` | The UI consumes `into_parts`, updates cutoff/revision conservatively, and applies returned receipts in order (`state.rs:1518–1544`). The metadata is not produced and then ignored. Matches D:260–264. |
| Shared Markdown result | Preparation borrows parsed spans into selector descriptors and retains the returned Arc (`presentation.rs:278–300`). It does not deep-clone the line collection. Matches D:66–72. |
| Public `RenderState` and `LayoutMetrics` | App passes its retained state through the public root; root forwards private chat state and invalidates it on panic (`APP:966–969`; `render.rs:18–53,220`). Benchmark metrics enumeration occurs outside timed drawing (`crates/cyril-ui/benches/conversation_render.rs:479–528`). Matches D:216–224. |

**Interface mismatch result: none.** The old `messages_version` path was absent from the inspected source inventory; the remaining `messages()` read method belongs to `MessageHistory`, not an old trait compatibility shim.

## 4. Dependency and seam mismatch assessment

The verified production direction is:

**App → root rendering → chat coordination/presentation → physical layout**, with presentation separately consuming **tracked-tool projections/DiffPreview** and **Markdown derivation**. Notification owners mutate **MessageHistory** and live revisions; Core receipt parsing supplies transform metadata to UI state.

This matches D:54–81,101–111,216–226.

### Candidate: does `Layout<P>` introduce the forbidden generic source service?

**Disposition: closed as conforming private payload ownership.**

The approved design forbids a generic source-provider service or renderer strategy family (D:70,111,222), not an opaque payload type used to keep backing and rows under one owner. The observed production specialization is `Layout<Presentation>` (`retained.rs:75`). Layout does not call a provider trait, interpret `ChatMessage`, choose source policy, or select among implementations; it receives per-visible-block closures (`chat_layout/mod.rs:209–217`; `retained.rs:341–364`).

The parameterization prevents a reverse dependency on chat and avoids parallel collections whose replacement/drop lifetimes could diverge. It is not evidence of multiple production adapters or permission to grow a future generic rendering framework.

### Candidate: module-reference cycle between history and traits

**Disposition: closed as a type-definition dependency, not a responsibility inversion.**

`message_history` stores the existing UI `ChatMessage`, while `TuiState` names the history owner (`message_history.rs:11,79–85`; `traits.rs:10,48–56`). History does not invoke `TuiState`, route events, or import layout/theme. That is consistent with the concrete UI-owned history contract in D:26,101.

### Candidate: 0.3.0 adaptation provenance versus the current released reference

**Disposition: closed as baseline/reference binding, not a new parity exception.**

D:85 and D:278 describe the original 0.3.0 source investigation. Current production explicitly distinguishes that adaptation provenance from the installed fidelity reference, widgets 0.3.2/core 0.1.2 (`chat_layout/wrap.rs:6–17`; `chat_layout/mod.rs:3–12`). The checked-in lock has those current versions (`Cargo.lock:2411–2414,2465–2468`), and the manifests/lock have no change in either reviewed diff layer.

I also inspected the installed 0.3.2 production wrapper and painter: both use `CellWidth`; its word-wrapper overflow/whitespace flow corresponds to the adaptation, apart from Cyril’s separately approved carry correction. The governing requirements are preservation of current cells and actual pinned Paragraph comparisons (D:10–21,87,95). Thus this does not authorize a new dependency, broader wrapping semantics, or another fidelity waiver. The historical source-version wording should not be cited as a claim that this worktree runs 0.3.0.

## 5. Protected-parent mismatch assessment

| Protected/existing parent | Approved allowance | Verified placement | Result |
|---|---|---|---|
| `APP` | Lifetime ownership and draw wiring only; no selection, derivation, or history bookkeeping (D:107). | Field, initialization, and draw argument at `APP:76–78,465–467,966–969`. | **No mismatch** |
| `UI/render.rs` | Concrete render state, fallback, and exact frame geometry; no layout/index bodies (D:106). | Private retained state and panic reset at `render.rs:18–53`; existing frame geometry feeds chat at `render.rs:91–170,191–220`. | **No mismatch** |
| `UI/state.rs`, `UI/subagent_ui.rs` | Notification/lifecycle/history policy; use history mutation methods, not layout/index bodies (D:102). | Stable-ID lookup validation, revision/cutoff ownership, receipt reconciliation, and adoption remain domain-state work. Actual physical algorithms are in layout. | **No mismatch** |
| `UI/traits.rs` | Read-only contract and explicitly approved authoritative projections; no retained physical layout (D:109). | Source decoding/resolution and cached first-Diff views at `traits.rs:373–489,576–731`; no row/index/wrap structures. | **No mismatch** |

The chat parent’s reduction to a facade plus its existing approval Diff helper is delegation within the approved owner, not an abandoned responsibility (`chat.rs:18–63`; D:104). Its child modules remain assigned to that same ledger row.

## 6. Explicit tradeoff dispositions and residual risks

These are not additional approvals or claims that executable qualification passed.

| Candidate/tradeoff | Governing design and production evidence | Disposition and residual risk |
|---|---|---|
| **Owned wrapping implementation, including F11** | D:87,288–296; `wrap.rs:122–270`. | **Approved tradeoff retained.** The carry check uses already measured pending widths before another positive-width symbol is appended. It is not runtime clipping or an incoming-inclusive replacement algorithm. Cyril owns fidelity maintenance; attribution and independent reference coverage remain necessary. |
| **Unsafe-reference test instrumentation** | D:292–296; `wrap.rs:327–448,450–480,583–741`; `presentation.rs:781–859`. | **Conforming, with distinct evidentiary roles.** Legacy mode classifies original unsafe starts and checks real Paragraph row count; corrected rows receive conservation/bounds/style checks and literal goldens. Per-row Paragraph painting is not misrepresented as an independent wrapping oracle. These tests were read, not run. |
| **Generated crossing-grapheme backing** | D:230–234; `presentation.rs:192–276`. | **Approved derivation exception retained.** Only crossing-boundary bytes join generated affixes; ordinary bodies remain selector-backed. Pathological graphemes can own arbitrarily many bytes, so no fixed bytes-per-cell or copy-size claim follows. |
| **Cold focus/width/theme transitions** | D:10,64,81; `retained.rs:121–154,157–235`. | **Approved cost boundary retained.** Only one transcript is retained. Width/theme/identity changes rebuild; height is not part of the geometry invalidation key. Returning to a previous transcript may be cold. |
| **Conservative invalidation of non-rendered ID binding** | D:48 says ID-only binding need not invalidate; `state.rs:1484–1512`; history coalescing at `message_history.rs:166–184`. | **Conforming conservative choice.** Binding uses the ordinary edit seam and can reprepare that changed block. This does not introduce a full-history invalidation or contradict a requirement that ID binding must be free. |
| **White-box storage/selector tests** | D:101,103,224,226; `message_history.rs:268–297`; `retained.rs:467–537,623–655`; `chat_layout/mod.rs:509–575`; `tool_text_tests.rs:19–102`. | **Supplementary coupling, not a replacement interface.** Pointer/metadata assertions establish ownership and reuse properties unavailable from pixels alone. They increase refactor coupling, but root tests also drive real notifications and the public draw seam (`conversation_performance_tests.rs:30–46,477–543,1045–1130,1251–1352`). |
| **Forwarding chat facade** | D:104,216; `chat.rs:18–63`. | **Retained naming/visibility boundary, not a claimed deep adapter.** The forwarding function is thin; the module also owns the approval Diff styling helper. No second production renderer implementation is claimed. |
| **Storage accounting versus total memory** | D:224; `retained.rs:88–118`; benchmark `conversation_render.rs:519–528`. | **Scope remains explicit.** Metrics enumerate prepared descriptors and owned/shared text outside drawing. `owned_text_bytes` is not allocator capacity, process RSS, or a complete heap accounting result. |

## 7. Non-MATCH and unresolved-candidate register

- **MISMATCH:** none.
- **UNCOVERED production modules:** none.
- **MISSING approved owners/interfaces:** none.
- **Unresolved design-conformance candidates:** none after the dispositions above.

This conclusion does not adopt the design document’s recorded gate results or native judgments. It concerns the directly inspected production partition, interfaces, dependencies, seams, and protected-parent placement only.

## Final comparison

**PASS — current working production maps all 16 changed production files to the nine approved ledger rows, with the physical-layout owner restored and no unresolved responsibility, interface, dependency/seam, or protected-parent drift identified.**