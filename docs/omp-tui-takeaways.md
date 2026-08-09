# TUI Takeaways from Oh My Pi

## Scope

Cyril should not replace Ratatui or copy Oh My Pi's TypeScript implementation. The useful lessons are rendering contracts, terminal lifecycle discipline, state ownership, and terminal-level verification.

Cyril already has strong equivalents for several Oh My Pi patterns:

- Ratatui `TestBackend` tests and render snapshots.
- Adaptive redraw cadence: 50 ms while streaming or running tools, 1 s while idle.
- Layered input routing for global shortcuts, modal overlays, autocomplete, and normal input.
- Typed ACP events, tool identifiers, and process cleanup.
- Panic-safe fallback rendering.

The recommendations below focus on gaps rather than replacing those foundations.

## Prioritized recommendations

### 1. Retain and version transcript blocks

Oh My Pi treats transcript entries as individually versioned blocks. Finalized blocks reuse their prior rendered rows; only the first changed block and the blocks after it are reassembled.

Cyril currently rebuilds one `Vec<Line>` from every retained message on every draw in `crates/cyril-ui/src/widgets/chat.rs`. Streaming text and thoughts are rendered separately after the committed messages. Cyril already maintains `messages_version`, but the chat renderer does not consume it. The markdown cache also clones its cached `Vec<Line>` values on hits.

Introduce an explicit transcript block model:

```rust
struct TranscriptBlock {
    id: TranscriptBlockId,
    revision: u64,
    phase: TranscriptPhase,
    content: TranscriptContent,
}

enum TranscriptPhase {
    Live,
    Final,
}
```

Cache rendered blocks by:

```text
(block id, revision, width, theme revision)
```

A finalized block should become an immutable render artifact. During a stream, only the live assistant, thought, or tool block should invalidate. Prefer shared cached data such as `Arc<[Line<'static>]>` or rendering cached lines by reference rather than cloning large vectors.

This should be benchmark-gated. Use a representative fixture with the maximum retained transcript, an active stream, tool updates, and Cyril's 50 ms busy cadence. The hypothesis is lower frame time and fewer allocations; do not introduce a complex retained cache unless the benchmark confirms the benefit.

Relevant Cyril files:

- `crates/cyril-ui/src/widgets/chat.rs`
- `crates/cyril-ui/src/widgets/markdown.rs`
- `crates/cyril-ui/src/cache.rs`
- `crates/cyril-ui/src/state.rs`

Relevant Oh My Pi files:

- `packages/coding-agent/src/modes/components/transcript-container.ts`
- `packages/tui/src/tui.ts`
- `docs/tui-runtime-internals.md`

### 2. Establish one display-safety seam

Oh My Pi routes terminal output through centralized helpers for tab replacement, cell-width truncation, path shortening, preview limits, and error formatting.

Cyril has display-width helpers in `crates/cyril-ui/src/text.rs`, but their use is not universal. User, system, command, and tool text can enter individual render paths directly, and some previews use local character limits.

Add one protocol-to-presentation seam:

```rust
fn sanitize_display_text(raw: &str, policy: DisplayPolicy) -> DisplayText;
```

It should:

- Remove or visibly encode terminal control sequences from untrusted text.
- Normalize tabs consistently.
- Bound pathological line lengths before rendering.
- Route all measuring, wrapping, slicing, padding, and truncation through one Unicode cell-width model.
- Shorten home and workspace paths where full paths add no value.
- Use named preview budgets instead of scattered numeric limits.
- Clamp malformed or over-wide content rather than panic in the render path.

Sanitize before Markdown parsing so raw agent output cannot carry terminal controls through an otherwise valid Markdown block. Preserve ordinary Unicode and newlines; the seam must not turn valid prose or source code into lossy display text.

Add adversarial tests for CSI/OSC input, tabs, combining marks, wide characters, very long lines, and paths under the user's home directory.

Relevant Cyril files:

- `crates/cyril-ui/src/text.rs`
- `crates/cyril-ui/src/widgets/chat.rs`
- `crates/cyril-ui/src/widgets/markdown.rs`

Relevant Oh My Pi files:

- `packages/coding-agent/src/tools/render-utils.ts`
- `packages/tui/src/utils.ts`
- `docs/tui-core-renderer.md`

### 3. Make terminal ownership RAII and shutdown idempotent

Oh My Pi centralizes terminal modes and uses one idempotent teardown path for normal exit, signals, and failures. Repeated teardown requests share the same in-progress operation.

Cyril currently distributes terminal ownership across:

- `ratatui::init()` and `ratatui::restore()` in `crates/cyril/src/main.rs`.
- Mouse and bracketed-paste enable/disable in `main.rs`.
- Mouse toggling in `crates/cyril/src/app.rs`.
- Bridge shutdown in the ordinary `should_quit` path.

Create a `TerminalSession` guard that owns every terminal mode Cyril changes and restores them in `Drop`. Restoration should be safe to call more than once. The guard should also support temporarily restoring and re-entering terminal state if suspend/resume is added.

Create a separate idempotent application shutdown path shared by:

- `/quit` and keyboard exit.
- Draw or terminal-event failure.
- `SIGINT`, `SIGTERM`, and `SIGHUP` where supported.
- Panic recovery.
- Bridge disconnect or fatal startup failure.

The first shutdown request should synchronously mark the app as shutting down before awaiting bridge or persistence work. Later requests should join the same shutdown operation rather than double-send shutdown, double-restore terminal modes, or race resource cleanup.

Cyril's subprocess `Drop` guards already protect abnormal process exit. This recommendation makes graceful application and terminal teardown equally explicit.

Relevant Cyril files:

- `crates/cyril/src/main.rs`
- `crates/cyril/src/app.rs`
- `crates/cyril-core/src/protocol/transport.rs`

Relevant Oh My Pi files:

- `packages/tui/src/terminal.ts`
- `packages/coding-agent/src/modes/session-teardown.ts`

### 4. Wire terminal capabilities into theme resolution

Oh My Pi resolves terminal capabilities once. Capability misses disable optimizations rather than changing correctness.

Cyril already defines `ColorMode::{TrueColor, Ansi256, Ansi16, None}` and has extensive palette and no-color tests. However, `UiState::new` currently resolves `CyrilDark` with `ColorMode::TrueColor` unconditionally.

Add runtime resolution for:

- `NO_COLOR`.
- Truecolor-capable terminals.
- 256-color terminals.
- Basic ANSI terminals.
- Non-TTY output.
- An explicit user override, if Cyril exposes one.

Resolve the capability once at startup and pass the resolved theme into UI state. Keep detection pure over environment and terminal facts so it can be table-tested without changing process-global environment during a test.

Unknown terminals should choose a conservative display mode. A detection error should affect color quality, not whether the TUI works.

Relevant Cyril files:

- `crates/cyril-ui/src/theme.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril/src/main.rs`

Relevant Oh My Pi files:

- `packages/tui/src/terminal-capabilities.ts`
- `docs/tui-core-renderer.md`

### 5. Shrink the renderer interface

Oh My Pi's low-level TUI module is message-agnostic. Its core interface is small: render, input, focus, overlays, and lifecycle.

Cyril's `TuiState` trait exposes transcript, input, session, billing, overlays, terminal state, timing, voice, and subagent state through one broad getter interface. Its primary production implementation is `UiState`; the other major implementation is a render-test mock. A test mock alone does not establish a useful runtime adapter seam.

Do not continue growing `TuiState`. Prefer one of two shapes:

1. Pass concrete `&UiState` where no behavior actually varies.
2. Project narrow immutable views for the renderer:

```rust
struct FrameView<'a> {
    chat: ChatView<'a>,
    input: InputView<'a>,
    chrome: ChromeView<'a>,
    overlay: Option<OverlayView<'a>>,
}
```

Each widget should receive only the view it needs. Render tests can then construct small view fixtures rather than implementing a large mock trait. The view types should contain already-derived presentation facts; widgets should not need to reconstruct domain decisions.

This is an incremental refactor. Start with new widgets or with the transcript cache rather than rewriting every widget at once.

Relevant Cyril files:

- `crates/cyril-ui/src/traits.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril-ui/src/render.rs`

Relevant Oh My Pi files:

- `packages/tui/src/tui.ts`
- `docs/tui-runtime-internals.md`

### 6. Consolidate notification reduction

Cyril currently applies a routed notification to `SessionController` and `UiState`, then performs additional notification-specific work in `App::handle_notification`. This creates several potential owners of one state transition.

As Cyril adds ACP engines, proxy stages, observers, and more session-scoped streams, use one transactional update seam:

```rust
fn update(&mut self, event: AppEvent) -> UpdateResult;

struct UpdateResult {
    effects: SmallVec<[Effect; 4]>,
    dirty: DirtyRegions,
}
```

The reducer should own ordering and state transitions. Effects should describe bridge sends, persistence, notifications, and terminal operations without executing them inside the reducer. Rendering remains a pure projection.

`DirtyRegions` can replace the single `redraw_needed: bool` with facts such as transcript, input, chrome, overlay, or full-layout invalidation. Ratatui remains immediate-mode, but expensive presentation work can reuse unchanged cached regions.

Do not copy Oh My Pi's large controller files literally. The transferable lesson is single ownership of each transition and a strict domain-to-presentation seam.

Relevant Cyril files:

- `crates/cyril/src/app.rs`
- `crates/cyril-ui/src/state.rs`
- `crates/cyril-core/src/session.rs`

Relevant Oh My Pi files:

- `packages/coding-agent/src/modes/controllers/event-controller.ts`
- `packages/coding-agent/src/modes/controllers/input-controller.ts`
- `docs/tui-runtime-internals.md`

### 7. Add a real terminal fidelity harness

Oh My Pi's strongest TUI verification drives emitted ANSI through a virtual terminal across streams, resizes, overlays, Unicode, and terminal configurations. An independent model checks the resulting screen and scrollback.

Cyril's Ratatui `TestBackend` tests are valuable, but they primarily verify widget buffers and layout. They do not exercise all behavior of the real binary, Crossterm modes, or terminal restoration.

Add a PTY or virtual-terminal integration harness that runs the real Cyril binary and scripts:

1. Streaming assistant and thought chunks.
2. Tool start, partial update, completion, and failure.
3. Resize while streaming.
4. Modal open, navigation, and close.
5. Mouse capture toggling.
6. Paste events.
7. Wide Unicode, combining marks, tabs, and terminal control characters.
8. Normal quit, bridge failure, Ctrl+C, and forced termination.

The oracle should inspect terminal cells, cursor state, and enabled terminal modes. It should not inspect Cyril source text. Keep a small deterministic scenario matrix first; randomized event sequences can follow once the oracle is trusted.

Relevant Cyril files:

- `crates/cyril-ui/src/floor_tests.rs`
- `crates/cyril-ui/src/render.rs`
- `crates/cyril/src/main.rs`
- `crates/cyril/src/app.rs`

Relevant Oh My Pi files:

- `packages/tui/test/render-stress-harness.ts`
- `docs/tui-core-renderer.md`

### 8. Introduce tool presenters only when concrete tools require them

Oh My Pi uses specialized renderers for reads, edits, commands, searches, browser activity, and other tools, with a generic fallback.

Cyril currently renders tools through one `ToolKind` match in `crates/cyril-ui/src/widgets/chat.rs`. That is simpler and should remain the fallback. Once vendor-neutral ACP agents expose materially different tool shapes, introduce a presentation seam:

```rust
trait ToolPresenter {
    fn present(&self, call: &ToolCall, context: &PresentationContext) -> ToolView;
}
```

Presenters should return a neutral `ToolView`, not Ratatui widgets. The UI layer remains responsible for layout and styling. Register an exact tool-name presenter only when it provides concrete value; unknown ACP and MCP tools must continue through a generic kind-based presenter.

Avoid copying Oh My Pi's full renderer option surface. Cyril's combined `ToolCall` lifecycle can support a smaller interface.

Relevant Cyril files:

- `crates/cyril-ui/src/widgets/chat.rs`
- `crates/cyril-ui/src/traits.rs`
- `crates/cyril-core/src/types/tool_call.rs`

Relevant Oh My Pi files:

- `packages/coding-agent/src/tools/renderers.ts`
- `packages/coding-agent/src/modes/components/tool-execution.ts`

#### Ratatui implementation note: dark tool-use cards

The dark, full-width inline tool card used by Oh My Pi is highly feasible in Ratatui. Cyril already has most of the required data in `render_tool_call`:

- Pending, running, completed, and failed status.
- Tool-specific labels.
- Paths and command summaries.
- Diff statistics and rendered diff lines.
- Output previews, exit codes, and error text.

Cyril also already defines `inset_background` (`#282c34` in the current truecolor theme), which is a suitable card surface. The missing work is primarily layout, full-row background painting, and optional interaction state.

##### Current architectural constraint

Cyril flattens the complete transcript into a `Vec<Line>` and gives it to one Ratatui `Paragraph`. A standalone Ratatui `Block` cannot be inserted between two lines inside that paragraph: widgets occupy frame rectangles, not positions inside another widget's text.

There are two viable implementations.

##### Option A: emit styled physical lines

This is the conservative first implementation. Change `render_tool_call` to receive the available width and emit pre-wrapped, explicitly padded lines:

```text

  ▌ ⟳ Bash   cargo test --workspace                 1.8s
  ▌   Compiling cyril-core
  ▌   Compiling cyril-ui
  ▌   …

```

Each physical card row must:

1. Wrap content to the card's inner display width.
2. Add explicit horizontal padding.
3. Pad with spaces through the complete card width.
4. Apply the card background to the complete padded row.
5. Use background-styled blank rows when vertical padding is desired.

Conceptually:

```rust
fn render_tool_card(
    lines: &mut Vec<Line<'static>>,
    tool: &TrackedToolCall,
    width: usize,
    theme: &Theme,
) {
    let card_width = width.saturating_sub(2);
    let background = Style::new().bg(theme.inset_background);

    lines.push(padded_line("", card_width, background));
    lines.push(padded_tool_header(tool, card_width, background, theme));

    for body_line in tool_body_lines(tool, card_width.saturating_sub(4), theme) {
        lines.push(padded_line(body_line, card_width, background));
    }

    lines.push(padded_line("", card_width, background));
}
```

Explicit padding is load-bearing. Styling only the occupied text cells produces a ragged background rather than a rectangular card.

Tool-card content should be pre-wrapped instead of relying on `Paragraph::wrap`. Paragraph-generated continuation rows would not automatically receive Cyril's card padding, state rail, or complete background style.

This option preserves Cyril's current transcript layout, scrolling model, and `TestBackend` tests.

##### Option B: implement a block-aware transcript widget

A custom `ChatTranscript` widget is the better long-term shape if tool cards become interactive. It would:

1. Lay out transcript blocks individually.
2. Calculate each block's wrapped height.
3. Determine which block rows intersect the viewport.
4. Paint a rectangular background for tool cards directly into Ratatui's buffer.
5. Render the card's text into its inner rectangle.
6. Maintain block-aware scrolling and focus.

That shape naturally supports:

- Expand and collapse.
- Keyboard selection and mouse hit testing.
- Full-output views.
- Per-tool presentations.
- Stable per-block render caches.
- Streaming updates without rebuilding unrelated messages.

Start with Option A, but have it consume a pure presentation model that a future custom transcript widget can reuse:

```rust
struct ToolCardView {
    id: ToolCallId,
    phase: ToolCardPhase,
    title: String,
    metadata: Vec<ToolMetadata>,
    body: Vec<ToolBodySection>,
    expanded: bool,
}

enum ToolCardPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
}
```

The flow should be:

```text
ToolCall -> ToolCardView -> Ratatui lines or buffer
```

Tool interpretation stays independent of Ratatui layout, and specialized presenters can produce the same `ToolCardView`.

##### Cyril visual direction

Do not use literal `Color::Black`. Use the theme's semantic inset surface so the card remains distinguishable from a black terminal canvas and can support future light themes.

A restrained Cyril-specific card should use:

- A near-black full-row background.
- One-cell horizontal margins around the card.
- A colored left rail and status icon.
- A compact title and metadata row.
- Secondary text for ordinary output.
- Existing semantic diff colors for additions and deletions.

Suggested state treatment:

| State | Rail and icon | Body |
|---|---|---|
| Pending or running | Emphasis or cyan | Live command, path, or useful output |
| Succeeded | Subdued green | Compact result summary |
| Failed | Subdued red | Actionable error kept visible |
| Expanded | Same state color | Larger bounded output or diff |

Avoid strong red or green full-card backgrounds. The rail and icon should carry most of the state; the surface should stay quiet enough for a long transcript.

No-color mode must preserve structure through the rail, status glyph, labels, and indentation even when every background resolves to reset.

##### Lifecycle and interaction

Use `ToolCallId` as the card identity. Partial and terminal updates should replace the same card rather than append new cards.

If expansion is added, key it by identifier rather than transcript position:

```rust
expanded_tool_calls: HashSet<ToolCallId>
```

This prevents streaming updates or transcript insertions from moving expansion state to the wrong tool.

Recommended behavior:

| Tool state | Display behavior |
|---|---|
| Pending arguments | Spinner, title, partial command or path |
| Running | Spinner, elapsed time, latest useful output |
| Completed read or search | Collapse to one or two summary rows |
| Completed edit | Keep a compact diff visible |
| Completed command | Exit status plus output tail |
| Failed | Keep the actionable error visible |
| Expanded | Show bounded full output with larger preview or local scrolling |

The initial implementation does not need focus or expansion. A non-interactive card already provides most of the visual improvement without changing Cyril's input routing.

##### Theme roles

The first implementation can reuse `inset_background`. If state-specific surfaces prove useful, add semantic roles rather than embedding RGB values in the widget:

```rust
tool_background
tool_pending_background
tool_success_background
tool_error_background
tool_title
tool_output
```

Keep state backgrounds subtle and ensure every role resolves meaningfully for truecolor, ANSI-256, ANSI-16, and no-color modes.

##### Verification

Ratatui `TestBackend` tests should verify:

- Every card row carries the expected background through its trailing cell.
- Narrow terminals do not panic or underflow widths.
- Wide Unicode and combining marks do not break padding.
- Long commands wrap within the card.
- Streaming updates replace the same logical card.
- Completed and failed cards use the correct semantic roles.
- No-color mode remains structurally understandable.
- Cards clipped at the top or bottom of the viewport render cleanly.
- Expanded state follows `ToolCallId`, not transcript index.

The visual card does not require Oh My Pi's native-scrollback implementation. Only exact normal-screen finalization and immutable shell history would require the more complex commit-boundary machinery described below.

## Optional: native shell scrollback

Oh My Pi's unusual renderer uses the normal terminal screen and an append-only native-scrollback contract. Final rows enter terminal history once; mutable rows remain below a commit boundary until they settle.

Cyril currently uses Ratatui's fullscreen alternate-screen initialization. Ratatui 0.30 supports an inline viewport and `Terminal::insert_before`, so Rust and Ratatui do not prevent a normal-screen design. However, changing viewport mode alone would not reproduce Oh My Pi's behavior. The difficult parts are:

- Distinguishing live, durable, and byte-final transcript rows.
- Never rewriting rows already committed to native history.
- Handling resize without losing or duplicating transcript content.
- Preventing overlays from entering history.
- Preserving cursor placement and terminal modes across direct terminals and multiplexers.

Keep Cyril fullscreen unless native shell scrollback, native selection, or transcript persistence after exit becomes an explicit product requirement. If it does, first build a small Ratatui inline-viewport prototype with streaming text, resize, and a modal. Do not begin by replacing Ratatui or porting Oh My Pi's ANSI renderer.

Ratatui reference:

- <https://github.com/ratatui/ratatui/blob/main/examples/apps/inline/src/main.rs>

Relevant Oh My Pi files:

- `docs/tui-core-renderer.md`
- `packages/tui/src/tui.ts`
- `packages/coding-agent/src/modes/components/transcript-container.ts`

## Suggested implementation order

1. Central display sanitization and runtime color capability selection.
2. Terminal RAII and unified idempotent shutdown.
3. A real PTY or virtual-terminal fidelity harness.
4. A long-transcript streaming benchmark.
5. Versioned transcript-block caching if the benchmark confirms the need.
6. Gradual replacement of broad `TuiState` access and multi-owner notification handling.
7. Specialized tool presenters only as concrete tool UX requires them.
8. Native scrollback only after an explicit product decision and prototype.

The largest likely performance opportunity is the transcript path: Cyril already has message versioning, Markdown caching, and adaptive rendering, but those pieces are not yet combined into an incremental rendering contract. The highest correctness opportunity is the terminal boundary: sanitize everything displayed, own terminal modes in one guard, and verify the real emitted terminal behavior rather than only in-memory Ratatui buffers.
