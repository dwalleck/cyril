# Frozen production-code reconstruction

## Summary

The implementation introduces **application-lifetime retained conversation rendering** backed by an independently revisioned message owner. Its principal boundaries are:

1. **Transcript ownership and mutation:** `MessageHistory`, populated by main-session and child-session state machines.
2. **Presentation preparation:** concrete source selectors, generated labels, and shared Markdown backing.
3. **Physical layout:** source-range wrapping, block-height indexing, and selected-row painting.
4. **Retained rendering coordination:** revision/geometry synchronization, transcript selection, scrolling, and live/activity blocks.
5. **Application integration:** one retained render state carried through the existing root draw path.

These are reconstructed code boundaries, not an assessment against an approved design. Formal “protected parent” status is **undetermined from production code**.

## Frozen revision and source scope

| Item | Observed value |
|---|---|
| Worktree | `/home/dwalleck/repos/cyril-wt-perf-cyril-c6la` |
| Full HEAD | `c0846f994994575683dd9d1568ed08f003859886` |
| `origin/main` | `03838a501dad8be140acd78dc3fff4cae35fafef` |
| Merge base | `03838a501dad8be140acd78dc3fff4cae35fafef` |
| Comparison | `03838a501dad8be140acd78dc3fff4cae35fafef..c0846f994994575683dd9d1568ed08f003859886` |

Scope was independently obtained using `git merge-base origin/main HEAD`, followed by `git diff --stat`, `--name-status`, `--numstat`, and selected source diffs. Git commands used `GIT_OPTIONAL_LOCKS=0`; diffs disabled external diff and text conversion.

The comparison contains **31 changed files under `crates/`, with 9,951 insertions and 1,308 deletions**. After separating runtime hunks from test-only hunks, **16 files contain production changes**; all 16 appear in the ledger below.

Important scope distinctions:

- `crates/cyril-ui/src/theme.rs` changes test-source fences, not production theme resolution or style projections. Its first changed hunk is within `mod tests`, at line 1273 onward; the relevant fence implementation is at `crates/cyril-ui/src/theme.rs:1273-1321`.
- `crates/cyril-ui/src/workflow_ui.rs` changes tests, not production workflow storage. Its first changed hunk is within tests at line 138; the production store remains at `crates/cyril-ui/src/workflow_ui.rs:18-99`.
- `crates/cyril-ui/src/widgets/powers_panel.rs` changes only its test draw helper, supplying `RenderState`; see `crates/cyril-ui/src/widgets/powers_panel.rs:262-268`.
- The benchmark is supporting measurement code, not an application production module: `crates/cyril-ui/benches/conversation_render.rs:62-78,365-373`.

A read-only `git diff --name-only HEAD -- crates` produced no paths, establishing no tracked source drift beneath `crates/` when the filesystem sources were read.

Only production source, necessary tests/benchmark code, and Git metadata/diffs were used. No change-artifact or design/specification/plan/checkpoint document contents were read. No builds, tests, linters, formatters, application runs, or native qualification were performed.

---

## 1. Changed module interfaces and responsibilities

### 1.1 Message ownership and change reporting

**`crates/cyril-ui/src/message_history.rs`**

`MessageHistory` owns committed `ChatMessage` data, per-message revisions, a latest-change index, stable retained positions, and an owner/epoch/sequence cursor. It is deliberately non-`Clone`; a new history gets a distinct identity. See `crates/cyril-ui/src/message_history.rs:13-33,73-107`.

Its public read interface is:

- `messages() -> &[ChatMessage]`
- `revision() -> HistoryRevision`
- `retained_range() -> Range<MessagePosition>`
- `get(MessagePosition) -> Option<(&ChatMessage, MessageRevision)>`
- `changed_since(HistoryRevision) -> HistoryChanges<'_>`

See `crates/cyril-ui/src/message_history.rs:110-150`.

Mutation is crate-private: `push`, `edit`, `drop_prefix`, `clear`, and `prepend`. Editing removes the previous change-index entry before recording the latest revision, so repeated edits coalesce. Prefix eviction preserves surviving positions; clear and nonempty prepend change the epoch. See `crates/cyril-ui/src/message_history.rs:153-239`.

`MessagePosition` is publicly named but its numeric field is crate-visible; `HistoryRevision` fields are private. Public consumers therefore obtain positions/cursors through the owner rather than constructing their representation. `LiveRevision` separately exposes text and thought counters; its bump methods are crate-private. See `crates/cyril-ui/src/message_history.rs:13-47`.

### 1.2 Main-session state adaptation

**`crates/cyril-ui/src/state.rs`**

`UiState` now stores a `MessageHistory`, independent live revisions, a displayed streaming-text cutoff, and a tool-ID-to-`MessagePosition` index. Its `TuiState` projection returns the history, the already-filtered live prefix, and the live counters. See `crates/cyril-ui/src/state.rs:41-52,209-225`.

Tool start/update notifications share `track_tool_call`. A valid existing index edits the retained row; otherwise the state flushes pending streams and inserts a new chronological row. The index is checked against the actual retained tool ID before mutation. Active-tool display remains a separate collection updated alongside history. See `crates/cyril-ui/src/state.rs:545-562,1266-1304`.

The state owns **when** revisions change:

- Agent and thought notification handling updates their respective sources.
- Flushes transfer pending text/thought into committed history.
- Receipt harvesting determines whether the displayed live prefix changed.
- Retention removes matching tool lookup entries before dropping the prefix.

See `crates/cyril-ui/src/state.rs:498-527,1309-1352,1518-1544,3575-3591`.

Session identity changes clear tool lookups and active-tool display while retaining old transcript data; `clear_messages` clears history and its lookup index. See `crates/cyril-ui/src/state.rs:913-925,1892-1896`.

### 1.3 Child-stream state adaptation

**`crates/cyril-ui/src/subagent_ui.rs`**

`SubagentStream` owns its own `MessageHistory`, live text/revision, tool-position index, and activity. Public consumers receive read-only `history`, `live_revision`, and `streaming_text` views. See `crates/cyril-ui/src/subagent_ui.rs:8-38`.

Child tool updates edit a row only after both position and tool-ID validation. Unlike the main-session path, an unknown child update is dropped rather than inserted. See `crates/cyril-ui/src/subagent_ui.rs:129-154`.

Occupied adoption prepends the earlier history and rebuilds tool positions chronologically; later duplicate IDs win because later inserts replace earlier map entries. See `crates/cyril-ui/src/subagent_ui.rs:78-97`.

### 1.4 UI read contract and tool text projections

**`crates/cyril-ui/src/traits.rs`**

`TuiState` exposes concrete revisioned history instead of a bare message slice, and exposes independent live revisions:

```rust
fn history(&self) -> &MessageHistory;
fn streaming_text(&self) -> &str;
fn streaming_thought(&self) -> Option<&str>;
fn live_revision(&self) -> LiveRevision;
```

See `crates/cyril-ui/src/traits.rs:46-60`.

`TrackedToolCall` owns the authoritative core tool call and a lazily initialized shared diff preview. `diff_view` returns the selected content ordinal, borrowed diff source, and cached preview together. See `crates/cyril-ui/src/traits.rs:417-495`.

Tool output selection has a concrete internal interface:

- `ToolText::Borrowed { source, text }`
- `ToolText::Derived(String)`
- `ToolTextSource::{RawOutput, RawField, ItemText, Content}`

See `crates/cyril-ui/src/traits.rs:380-414`.

Public `output_text` and `error_message` return `Option<Cow<'_, str>>`. Internal preparation selects a source once; `resolve_text_source` subsequently follows the recorded branch without repeating precedence selection. JSON serialization supplies genuinely derived owned text. See `crates/cyril-ui/src/traits.rs:568-652,668-738`.

### 1.5 Root renderer lifetime and diagnostics

**`crates/cyril-ui/src/render.rs`**

The public entry point now takes explicit retained state:

```rust
pub fn draw(
    frame: &mut Frame,
    state: &dyn TuiState,
    render_state: &mut RenderState,
)
```

`RenderState` owns private chat rendering state and exposes construction plus `layout_metrics()`. `LayoutMetrics` reports retained messages, rows, runs, source slots, and owned text bytes. See `crates/cyril-ui/src/render.rs:8-41`.

The existing panic wrapper now clears retained state before drawing the fallback, preventing reuse of a partially updated preparation. Root composition forwards the chat allocation and retained owner to the chat widget. See `crates/cyril-ui/src/render.rs:43-53,187-225`.

### 1.6 Chat facade and approval diff adapter

**`crates/cyril-ui/src/widgets/chat.rs`**

The production chat entry is a thin delegation to `retained::render`, with `ChatRenderState` re-exported crate-locally. See `crates/cyril-ui/src/widgets/chat.rs:6-27`.

The file also retains a substantive `render_diff_lines` adapter: it styles cached diff rows using the current theme and borrows their source text. Approval rendering calls this adapter independently of transcript completion/kind gates. See `crates/cyril-ui/src/widgets/chat.rs:29-63` and `crates/cyril-ui/src/widgets/approval.rs:580-584`.

### 1.7 Presentation preparation

**`crates/cyril-ui/src/widgets/chat/presentation.rs`**

`Preparation` converts domain display data into logical styled lines and finishes them into a `PreparedBlock`. A block retains:

- Source selectors and byte ranges.
- Generated strings where necessary.
- Shared `Arc<[Line<'static>]>` Markdown backing.
- Addressable wrapped rows.

See `crates/cyril-ui/src/widgets/chat/presentation.rs:18-56,113-160,736-757`.

`Sources` supplies the current authoritative message/tool/thought. Source resolution covers message bodies, plan titles, steer notes, tool-selected text, diff rows, live thought, generated backing, and shared Markdown spans. See `crates/cyril-ui/src/widgets/chat/presentation.rs:59-129`.

This module owns message/tool visual policy: role labels, plan status prefixes, steer status/note formatting, tool status/kind labels, diff rows, output summaries, and truncation labels. See `crates/cyril-ui/src/widgets/chat/presentation.rs:311-453,528-733`.

Affixed text has explicit grapheme-boundary handling so generated prefixes/suffixes can preserve segmentation without retaining a copied body. See `crates/cyril-ui/src/widgets/chat/presentation.rs:200-284`.

### 1.8 Retained synchronization and viewport rendering

**`crates/cyril-ui/src/widgets/chat/retained.rs`**

`ChatRenderState` owns the selected transcript’s retained preparation: geometry/theme key, history cursor/range, prepared message deque, row-height index, and independent header/live/thought/activity blocks. See `crates/cyril-ui/src/widgets/chat/retained.rs:47-64`.

History synchronization:

- Rebuilds on owner/epoch reset or width/theme change.
- Retires removed prefixes.
- Appends new chronological positions.
- Replaces only changed pre-existing positions.
- Updates the height index alongside blocks.

See `crates/cyril-ui/src/widgets/chat/retained.rs:116-165`.

Live text, thought, child-name labels, and activity use separate keys. Unchanged live text is not requested from the source accessor. See `crates/cyril-ui/src/widgets/chat/retained.rs:167-241,399-427`.

The render function selects main versus focused child history, reserves a stable indicator column when width permits, computes follow/back scrolling, paints the selected physical rows, and renders a scrollbar. Focused children follow their tail independently of main browse state. See `crates/cyril-ui/src/widgets/chat/retained.rs:369-454`.

Painting uses the height index to find the starting message and then traverses visible blocks/rows only. See `crates/cyril-ui/src/widgets/chat/retained.rs:244-304,325-349`.

### 1.9 Physical layout primitives

**`crates/cyril-ui/src/widgets/chat_layout/index.rs`**

`RowHeights` is a concrete Fenwick-tree index over block heights. Its interface is `len`, `push`, `set`, `discard_prefix`, `total`, and `locate`. It owns numeric height metadata, not messages, text, or wrapped rows. Prefix removal leaves tombstones until compaction rebuilds the numeric index. See `crates/cyril-ui/src/widgets/chat_layout/index.rs:1-18,21-161`.

**`crates/cyril-ui/src/widgets/chat_layout/wrap.rs`**

`wrap_lines` consumes logical `SourceLine`/`SourceSpan` values and returns `WrappedRow`/`RowRun` metadata containing source IDs, byte ranges, styles, widths, and alignment. See `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-117`.

The implementation performs grapheme-aware wrapping, retains span boundaries, filters according to Ratatui’s styled-grapheme iterator, and includes explicit carried-prefix handling. Temporary grapheme queues do not escape preparation; `finish_row` coalesces compatible contiguous runs within the same original span. See `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:122-270`.

**`crates/cyril-ui/src/widgets/chat_layout/mod.rs`**

This is not merely an index file: `paint_row` resolves retained source ranges and writes grapheme start cells into a Ratatui buffer, applying alignment and physical cell widths. See `crates/cyril-ui/src/widgets/chat_layout/mod.rs:14-55`.

### 1.10 Shared Markdown output

**`crates/cyril-ui/src/widgets/markdown.rs`**

The public renderer returns `Arc<[Line<'static>]>`. The concrete global cache stores that shared backing; a hit clones the `Arc`, while a miss derives lines and inserts shared ownership. Cache identity includes source content, width, and `MarkdownStyle`. See `crates/cyril-ui/src/widgets/markdown.rs:17-18,33-76`.

Parsing remains owned here through `pulldown_cmark`; highlighting is delegated to the existing highlighting adapter. See `crates/cyril-ui/src/widgets/markdown.rs:78-112,181-199`.

### 1.11 Receipt-harvest mutation metadata

**`crates/cyril-core/src/types/steer_receipt.rs`**

`harvest(&mut String)` now returns `HarvestedReceipts`, whose `into_parts` provides ordered receipts plus `Option<usize>` for the earliest removed start. The parser records the minimum splice position while removing valid markers. See `crates/cyril-core/src/types/steer_receipt.rs:77-93,164-200`.

The UI consumes that result to distinguish receipt-only/raw-buffer edits from changes affecting the displayed prefix. Parsing/removal metadata belongs to core; display cutoff and live invalidation remain in UI state. See `crates/cyril-ui/src/state.rs:1518-1544`.

### 1.12 Application and module registration

- `App` owns and initializes one `RenderState`, then supplies it on root draws: `crates/cyril/src/app.rs:76-78,465-467,965-969`.
- The crate registers public message history and test modules: `crates/cyril-ui/src/lib.rs:1-27`.
- Widgets register the private physical-layout module: `crates/cyril-ui/src/widgets/mod.rs:1-8`.

---

## 2. Responsibility clusters and the separating rule

| Cluster | Reconstructed separating rule | Evidence |
|---|---|---|
| Authoritative transcript storage | Own committed message identity, stable positions, and mutation/change accounting; do not own visual rows. | `crates/cyril-ui/src/message_history.rs:73-150,164-239` |
| Notification/state policy | Decide chronological insertion, edits, stream commits, tool identity, and live revision advancement. Delegate storage bookkeeping to history. | `crates/cyril-ui/src/state.rs:498-562,1266-1352`; `crates/cyril-ui/src/subagent_ui.rs:115-172` |
| Presentation policy | Decide labels, colors, display precedence, and source selectors; produce prepared blocks rather than selecting viewport positions. | `crates/cyril-ui/src/widgets/chat/presentation.rs:311-453,528-757` |
| Physical layout mechanics | Operate on logical spans, byte ranges, physical widths, and numeric heights without understanding tool/message kinds. | `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-117`; `crates/cyril-ui/src/widgets/chat_layout/index.rs:1-18`; `crates/cyril-ui/src/widgets/chat_layout/mod.rs:28-55` |
| Retention/viewport coordination | Connect source revisions to prepared blocks, invalidate/reuse them, and choose/paint the visible physical window. | `crates/cyril-ui/src/widgets/chat/retained.rs:116-165,244-304,369-454` |
| Shared derivations | Own independently reusable derived values: shared Markdown and theme-independent diff previews. | `crates/cyril-ui/src/widgets/markdown.rs:33-76`; `crates/cyril-ui/src/tool_diff.rs:33-126` |
| Root integration | Own retained-state lifetime and frame composition, not transcript layout algorithms. | `crates/cyril/src/app.rs:76-78,465-467,965-969`; `crates/cyril-ui/src/render.rs:18-53,187-225` |

Two distinctions are especially visible:

- **Committed versus live data:** history change cursors and text/thought revisions are independent, and retained blocks synchronize them independently. See `crates/cyril-ui/src/message_history.rs:22-47` and `crates/cyril-ui/src/widgets/chat/retained.rs:116-236`.
- **Authoritative versus derived text:** ordinary bodies remain source-selected; generated labels/serialized JSON and shared Markdown are owned derivations. See `crates/cyril-ui/src/widgets/chat/presentation.rs:18-49,113-129,669-733`.

---

## 3. Dependency direction and concrete adapters

The principal execution/data flow is:

```text
App
  → render::draw + RenderState
    → chat::render
      → retained synchronization / viewport selection
        → MessageHistory read/change interface
        → Preparation
          → TrackedToolCall text/diff projections
          → Markdown renderer
          → source-range wrapper
        → RowHeights
        → paint_row
          → Ratatui Buffer
```

Evidence: `crates/cyril/src/app.rs:965-969`; `crates/cyril-ui/src/render.rs:219-225`; `crates/cyril-ui/src/widgets/chat.rs:18-27`; `crates/cyril-ui/src/widgets/chat/retained.rs:116-165,307-349`; `crates/cyril-ui/src/widgets/chat/presentation.rs:286-308,736-757`.

Concrete adapters are:

- **`UiState → TuiState`:** read-only application projection, including filtered live source and revisions (`crates/cyril-ui/src/state.rs:201-225`).
- **`TrackedToolCall → ToolText/ToolTextSource`:** authoritative output/error decoding with later direct source resolution (`crates/cyril-ui/src/traits.rs:568-652,668-725`).
- **Presentation → Markdown:** concrete shared styled-line derivation (`crates/cyril-ui/src/widgets/chat/presentation.rs:286-308`).
- **Markdown → highlighting:** existing concrete syntax adapter, with structural style inputs (`crates/cyril-ui/src/widgets/markdown.rs:181-199`; `crates/cyril-ui/src/highlight.rs:25-38`).
- **Tool diff → `similar::TextDiff`:** theme-independent bounded diff derivation (`crates/cyril-ui/src/tool_diff.rs:33-109`).
- **Physical painter → Ratatui:** concrete buffer/cell/grapheme operations (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:17-55`).
- **Core receipt parser → UI invalidation policy:** core reports splice metadata; UI interprets it (`crates/cyril-core/src/types/steer_receipt.rs:77-93`; `crates/cyril-ui/src/state.rs:1518-1544`).

The direction is **not perfectly one-way at the Rust module/type level**:

1. `message_history` imports `traits::ChatMessage`, while `traits` imports `MessageHistory` and returns it from `TuiState`. This is an intra-crate bidirectional type dependency. See `crates/cyril-ui/src/message_history.rs:11` and `crates/cyril-ui/src/traits.rs:10,53`.
2. The retained child constructs the public parent’s `render::LayoutMetrics` type. See `crates/cyril-ui/src/widgets/chat/retained.rs:71-105` and `crates/cyril-ui/src/render.rs:8-16`.

There is no generic history-provider or pluggable layout interface in this path: the renderer consumes concrete `MessageHistory`, concrete preparation blocks, and concrete row-height/wrapping implementations. See `crates/cyril-ui/src/traits.rs:53` and `crates/cyril-ui/src/widgets/chat/retained.rs:9-14,48-64`.

---

## 4. Pass-through modules and hypothetical seams

| Surface | Reconstruction |
|---|---|
| `widgets/chat.rs::render` | A literal forwarding function to `retained::render`; no independent rendering decision is made there. `chat.rs` nevertheless also owns the real approval diff styling adapter, so the whole module is not pass-through. Evidence: `crates/cyril-ui/src/widgets/chat.rs:18-63`. |
| `RenderState::layout_metrics` | Forwards to chat state, but the containing type owns application-lifetime retained storage and the root owns panic recovery. Evidence: `crates/cyril-ui/src/render.rs:18-53`. |
| `lib.rs`, `widgets/mod.rs` | Registration surfaces, not behavioral seams. Evidence: `crates/cyril-ui/src/lib.rs:1-29`; `crates/cyril-ui/src/widgets/mod.rs:1-17`. |
| `chat_layout/mod.rs` | Owns the actual physical painter in addition to registering child modules; not an empty facade. Evidence: `crates/cyril-ui/src/widgets/chat_layout/mod.rs:14-55`. |
| `Preparation::into_lines` | Test-only adapter exposing logical presentation before retained wrapping, not a production extension seam. Evidence: `crates/cyril-ui/src/widgets/chat/presentation.rs:760-788`. |
| `paint_row` source callback | A concrete mechanism for resolving source IDs at paint time, not an injected renderer/service family. Evidence: `crates/cyril-ui/src/widgets/chat_layout/mod.rs:28-44`. |

**Undetermined from production code:** whether the chat forwarding function, metrics forwarding, or any other boundary was intended for a future alternate implementation. The observed path contains concrete implementations rather than such future adapters.

---

## 5. Parent-module growth: wiring versus responsibility

The following concerns existing parent/integration files; it does not assert their formal protection status.

| Parent | Observed change classification |
|---|---|
| `crates/cyril/src/app.rs` | **Wiring/lifetime ownership.** Adds the retained-state field, initializes it, and passes it to the existing root draw. The production diff introduces no wrapping/indexing algorithm here. Evidence: `:76-78,465-467,965-969`. |
| `crates/cyril-ui/src/render.rs` | **Wiring plus root-level ownership/recovery/diagnostics.** Adds `RenderState`, `LayoutMetrics`, forwarding into chat, and panic-path invalidation. This is more than a signature-only migration, but preparation and viewport algorithms remain in children. Evidence: `:8-53,219-225`. |
| `crates/cyril-ui/src/state.rs` | **Substantive state-machine responsibility, not mere wiring.** Owns stable tool lookup validation/insertion, live revision advancement, cutoff calculation, receipt reconciliation edits, session lookup retirement, and eviction integration. Evidence: `:913-925,1266-1352,1484-1544,1687-1723,3575-3591`. |
| `crates/cyril-ui/src/traits.rs` | **Interface plus concrete projection behavior.** Changes the read contract and adds borrowed/derived output selection and recorded-source resolution inside the existing tool wrapper. Evidence: `:48-60,380-414,568-738`. |
| `crates/cyril-ui/src/subagent_ui.rs` | **Existing stream responsibility adapted to revisioned ownership.** Child notification processing, validation, live revisions, and adoption position rebuilding remain here. Evidence: `:8-38,78-97,107-172`. |
| `crates/cyril-ui/src/widgets/chat.rs` | **Behavior moved out; facade narrowed.** Transcript rendering delegates to a retained child, while approval diff styling remains local. Evidence: `:6-63`. |
| `crates/cyril-ui/src/widgets/markdown.rs` | **Existing derivation/cache responsibility with changed ownership.** Shared immutable results replace copied result ownership; parser/cache policy remains here. Evidence: `:17-18,33-89`. |
| `crates/cyril-ui/src/lib.rs`, `crates/cyril-ui/src/widgets/mod.rs` | **Registration only.** Evidence: `lib.rs:1-27`; `widgets/mod.rs:1-8`. |
| `crates/cyril-ui/src/theme.rs`, `crates/cyril-ui/src/workflow_ui.rs`, `crates/cyril-ui/src/widgets/powers_panel.rs` | **No production growth in this comparison.** Changes are test fences, tests, or test draw migration; see scope distinctions above. |

For scale, Git’s whole-file deltas include tests: `app.rs +118/-64`, `render.rs +244/-72`, `state.rs +700/-272`, `traits.rs +312/-94`, and `chat.rs +201/-495`. These are **not production-only growth counts**.

---

## 6. Tests reaching past interfaces

These are source-observed couplings, not exercised verification results.

| Test area | Interface bypass or lower-level coupling |
|---|---|
| History owner tests | Inspect private `latest_changes` and revision-vector lengths, and forge a cursor sequence to test reset handling. This couples tests to storage/accounting representation. `crates/cyril-ui/src/message_history.rs:276-293`. |
| Main-state defensive-index tests | Directly corrupt private `tool_call_index` before sending an update. This tests a defensive invariant using state unreachable through the ordinary public mutation API. `crates/cyril-ui/src/state.rs:3768-3797`. |
| Child-stream unit tests | Read and corrupt the child’s private tool-position index to exercise wrong-ID validation. `crates/cyril-ui/src/subagent_ui.rs:335-369`. |
| Retained renderer tests | Call private synchronization methods, inspect prepared block/deque/index internals, and compare row allocation pointers to establish reuse. `crates/cyril-ui/src/widgets/chat/retained.rs:535-595,643-665`. |
| Presentation tests | Inspect private source slots/selectors and pointer identity; directly compare lower-level wrapping/painting to reference buffers. `crates/cyril-ui/src/widgets/chat/presentation.rs:807-887,1289-1339`. |
| Chat shape tests | Use test-only `Preparation::into_lines`, bypassing retained synchronization and physical wrapping to inspect presentation output. `crates/cyril-ui/src/widgets/chat.rs:78-124`; `crates/cyril-ui/src/widgets/chat/presentation.rs:760-788`. |
| Height-index tests | Inspect `retired`, underlying vector lengths, tree lengths, and lookup-step counters, in addition to testing public-to-module behavior. `crates/cyril-ui/src/widgets/chat_layout/index.rs:257-289,356-382`. |
| Tool text tests | Consume crate-private `ToolText`/selectors and use pointer equality against authoritative JSON strings; one test demonstrates that a recorded source resolves without reselection after precedence changes. `crates/cyril-ui/src/tool_text_tests.rs:3-5,43-105`. |
| Markdown cache tests | Inject private local caches, calculate private keys, inspect eviction contents, and assert `Arc` allocation identity. `crates/cyril-ui/src/widgets/markdown.rs:1017-1075`. |
| Root retained-state tests | Call private `RenderState::invalidate` directly. This establishes reset/rebuild behavior, but the shown test does not inject a panic during preparation. `crates/cyril-ui/src/render.rs:669-678`. |
| Performance tests | Drive real `UiState` notifications and public root draws, but observe crate-private counters across preparation, wrapping, Markdown, diff derivation, and index seeking. They also call the root’s private geometry helper. `crates/cyril-ui/src/conversation_performance_tests.rs:29-53,139-154,603-613`. |
| Mock renderer fixtures | Construct public mock fields directly, including live text and counters. Helpers exist to bump revisions, but tests can bypass them; that representation permits stale revision/source combinations if used incorrectly. `crates/cyril-ui/src/traits.rs:1311-1317,1404-1421`. |
| Source-level architecture/theme fences | Read source text, depend on file paths, function names, import spellings, and the conventional final test-module marker. These intentionally test implementation shape rather than runtime interfaces. `crates/cyril-ui/tests/widget_theme_sources.rs:4-28,43-61,132-152`; `crates/cyril-ui/src/theme.rs:1273-1321`. |

There are also root-level behavioral tests that **do not bypass private state for mutation**: for example, orphan tool updates are sent through `UiState::apply_notification`, inspected through read-only history, and rendered through `render::draw`. See `crates/cyril-ui/tests/missing_tool_updates.rs:38-65,68-110,188-208`.

---

## 7. Reconstructed production ledger

“Interface owner” identifies the observed owning type/module, not an approved-design row.

| Path | Responsibility | Interface owner |
|---|---|---|
| `crates/cyril-core/src/types/steer_receipt.rs` | Parse/remove receipt markers and return ordered receipts plus conservative splice metadata. Evidence: `:77-93,164-200`. | `steer_receipt::harvest`, `HarvestedReceipts::into_parts` |
| `crates/cyril-ui/src/lib.rs` | Register/export message history and register supporting tests. Evidence: `:1-27`. | UI crate root |
| `crates/cyril-ui/src/message_history.rs` | Own committed display messages, identity/epoch/revisions, stable retained positions, and coalesced changes. Evidence: `:73-150,164-239`. | `MessageHistory`; cursor/position/change types |
| `crates/cyril-ui/src/render.rs` | Own root retained-render lifetime, expose layout diagnostics, compose the frame, and reset retention on caught draw failure. Evidence: `:8-53,187-225`. | `RenderState`, `LayoutMetrics`, `draw` |
| `crates/cyril-ui/src/state.rs` | Translate main notifications/actions into ordered history mutations, tool lookup maintenance, displayed live-source changes, and revisions. Evidence: `:498-562,1266-1352,1518-1544,3575-3591`. | `UiState`; its `TuiState` implementation |
| `crates/cyril-ui/src/subagent_ui.rs` | Own child transcript/live state, validate indexed tool edits, and rebuild positions during occupied adoption. Evidence: `:8-38,78-97,115-172`. | `SubagentStream`, `SubagentUiState` |
| `crates/cyril-ui/src/traits.rs` | Define read-only UI state and display data; adapt authoritative tool values into borrowed/derived text and cached diff views. Evidence: `:48-60,380-495,568-738`. | `TuiState`, `TrackedToolCall`; internal `ToolText` selectors |
| `crates/cyril-ui/src/widgets/chat.rs` | Provide chat entry/re-export boundary and shared approval diff-row styling. Evidence: `:6-63`. | `chat::render`, `chat::render_diff_lines` |
| `crates/cyril-ui/src/widgets/chat/presentation.rs` | Convert message/tool/live data into styled source-selected prepared blocks and shared/generated backing. Evidence: `:18-56,113-160,311-453,528-757`. | `Preparation`, `PreparedBlock`, `Sources` |
| `crates/cyril-ui/src/widgets/chat/retained.rs` | Synchronize preparation by source revisions/geometry; select transcript and visible rows; paint retained/live/activity blocks and scrollbar. Evidence: `:47-64,116-241,244-349,369-454`. | `ChatRenderState`, retained `render` |
| `crates/cyril-ui/src/widgets/chat_layout/index.rs` | Index retained block heights and locate physical rows using numeric metadata. Evidence: `:1-18,21-161`. | `RowHeights` |
| `crates/cyril-ui/src/widgets/chat_layout/mod.rs` | Register physical-layout children and resolve/paint one selected wrapped row into Ratatui cells. Evidence: `:14-55`. | `paint_row` |
| `crates/cyril-ui/src/widgets/chat_layout/wrap.rs` | Convert logical source spans into addressable physical-row source ranges, preserving grapheme/span/style semantics. Evidence: `:65-117,122-270`. | `wrap_lines`, `SourceLine`, `SourceSpan`, `WrappedRow`, `RowRun` |
| `crates/cyril-ui/src/widgets/markdown.rs` | Derive styled Markdown lines and cache/share immutable results by content, width, and structural style. Evidence: `:17-18,33-89`. | `render_with_theme`; private cache/derivation functions |
| `crates/cyril-ui/src/widgets/mod.rs` | Register the private physical-layout module within the widget namespace. Evidence: `:1-8`. | Widget module root |
| `crates/cyril/src/app.rs` | Own one retained render state for the application and carry it through root draws. Evidence: `:76-78,465-467,965-969`. | `App` |

**Necessary unchanged collaborators, not additional changed-production rows:** workflow storage/adoption (`crates/cyril-ui/src/workflow_ui.rs:18-99`), diff derivation (`crates/cyril-ui/src/tool_diff.rs:33-126`), structural theme projections (`crates/cyril-ui/src/theme.rs:424-461`), syntax highlighting (`crates/cyril-ui/src/highlight.rs:25-38`), and approval consumption of chat diff rows (`crates/cyril-ui/src/widgets/approval.rs:580-584`).

This is the complete frozen reconstruction for the stated revision and comparison; it contains no approved-design comparison or conformance verdict.