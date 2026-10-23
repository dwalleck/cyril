# Current-production reconstruction

## Summary

The production change separates authoritative conversation state, presentation preparation, and indexed physical layout:

- `MessageHistory` owns committed messages, stable retained positions, and change cursors; application state owns live-source revisions and notification-to-message mutation. (`crates/cyril-ui/src/message_history.rs:19-32`, `crates/cyril-ui/src/message_history.rs:74-85`, `crates/cyril-ui/src/state.rs:41-52`)
- Chat retention selects the main or focused-subagent transcript, interprets revisions, and prepares changed presentation blocks. It does **not** own the physical-row index or the cell-painting implementation in the current working files. (`crates/cyril-ui/src/widgets/chat/retained.rs:70-81`, `crates/cyril-ui/src/widgets/chat/retained.rs:121-154`, `crates/cyril-ui/src/widgets/chat/retained.rs:274-375`)
- `chat_layout` owns prepared physical rows together with their opaque presentation payloads, the retained block collection, height indexing, visible-window traversal, and painting. Presentation supplies source-selection closures. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:27-85`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-173`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-341`)
- Application code retains one `RenderState` across draws; root rendering passes its private chat state down and invalidates it after a caught rendering panic. (`crates/cyril/src/app.rs:76-78`, `crates/cyril/src/app.rs:465-467`, `crates/cyril/src/app.rs:966-969`, `crates/cyril-ui/src/render.rs:18-53`)

This is a production reconstruction only, not a design-conformance verdict.

## Evidence scope and changed-file census

Read-only Git inspection established:

| Item | Observed value |
|---|---|
| HEAD | `c0846f994994575683dd9d1568ed08f003859886` |
| `merge-base origin/main HEAD` | `03838a501dad8be140acd78dc3fff4cae35fafef` |
| Committed crate delta | 31 files, including tests and benchmark code |
| Runtime-source files with production changes | 16, enumerated below |
| Other non-test-named source files changed only inside tests | `theme.rs`, `workflow_ui.rs`, `widgets/powers_panel.rs` |
| Working-tree production delta relative to HEAD | `widgets/chat/presentation.rs`, `widgets/chat/retained.rs`, `widgets/chat_layout/mod.rs` |
| Untracked files under `crates/` | None reported |

The census used merge-base-to-HEAD diffs **and** HEAD-to-working-tree diffs, with `GIT_OPTIONAL_LOCKS=0`, `--no-ext-diff`, and `--no-textconv`. All production descriptions and line anchors below refer to the **working files**, including the three uncommitted files—not their HEAD versions.

The supplied qualification descriptor was not independently examined or used to infer intended behavior. No change-artifact contents, design/spec/plan/checkpoint documents, ADRs, prior reviews, PR descriptions, or issue text were consulted. Tests were inspected only for interface coupling. No builds, tests, linters, formatters, or benchmark runs were performed.

# Report

## 1. Changed module interfaces and responsibilities

### Files with production changes

| Path | Actual responsibility and interface |
|---|---|
| `crates/cyril-core/src/types/steer_receipt.rs` | Receipt parsing now exposes `HarvestedReceipts::into_parts(self) -> (Vec<SteerReceipt>, Option<usize>)`. `harvest(&mut String)` removes complete receipts and returns emission-ordered receipts plus the earliest removal offset. `withheld_tail(&str) -> usize` identifies an incomplete receipt suffix; its contract assumes harvesting has already occurred. These are parsing/buffer-offset operations, not rendering operations. (`crates/cyril-core/src/types/steer_receipt.rs:77-92`, `crates/cyril-core/src/types/steer_receipt.rs:164-224`) |
| `crates/cyril-ui/src/message_history.rs` | Sole mutable owner of committed display messages and revision metadata. Its public read interface comprises `messages`, `revision`, `retained_range`, `get`, and `changed_since`; construction is through `new`, `Default`, or `From<Vec<ChatMessage>>`. Crate-visible mutation comprises `push`, closure-based `edit`, `drop_prefix`, `clear`, and `prepend`. Positions survive append/edit/prefix eviction; owner or epoch mismatch requests reset. (`crates/cyril-ui/src/message_history.rs:74-146`, `crates/cyril-ui/src/message_history.rs:156-255`) |
| `crates/cyril-ui/src/traits.rs` | Defines the renderer-facing `TuiState` interface, including borrowed `history`, independent `live_revision`, live text/thought accessors, and `Option<usize>` scroll-back. Also owns the display projection over concrete core tool calls: public `Cow` output/error views and internal `ToolText`/`ToolTextSource` selection and resolution. Diff derivation stays attached to `TrackedToolCall` through `OnceLock<Arc<DiffPreview>>`. (`crates/cyril-ui/src/traits.rs:48-60`, `crates/cyril-ui/src/traits.rs:144-148`, `crates/cyril-ui/src/traits.rs:372-489`, `crates/cyril-ui/src/traits.rs:576-737`) |
| `crates/cyril-ui/src/state.rs` | Concrete main-session `TuiState` adapter and notification-driven state owner. Owns `MessageHistory`, live revisions, raw live text and displayed cutoff, and tool-ID-to-stable-position lookup. Tool observations insert or edit retained rows; live text appends harvest receipts, compute the displayed cutoff, and bump text revision when the visible source can change. Scroll offsets use `usize`. (`crates/cyril-ui/src/state.rs:41-52`, `crates/cyril-ui/src/state.rs:201-225`, `crates/cyril-ui/src/state.rs:1266-1318`, `crates/cyril-ui/src/state.rs:1515-1544`, `crates/cyril-ui/src/state.rs:3542-3565`) |
| `crates/cyril-ui/src/subagent_ui.rs` | `SubagentStream` owns a child transcript, live text/revision, activity, and tool-position lookup. Its read interface exposes history, live revision, streaming text, latest agent text, and activity. Notification handling and stream adoption mutate through history methods. `SubagentUiState` owns the stream map and focus/removal interface. (`crates/cyril-ui/src/subagent_ui.rs:8-76`, `crates/cyril-ui/src/subagent_ui.rs:79-172`, `crates/cyril-ui/src/subagent_ui.rs:176-210`, `crates/cyril-ui/src/subagent_ui.rs:240-291`) |
| `crates/cyril-ui/src/widgets/chat/presentation.rs` | Concrete semantic presentation preparation. `Preparation` maps messages, tools, thoughts, activity, and labels to logical styled lines and source slots; `finish(sources, width)` returns `PreparedBlock<Presentation>`. `Presentation::source_text(slot, sources)` resolves generated/static/shared-Markdown backing or authoritative borrowed source text. Source matching and byte-range validity are invariants, not recoverable errors. (`crates/cyril-ui/src/widgets/chat/presentation.rs:18-55`, `crates/cyril-ui/src/widgets/chat/presentation.rs:58-129`, `crates/cyril-ui/src/widgets/chat/presentation.rs:148-190`, `crates/cyril-ui/src/widgets/chat/presentation.rs:278-304`, `crates/cyril-ui/src/widgets/chat/presentation.rs:728-733`) |
| `crates/cyril-ui/src/widgets/chat/retained.rs` | Transcript selection, presentation invalidation, fixed-gutter geometry, scroll-start selection, and scrollbar composition. `ChatRenderState` retains history/geometry/live/activity keys and a `Layout<Presentation>`. Its rendering entry accepts frame, area, `&dyn TuiState`, theme, and mutable retained state. It prepares/replaces blocks and supplies authoritative source contexts to layout painting. (`crates/cyril-ui/src/widgets/chat/retained.rs:70-85`, `crates/cyril-ui/src/widgets/chat/retained.rs:121-235`, `crates/cyril-ui/src/widgets/chat/retained.rs:274-375`) |
| `crates/cyril-ui/src/widgets/chat_layout/mod.rs` | Physical preparation and retained-window rendering owner. `prepare(logical_lines, width, source_text)` creates `PreparedRows`. `PreparedBlock<P>` binds opaque payload and physical rows to one replacement/drop owner. `Layout<P>` owns leading/indexed/four trailing blocks and the height index; its interface supports append, replacement, prefix discard, edge replacement, totals, metrics, and indexed painting with a per-visible-block source factory. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:27-85`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-205`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-341`) |
| `crates/cyril-ui/src/widgets/chat_layout/index.rs` | Text-independent physical-row-height index. `RowHeights` exposes `push`, `set`, `discard_prefix`, `total`, and `locate`, using retained-relative block indices. It implements Fenwick-tree updates/seeks and tombstone compaction; it does not own source text or prepared rows. (`crates/cyril-ui/src/widgets/chat_layout/index.rs:1-24`, `crates/cyril-ui/src/widgets/chat_layout/index.rs:26-126`, `crates/cyril-ui/src/widgets/chat_layout/index.rs:129-160`) |
| `crates/cyril-ui/src/widgets/chat_layout/wrap.rs` | Source-range word wrapping. `wrap_lines(&[SourceLine], width) -> Vec<WrappedRow>` consumes borrowed styled source spans and returns rows containing source IDs, UTF-8 byte ranges, styles, widths, and alignment—not copied bodies. Original span boundaries survive run coalescing. Width zero yields no rows. (`crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-118`, `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:122-238`, `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:241-270`) |
| `crates/cyril-ui/src/widgets/markdown.rs` | Markdown parsing/styling and immutable shared-result caching. Public `render_with_theme(&str, usize, &Theme) -> Arc<[Line<'static>]>` projects the consumed style, hashes content/width/style, and shares cached or newly derived lines. The cache is process-global, mutex-protected, and capacity 256; an unavailable lock causes uncached computation rather than propagating a cache error. (`crates/cyril-ui/src/widgets/markdown.rs:10-18`, `crates/cyril-ui/src/widgets/markdown.rs:33-85`) |
| `crates/cyril-ui/src/widgets/chat.rs` | Thin chat entry and module visibility facade: re-exports crate-visible `ChatRenderState`, forwards `render` to `retained::render`, and retains a separate borrowed diff-line styling helper used for approval previews. It no longer contains the transcript preparation implementation. (`crates/cyril-ui/src/widgets/chat.rs:1-26`, `crates/cyril-ui/src/widgets/chat.rs:29-63`) |
| `crates/cyril-ui/src/render.rs` | Public retained rendering lifetime and whole-frame composition. `RenderState` privately owns chat state, exposes `new` and storage metrics, and must be passed to `draw(frame, state, render_state)` across frames. `draw` catches panics, invalidates possibly partial preparation, and paints fallback output. `draw_inner` resolves frame theme, paints canvas, computes frame areas, and invokes chat with its retained state. (`crates/cyril-ui/src/render.rs:8-53`, `crates/cyril-ui/src/render.rs:191-220`) |
| `crates/cyril/src/app.rs` | Application lifetime owner and terminal adapter wiring. `App` stores `RenderState`, initializes it once, and passes it to root rendering from `draw_frame`; `draw_frame` also updates state with actual terminal dimensions and maps backend failures into core transport errors. (`crates/cyril/src/app.rs:76-78`, `crates/cyril/src/app.rs:465-467`, `crates/cyril/src/app.rs:949-978`) |
| `crates/cyril-ui/src/lib.rs` | Crate registration/export wiring: exports `message_history` alongside the existing render/state modules; separately registers test-only performance and tool-text modules. (`crates/cyril-ui/src/lib.rs:4-7`, `crates/cyril-ui/src/lib.rs:13-24`) |
| `crates/cyril-ui/src/widgets/mod.rs` | Widget namespace wiring. Registers `chat_layout` privately beside public `chat`; it adds no layout policy itself. (`crates/cyril-ui/src/widgets/mod.rs:1-8`) |

### Changed source files whose production sections are unchanged

The committed comparison showed identical pre-test production sections for these three files. Their existing production roles still matter to the dependency map:

| Path | Existing production role; changed-file classification |
|---|---|
| `crates/cyril-ui/src/theme.rs` | Owns resolved `Theme`, `HighlightStyle`, and `MarkdownStyle` projection. The latter includes direct Markdown colors and nested highlighting inputs. Changes in this file are test-side, not a new runtime theme/cache responsibility. (`crates/cyril-ui/src/theme.rs:386-466`, `crates/cyril-ui/src/theme.rs:887-888`) |
| `crates/cyril-ui/src/workflow_ui.rs` | Owns workflow-session routing/storage and stream adoption, reusing concrete `SubagentStream`. A vacant adoption moves the stream; occupied adoption invokes `absorb_earlier`. This production logic is unchanged; changed tests exercise its interaction with the new history interface. (`crates/cyril-ui/src/workflow_ui.rs:19-79`, `crates/cyril-ui/src/workflow_ui.rs:100-101`, `crates/cyril-ui/src/workflow_ui.rs:144-175`) |
| `crates/cyril-ui/src/widgets/powers_panel.rs` | The changed hunk is in its test drawing helper, which now creates/passes `RenderState`; no powers-widget production responsibility was added. (`crates/cyril-ui/src/widgets/powers_panel.rs:201-204`, `crates/cyril-ui/src/widgets/powers_panel.rs:263-267`) |

## 2. Responsibility clusters and separation rule

### A. Authoritative message and live-source state

**Members:** receipt parser, history owner, `UiState`, and `SubagentStream`.

- The parser owns receipt extraction and suffix classification; `UiState` combines those results with its own displayed-cutoff and revision state. (`crates/cyril-core/src/types/steer_receipt.rs:164-224`, `crates/cyril-ui/src/state.rs:1515-1544`)
- History owns mutation bookkeeping. Its edit index keeps the latest revision per retained message, removing superseded change entries; changed-position iteration is therefore latest-change order, not necessarily transcript order. (`crates/cyril-ui/src/message_history.rs:132-146`, `crates/cyril-ui/src/message_history.rs:166-184`)
- Main and child state translate notifications into those history mutations. Main state validates tool IDs against retained positions before editing; child state performs the equivalent validation for its own stream. (`crates/cyril-ui/src/state.rs:1266-1291`, `crates/cyril-ui/src/subagent_ui.rs:137-157`)

**Separation rule:** authoritative content and mutation facts live here; wrapped rows, viewport lookup, and painting do not. The state fields are message/live/tool lookup data, whereas physical rows and the height index are fields of layout. (`crates/cyril-ui/src/state.rs:41-52`, `crates/cyril-ui/src/subagent_ui.rs:8-15`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-139`)

### B. Concrete semantic presentation

**Members:** tool display projection in `traits.rs`, `chat/presentation.rs`, and Markdown derivation.

- Tool-text precedence is selected once into a typed source selector; subsequent resolution follows the selector instead of repeating precedence or serialization. (`crates/cyril-ui/src/traits.rs:372-395`, `crates/cyril-ui/src/traits.rs:582-658`, `crates/cyril-ui/src/traits.rs:721-737`)
- Presentation chooses labels, colors, indentation, tool summaries, output truncation, thought appearance, and message-kind formatting. (`crates/cyril-ui/src/widgets/chat/presentation.rs:303-445`, `crates/cyril-ui/src/widgets/chat/presentation.rs:447-586`, `crates/cyril-ui/src/widgets/chat/presentation.rs:623-725`)
- Source slots borrow authoritative content by selector/range. Owned presentation backing is generated strings and shared Markdown lines; affix preparation may retain a genuinely derived grapheme crossing a generated-prefix/body/suffix join. (`crates/cyril-ui/src/widgets/chat/presentation.rs:18-49`, `crates/cyril-ui/src/widgets/chat/presentation.rs:113-145`, `crates/cyril-ui/src/widgets/chat/presentation.rs:192-276`)
- Presentation hands logical lines to layout at `finish`; it does not itself maintain the height tree or traverse the viewport. (`crates/cyril-ui/src/widgets/chat/presentation.rs:728-733`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:135-139`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-264`)

**Separation rule:** this cluster knows *what* conversation content means and how it is styled; layout knows source tokens, ranges, styles, widths, and positions without interpreting message kinds. (`crates/cyril-ui/src/widgets/chat/presentation.rs:303-304`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:27-31`, `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-102`)

### C. Retention and transcript coordination

**Member:** `chat/retained.rs`.

- It chooses focused child versus main history, live revision, thought source, and display name. Workflow streams are not selected by this rendering branch. (`crates/cyril-ui/src/widgets/chat/retained.rs:289-307`)
- It resets on owner/epoch invalidation or content-width/theme change. Otherwise it discards retired blocks, appends new positions chronologically, then replaces previously retained edited positions. (`crates/cyril-ui/src/widgets/chat/retained.rs:121-154`)
- Live text, thought, name-dependent labels, and activity have separate preparation keys; unchanged live text is not fetched through the state accessor. (`crates/cyril-ui/src/widgets/chat/retained.rs:157-235`, `crates/cyril-ui/src/widgets/chat/retained.rs:308-333`)
- It reserves the indicator column independently of overflow, calculates main browse/follow or child-bottom start, and supplies source contexts only when layout requests an intersecting block. (`crates/cyril-ui/src/widgets/chat/retained.rs:281-288`, `crates/cyril-ui/src/widgets/chat/retained.rs:337-375`)

**Separation rule:** retention decides *which blocks need preparation and which window to request*; physical storage/index traversal and cell writes are delegated to layout. (`crates/cyril-ui/src/widgets/chat/retained.rs:121-154`, `crates/cyril-ui/src/widgets/chat/retained.rs:337-366`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-341`)

### D. Physical layout and painting

**Members:** `chat_layout/mod.rs`, `index.rs`, and `wrap.rs`.

- Wrapping converts borrowed logical source spans into physical source-range runs. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:39-64`, `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-118`)
- Layout owns payload and physical rows together, preventing independently maintained payload/row collections. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:66-85`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-173`)
- The index stores only height metadata and supports logarithmic point operations/seeks, constant-time totals, and occasional linear metadata compaction. (`crates/cyril-ui/src/widgets/chat_layout/index.rs:1-18`, `crates/cyril-ui/src/widgets/chat_layout/index.rs:26-126`, `crates/cyril-ui/src/widgets/chat_layout/index.rs:138-160`)
- Layout finds the first indexed visible row, walks intersecting blocks, resolves their visible runs, and paints graphemes using `CellWidth`. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-308`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:310-341`)

**Separation rule:** physical layout owns geometry and source addressing, but not source meaning. The payload is generic, and sources arrive through closures. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-139`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-217`)

### E. Frame/application composition and lifetime

**Members:** `app.rs`, `render.rs`, and namespace facades.

The application owns persistent rendering lifetime; root rendering owns full-frame composition and panic recovery; chat’s facade forwards to its concrete retained renderer. None of those callers implements word wrapping or Fenwick indexing. (`crates/cyril/src/app.rs:76-78`, `crates/cyril/src/app.rs:949-978`, `crates/cyril-ui/src/render.rs:18-53`, `crates/cyril-ui/src/render.rs:191-220`, `crates/cyril-ui/src/widgets/chat.rs:18-26`)

## 3. Dependency direction and concrete adapters

### Architecture

```text
App
  owns UiState + RenderState
  └─ render::draw(&dyn TuiState, &mut RenderState)
       └─ chat::render
            └─ retained::render
                 ├─ reads main MessageHistory / focused SubagentStream
                 ├─ Presentation preparation
                 │    ├─ TrackedToolCall source selection / DiffPreview
                 │    └─ Markdown → highlighting / shared cache
                 └─ Layout<Presentation>
                      ├─ source-range wrapping
                      ├─ RowHeights index
                      └─ visible-block source closures → cell painter

Notifications
  ├─ UiState → MessageHistory mutation + live revisions
  │    └─ core steer_receipt parser
  └─ SubagentStream → MessageHistory mutation + live revisions
       └─ moved/adopted by workflow stream store
```

Edges are implemented at `crates/cyril/src/app.rs:949-978`, `crates/cyril-ui/src/render.rs:191-220`, `crates/cyril-ui/src/widgets/chat.rs:18-26`, `crates/cyril-ui/src/widgets/chat/retained.rs:239-254`, `crates/cyril-ui/src/widgets/chat/retained.rs:289-366`, `crates/cyril-ui/src/widgets/chat/presentation.rs:278-300`, `crates/cyril-ui/src/widgets/chat/presentation.rs:520-586`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:39-64`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:135-139`, `crates/cyril-ui/src/state.rs:1518-1544`, and `crates/cyril-ui/src/workflow_ui.rs:61-73`.

### Concrete adapters and seam ownership

| Seam | Actual adapters/dependencies |
|---|---|
| Renderer state input: `TuiState` | Production `UiState`; test-only `MockTuiState`. The production implementation returns its owned history and live revisions. The mock supplies the same interface with explicit live-revision helpers. (`crates/cyril-ui/src/state.rs:201-225`, `crates/cyril-ui/src/traits.rs:1404-1438`) |
| Child transcript input | Concrete `SubagentStream`, not a second `TuiState` implementation. Retention selects and reads it directly. (`crates/cyril-ui/src/subagent_ui.rs:8-38`, `crates/cyril-ui/src/widgets/chat/retained.rs:289-307`) |
| Layout source acquisition | Retention creates a per-block `Sources` context and returns a closure calling `Presentation::source_text`. Layout knows only `BlockLocation`, opaque payload, and slot resolver. (`crates/cyril-ui/src/widgets/chat/retained.rs:347-365`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-217`) |
| Tool display projection | Concrete `TrackedToolCall` wraps core `ToolCall`; typed selectors resolve its authoritative JSON/content fields. There is no injected decoder implementation at this seam. (`crates/cyril-ui/src/traits.rs:405-417`, `crates/cyril-ui/src/traits.rs:721-737`) |
| Diff derivation | Existing concrete `DiffPreview` uses `similar::TextDiff`; `TrackedToolCall` lazily owns/shares it and detaches the cache when the effective diff source changes. (`crates/cyril-ui/src/tool_diff.rs:5-6`, `crates/cyril-ui/src/tool_diff.rs:39-47`, `crates/cyril-ui/src/traits.rs:423-431`, `crates/cyril-ui/src/traits.rs:458-475`) |
| Markdown derivation | Concrete `pulldown_cmark::Parser`, shared `HashCache`, and syntax highlighting. Highlighting is concrete Syntect-backed code. (`crates/cyril-ui/src/widgets/markdown.rs:4-18`, `crates/cyril-ui/src/widgets/markdown.rs:79-86`, `crates/cyril-ui/src/widgets/markdown.rs:226-230`, `crates/cyril-ui/src/highlight.rs:4-13`) |
| Terminal rendering | App drawing is generic over Ratatui `Backend`; inspected tests and benchmark use `TestBackend`. This is an existing concrete backend substitution point, distinct from conversation layout policy. (`crates/cyril/src/app.rs:949-955`, `crates/cyril-ui/src/conversation_performance_tests.rs:30-46`, `crates/cyril-ui/benches/conversation_render.rs:479-495`) |

There is a Rust module-reference cycle at the type-definition level: `message_history` stores `traits::ChatMessage`, while `traits::TuiState` names `MessageHistory` and `LiveRevision`. This is not a runtime call cycle; the history implementation itself operates on the message type, not on `TuiState`. (`crates/cyril-ui/src/message_history.rs:11`, `crates/cyril-ui/src/message_history.rs:79-85`, `crates/cyril-ui/src/traits.rs:10`, `crates/cyril-ui/src/traits.rs:48-56`)

## 4. Pass-through modules and hypothetical seams

| Module/seam | Reconstruction |
|---|---|
| `chat::render` | A genuine forwarding function: identical rendering inputs are handed directly to `retained::render`. Deleting this function would remove naming/visibility indirection, not rendering complexity. The file as a whole is not entirely pass-through because it still styles approval-preview diff lines. (`crates/cyril-ui/src/widgets/chat.rs:18-63`) |
| `lib.rs`, `widgets/mod.rs` | Namespace/export wiring, not behavioral modules. Their changed production declarations expose history and privately register layout. (`crates/cyril-ui/src/lib.rs:13-16`, `crates/cyril-ui/src/widgets/mod.rs:1-4`) |
| `RenderState::layout_metrics` | A forwarding accessor, but `RenderState` earns a separate application-facing interface by hiding private chat state and participating in panic invalidation. (`crates/cyril-ui/src/render.rs:18-53`) |
| `WorkflowUiState::apply_notification` | Thin per-session creation/routing to concrete `SubagentStream`. The store as a whole additionally owns adoption and workflow activity aggregation; it is not merely an alias around a child stream. Production behavior is unchanged in this change. (`crates/cyril-ui/src/workflow_ui.rs:33-43`, `crates/cyril-ui/src/workflow_ui.rs:61-90`) |
| `Layout<P>` and source closures | A meaningful internal seam: callers assign semantic meaning while layout owns rows, indexing, storage, and painting. The observed production payload is `Presentation`; tests instantiate `String`. A second production payload or alternate layout implementation is **undetermined from production code**, so multiple production adapters must not be claimed. (`crates/cyril-ui/src/widgets/chat/retained.rs:75`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-139`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:208-217`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:349-368`) |
| `TuiState` | Real production/test substitution exists (`UiState`/mock), but the inspected code does not establish multiple production state adapters. Child streams are concrete reads, not evidence of a second implementation. (`crates/cyril-ui/src/state.rs:201-225`, `crates/cyril-ui/src/traits.rs:1424-1438`, `crates/cyril-ui/src/widgets/chat/retained.rs:289-307`) |
| Alternate parser/wrapper/index/cache strategies | No injected strategy interface exists in the reconstructed path: preparation calls the concrete wrapper, layout owns concrete `RowHeights`, and Markdown uses concrete parser/cache code. Alternative implementations would be hypothetical seams, not current adapters. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:24-25`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:61-63`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:135-139`, `crates/cyril-ui/src/widgets/markdown.rs:17-18`, `crates/cyril-ui/src/widgets/markdown.rs:85-86`) |

## 5. Existing-parent growth: wiring versus responsibility

The normative set of “protected parents” is **undetermined from production code**. The following classifies changed existing parents by actual code placement, without assuming an approved protection list.

| Existing parent | Wiring versus substantive responsibility |
|---|---|
| `crates/cyril/src/app.rs` | **Wiring/lifetime ownership.** Adds the retained state field, initialization, and extra root-draw argument. Layout, cache invalidation decisions, wrapping, and row seeking remain below the app. (`crates/cyril/src/app.rs:76-78`, `crates/cyril/src/app.rs:465-467`, `crates/cyril/src/app.rs:966-969`) |
| `crates/cyril-ui/src/render.rs` | **Wiring plus render-lifetime/error-recovery responsibility.** Adds public retained state/metrics and panic-path invalidation; forwards the private chat state during frame composition. It does not absorb the physical index or wrapping implementation. (`crates/cyril-ui/src/render.rs:8-53`, `crates/cyril-ui/src/render.rs:191-220`) |
| `crates/cyril-ui/src/state.rs` | **Substantive state-management implementation, not just plumbing.** Stable-position tool lookup validates identity, merges observed tool frames, retires lookup entries on eviction, and clears session-specific lookup on session change. Receipt processing owns the displayed cutoff and live-text invalidation. These remain within application display-state ownership rather than physical layout. (`crates/cyril-ui/src/state.rs:920-924`, `crates/cyril-ui/src/state.rs:1266-1303`, `crates/cyril-ui/src/state.rs:1515-1544`, `crates/cyril-ui/src/state.rs:3575-3590`) |
| `crates/cyril-ui/src/traits.rs` | **Interface migration plus substantive display-source projection.** History/live/scroll signatures change caller obligations. Typed tool selectors and borrowed/derived views centralize decoding and later source resolution. This is more than trait declaration wiring. (`crates/cyril-ui/src/traits.rs:48-56`, `crates/cyril-ui/src/traits.rs:144-145`, `crates/cyril-ui/src/traits.rs:372-403`, `crates/cyril-ui/src/traits.rs:582-737`) |
| `crates/cyril-ui/src/subagent_ui.rs` | **Substantive mutation bookkeeping within existing stream ownership.** Uses history mutation and live revisions; occupied adoption prepends history and rebuilds tool positions with later rows winning repeated IDs. (`crates/cyril-ui/src/subagent_ui.rs:79-113`, `crates/cyril-ui/src/subagent_ui.rs:116-172`) |
| `crates/cyril-ui/src/widgets/chat.rs` | **Responsibility removal/delegation.** Current production code is the render facade plus approval diff helper; transcript preparation is in `presentation.rs`, coordination in `retained.rs`, and physical machinery in `chat_layout`. (`crates/cyril-ui/src/widgets/chat.rs:1-63`, `crates/cyril-ui/src/widgets/chat/presentation.rs:303-304`, `crates/cyril-ui/src/widgets/chat/retained.rs:274-288`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-139`) |
| `crates/cyril-ui/src/widgets/markdown.rs` | **Shared-result ownership/cache behavior.** Public results are immutable `Arc` slices and cache hits share backing; consumed style forms the cache identity. Parsing remains here rather than migrating into retained chat. (`crates/cyril-ui/src/widgets/markdown.rs:33-85`) |
| `crates/cyril-core/src/types/steer_receipt.rs` | **Parser-result enrichment.** Adds removal-offset information to receipt harvesting; the UI, not core parsing, decides how that affects displayed-source revision. (`crates/cyril-core/src/types/steer_receipt.rs:77-92`, `crates/cyril-core/src/types/steer_receipt.rs:170-199`, `crates/cyril-ui/src/state.rs:1518-1544`) |
| `lib.rs`, `widgets/mod.rs` | **Registration only.** No new behavioral ownership. (`crates/cyril-ui/src/lib.rs:13-24`, `crates/cyril-ui/src/widgets/mod.rs:1-8`) |
| `theme.rs`, `workflow_ui.rs`, `widgets/powers_panel.rs` | **No production growth in the committed delta.** Their changes are test-side: theme source-coupling checks, workflow history/adoption checks, and retained-render test plumbing respectively. (`crates/cyril-ui/src/theme.rs:2031-2057`, `crates/cyril-ui/src/workflow_ui.rs:144-175`, `crates/cyril-ui/src/widgets/powers_panel.rs:263-267`) |

An important current-working placement distinction is that `ChatRenderState` holds a `Layout<Presentation>`, **not parallel row/payload/index collections**. The layout module is a behavioral owner, not the small painter-only module represented by its committed HEAD prefix. (`crates/cyril-ui/src/widgets/chat/retained.rs:70-81`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:66-85`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-173`)

## 6. Tests reaching past interfaces

These are coupling observations only; test expectations were not treated as intended-design authority.

### White-box or cross-module internal coupling

| Test location | Interface crossed or implementation detail reached |
|---|---|
| History unit tests | Inspect private `latest_changes` and revision vectors, construct a cursor with an impossible sequence, and assert metadata/storage lengths. This couples tests to history bookkeeping beyond its public changed-since/read interface. (`crates/cyril-ui/src/message_history.rs:268-297`) |
| Main-state unit tests | Read and deliberately corrupt private `tool_call_index` entries, then drive notifications. This reaches behind `UiState`’s notification/read interface. (`crates/cyril-ui/src/state.rs:3776-3782`) |
| Child-stream unit tests | Call private `flush_streaming_text`, inspect live revisions, and corrupt private tool lookup entries. Other cases access the store’s private `streams` map. (`crates/cyril-ui/src/subagent_ui.rs:321-338`, `crates/cyril-ui/src/subagent_ui.rs:342-365`, `crates/cyril-ui/src/subagent_ui.rs:425-429`) |
| Retained renderer unit tests | Call private synchronization methods; inspect geometry; traverse the test-only `layout.block(...).rows()` interface; compare physical-row allocation pointers; and resolve presentation payloads directly. This binds tests to cache-retention/storage identity, not only rendered cells. (`crates/cyril-ui/src/widgets/chat/retained.rs:386-410`, `crates/cyril-ui/src/widgets/chat/retained.rs:467-537`) |
| Presentation unit tests | Import wrapper test helpers, inspect `PreparedBlock` physical rows, and invoke layout’s row painter directly. Ownership tests inspect private source slots/ranges and compare resolved pointers. This crosses presentation into physical-layout internals. (`crates/cyril-ui/src/widgets/chat/presentation.rs:771-859`, `crates/cyril-ui/src/widgets/chat/presentation.rs:1271-1324`) |
| Chat shape/style tests | Test-only helpers call `Preparation` methods and `into_lines` rather than the public root draw interface. `into_lines` is explicitly test-only and materializes a snapshot from production preparation; it is not a second production renderer. (`crates/cyril-ui/src/widgets/chat.rs:78-122`, `crates/cyril-ui/src/widgets/chat/presentation.rs:735-763`) |
| Height-index unit tests | Assert private tombstone counts and backing-vector lengths around compaction. This is internal representation coupling beyond `locate`/`total` behavior. (`crates/cyril-ui/src/widgets/chat_layout/index.rs:256-272`) |
| Tool-text tests | Import crate-private `ToolText`/`ToolTextSource`, call selection/resolution methods directly, and assert authoritative allocation-pointer identity. These test the internal selector interface rather than only public `Cow` output. (`crates/cyril-ui/src/tool_text_tests.rs:3-5`, `crates/cyril-ui/src/tool_text_tests.rs:19-102`) |
| Root performance tests | Drive real `UiState` and root `render::draw`, but additionally consume private chat/Markdown/wrap/diff work counters. Behavioral execution crosses the normal root seam; work attribution reaches implementation instrumentation. (`crates/cyril-ui/src/conversation_performance_tests.rs:18-22`, `crates/cyril-ui/src/conversation_performance_tests.rs:30-46`, `crates/cyril-ui/src/conversation_performance_tests.rs:124-153`) |
| Markdown tests | Inject local concrete cache instances into private `render_with_cache`, derive keys directly, inspect entries and `Arc` identity, and exercise the poisoned-cache path. These reach behind `render_with_theme`. (`crates/cyril-ui/src/widgets/markdown.rs:1017-1028`, `crates/cyril-ui/src/widgets/markdown.rs:1031-1046`, `crates/cyril-ui/src/widgets/markdown.rs:1125-1146`) |
| Theme and widget-source tests | Read source text and enforce import/color-token patterns and an exact recursive module inventory. These couple to source spelling, module paths, and conventional test-module markers rather than solely callable interfaces. (`crates/cyril-ui/src/theme.rs:1273-1305`, `crates/cyril-ui/src/theme.rs:2031-2092`, `crates/cyril-ui/tests/widget_theme_sources.rs:4-28`, `crates/cyril-ui/tests/widget_theme_sources.rs:133-155`, `crates/cyril-ui/tests/widget_theme_sources.rs:280-342`) |
| Chrome widget tests | Invoke crate-private chat drawing and instantiate `ChatRenderState` directly rather than composing through root `RenderState`. (`crates/cyril-ui/src/chrome_theme_tests.rs:607-610`) |

### Tests using exposed interfaces rather than private renderer storage

- Missing-tool-update integration tests drive notifications, inspect public history/change results, and render through public `RenderState`/`draw`. Their message inspection uses the exposed history/message-kind interface. (`crates/cyril-ui/tests/missing_tool_updates.rs:39-55`, `crates/cyril-ui/tests/missing_tool_updates.rs:195-224`)
- Failed-validation integration tests use public history, `TrackedToolCall::error_message`, and root rendering. (`crates/cyril-ui/tests/v2_failed_validation.rs:17-21`, `crates/cyril-ui/tests/v2_failed_validation.rs:81-105`)
- Workflow adoption tests use stream-store access, public history cursors, and notifications to observe preservation/reset behavior. They rely on the concrete reused stream type, but do not require private layout storage. (`crates/cyril-ui/src/workflow_ui.rs:144-175`, `crates/cyril-ui/src/workflow_ui.rs:192-212`, `crates/cyril-ui/src/workflow_ui.rs:236-259`)
- Floor/approval tests pass retained root state into drawing; the benchmark similarly owns a terminal plus public `RenderState` and reads public layout metrics outside its draw timer. (`crates/cyril-ui/src/floor_tests.rs:17-24`, `crates/cyril-ui/src/always_allow_tests.rs:505-509`, `crates/cyril-ui/benches/conversation_render.rs:479-528`)
- Changed application/event-routing assertions now traverse `history().messages()` rather than the former message accessor. Application unit tests also access the enclosing app’s private `ui_state`, but do not thereby reach into layout internals. (`crates/cyril/src/app/tests/current_runtime_contract/ordering.rs:8-10`, `crates/cyril/src/app/tests/current_runtime_contract/ordering.rs:75-80`, `crates/cyril/src/app/tests/review.rs:102-108`, `crates/cyril/tests/event_routing.rs:62-66`)

The mock has public message/live fixture fields and explicit revision-bumping setters. Consequently fixture code has more mutation freedom than production callers borrowing `&MessageHistory`; keeping live text and revision consistent is part of fixture construction. This is an interface-coupling obligation, not evidence that a particular fixture is incorrect. (`crates/cyril-ui/src/traits.rs:1313-1317`, `crates/cyril-ui/src/traits.rs:1404-1438`, `crates/cyril-ui/src/message_history.rs:110-128`)

## 7. Reconstructed ownership ledger

“Interface owner” below means the production module defining and enforcing the seam, not an organizational code owner.

### Changed-production ledger

| Path | Responsibility | Interface owner |
|---|---|---|
| `crates/cyril-core/src/types/steer_receipt.rs` | Receipt extraction and earliest-removal/suffix information | Core receipt parser: `HarvestedReceipts`, `harvest`, `withheld_tail`. (`crates/cyril-core/src/types/steer_receipt.rs:77-92`, `crates/cyril-core/src/types/steer_receipt.rs:164-224`) |
| `crates/cyril-ui/src/message_history.rs` | Committed-message ownership, stable positions, revision/change lifecycle | `MessageHistory` and its opaque position/revision types. (`crates/cyril-ui/src/message_history.rs:13-25`, `crates/cyril-ui/src/message_history.rs:74-85`, `crates/cyril-ui/src/message_history.rs:110-236`) |
| `crates/cyril-ui/src/traits.rs` | Renderer input contract and tool display-source projection | `TuiState`; concrete `TrackedToolCall` selector/view interface. (`crates/cyril-ui/src/traits.rs:48-60`, `crates/cyril-ui/src/traits.rs:372-417`, `crates/cyril-ui/src/traits.rs:576-737`) |
| `crates/cyril-ui/src/state.rs` | Main-session notification projection, history mutation, live cutoff/revisions, tool positions, scroll state | `UiState` mutation methods and `TuiState` implementation; history bookkeeping delegated to `MessageHistory`. (`crates/cyril-ui/src/state.rs:201-225`, `crates/cyril-ui/src/state.rs:1266-1318`, `crates/cyril-ui/src/state.rs:1515-1544`, `crates/cyril-ui/src/state.rs:3542-3590`) |
| `crates/cyril-ui/src/subagent_ui.rs` | Child-stream mutation/adoption and stream focus/store | `SubagentStream` and `SubagentUiState`. (`crates/cyril-ui/src/subagent_ui.rs:8-38`, `crates/cyril-ui/src/subagent_ui.rs:79-172`, `crates/cyril-ui/src/subagent_ui.rs:240-291`) |
| `crates/cyril-ui/src/widgets/chat/presentation.rs` | Semantic formatting, authoritative source selectors, generated/shared backing | `Preparation`, `Presentation`, and `Sources`; returns layout-owned `PreparedBlock<Presentation>`. (`crates/cyril-ui/src/widgets/chat/presentation.rs:18-55`, `crates/cyril-ui/src/widgets/chat/presentation.rs:113-152`, `crates/cyril-ui/src/widgets/chat/presentation.rs:728-733`) |
| `crates/cyril-ui/src/widgets/chat/retained.rs` | Transcript selection, invalidation, preparation coordination, gutter/scrollbar/window policy | `ChatRenderState` and retained render entry; physical storage delegated to `Layout`. (`crates/cyril-ui/src/widgets/chat/retained.rs:70-81`, `crates/cyril-ui/src/widgets/chat/retained.rs:121-235`, `crates/cyril-ui/src/widgets/chat/retained.rs:274-375`) |
| `crates/cyril-ui/src/widgets/chat_layout/mod.rs` | Physical block preparation/ownership, indexed window traversal, source acquisition, painting | `LogicalLine`, `PreparedRows`, `PreparedBlock<P>`, `Layout<P>`, source-resolver closures. (`crates/cyril-ui/src/widgets/chat_layout/mod.rs:27-85`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:130-217`, `crates/cyril-ui/src/widgets/chat_layout/mod.rs:276-341`) |
| `crates/cyril-ui/src/widgets/chat_layout/index.rs` | Height metadata, row-to-block lookup, prefix retirement/compaction | `RowHeights`. (`crates/cyril-ui/src/widgets/chat_layout/index.rs:9-24`, `crates/cyril-ui/src/widgets/chat_layout/index.rs:26-160`) |
| `crates/cyril-ui/src/widgets/chat_layout/wrap.rs` | Styled source-range word wrapping and physical run construction | `SourceSpan`, `SourceLine`, `RowRun`, `WrappedRow`, `wrap_lines`. (`crates/cyril-ui/src/widgets/chat_layout/wrap.rs:65-118`, `crates/cyril-ui/src/widgets/chat_layout/wrap.rs:241-270`) |
| `crates/cyril-ui/src/widgets/markdown.rs` | Markdown derivation and shared immutable result caching | `render_with_theme`; private concrete cache/parser implementation. (`crates/cyril-ui/src/widgets/markdown.rs:17-18`, `crates/cyril-ui/src/widgets/markdown.rs:33-85`) |
| `crates/cyril-ui/src/widgets/chat.rs` | Chat facade and approval diff styling | Forwarding `render`; `render_diff_lines`; implementation owned by retained/presentation/layout modules. (`crates/cyril-ui/src/widgets/chat.rs:6-63`) |
| `crates/cyril-ui/src/render.rs` | Public retained-render lifetime, whole-frame composition, panic recovery, metrics access | `RenderState`, `LayoutMetrics`, `draw`. (`crates/cyril-ui/src/render.rs:8-53`, `crates/cyril-ui/src/render.rs:191-220`) |
| `crates/cyril/src/app.rs` | Application ownership and terminal-to-renderer connection | `App::draw_frame`, consuming public `RenderState`/`draw`. (`crates/cyril/src/app.rs:76-78`, `crates/cyril/src/app.rs:465-467`, `crates/cyril/src/app.rs:949-978`) |
| `crates/cyril-ui/src/lib.rs` | Public module registration | UI crate root. (`crates/cyril-ui/src/lib.rs:13-24`) |
| `crates/cyril-ui/src/widgets/mod.rs` | Private layout-module registration | Widgets namespace. (`crates/cyril-ui/src/widgets/mod.rs:1-8`) |

### Changed-file context rows without production changes

| Path | Responsibility | Interface owner |
|---|---|---|
| `crates/cyril-ui/src/theme.rs` | Existing resolved appearance and consumed-style projection; changed tests inspect usage/source placement | Theme module; no new runtime ownership in this delta. (`crates/cyril-ui/src/theme.rs:386-466`, `crates/cyril-ui/src/theme.rs:2031-2057`) |
| `crates/cyril-ui/src/workflow_ui.rs` | Existing workflow stream store/adoption, using `SubagentStream`; changed history/adoption tests | `WorkflowUiState` for routing/store; `SubagentStream` for stream mutation. (`crates/cyril-ui/src/workflow_ui.rs:19-79`, `crates/cyril-ui/src/workflow_ui.rs:144-175`) |
| `crates/cyril-ui/src/widgets/powers_panel.rs` | Existing powers widget; changed test helper accommodates root retained rendering | Powers widget unchanged; test helper consumes `render::RenderState`. (`crates/cyril-ui/src/widgets/powers_panel.rs:201-204`, `crates/cyril-ui/src/widgets/powers_panel.rs:263-267`) |

### Matters undetermined from production

- Which modules were normatively designated protected parents, and their approved growth allowances.
- Whether the reconstructed ownership partition matches an approved feature ledger.
- Whether any single-production-adapter seam was approved specifically for future variation.
- Whether measured behavior satisfies the supplied qualification descriptor.

Those questions require separate evidence or design comparison; they do not alter the current-production ownership map above.