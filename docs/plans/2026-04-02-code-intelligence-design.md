# Code Intelligence Support — Design Document

**Date:** 2026-04-02
**Status:** Approved
**Branch:** v2-rewrite

## Context

Kiro CLI v1.29+ adds code intelligence — an embedded LSP client that manages language servers (rust-analyzer, pyright, tsserver, etc.) for the workspace. This surfaces in two ways:

1. **`/code` slash command** — user-facing TUI command for managing LSP status, with subcommands (`init`, `summary`, `logs`). Uses the existing `kiro.dev/commands/execute` ACP method.
2. **`code` native tool** — agent-side tool with operations like `search_symbols`, `lookup_symbols`, `pattern_search`, `pattern_rewrite`. Appears as a regular tool call in the ACP notification stream.

Kiro manages all LSP lifecycle (spawning servers, file watching, diagnostics) internally. None of this crosses the ACP wire. The client only sees command responses and tool calls.

## Scope

### Phase 1 (this design)
- `/code` status panel overlay with LSP server table
- `executePrompt` auto-send for `/code summary` and similar subcommands
- Toolbar indicator for code intelligence active state
- Fallback formatting for unknown `/code` subcommand responses

### Phase 2 (future)
- Rich rendering for the `code` native tool calls (symbol search results, AST matches)
- Subcommand autocomplete for `/code init`, `/code summary`, etc.
- `/code logs` dedicated panel

### Out of scope
- `.kiro/settings/lsp.json` management (Kiro owns this)
- LSP server management (entirely internal to Kiro)
- New ACP wire protocol methods (none needed)

## Response Routing

The `/code` command response from Kiro has three shapes, detected by inspecting `data`:

| Shape | Detection | Action |
|---|---|---|
| Panel | `data.status` + `data.lsps` exist | Open code panel overlay |
| Prompt | `data.executePrompt` exists | Show system message + auto-send prompt |
| Unknown | Neither pattern matches | Fallback to `format_command_response()` |

## Data Types (cyril-core)

### `CodePanelData`

```rust
// crates/cyril-core/src/types/code_panel.rs

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStatus {
    Initialized,
    Initializing,
    Failed,
    Unknown(String),
}

#[derive(Debug, Clone)]
pub struct LspServerInfo {
    pub name: String,
    pub languages: Vec<String>,
    pub status: LspStatus,
    pub init_duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct CodePanelData {
    pub status: LspStatus,
    pub message: Option<String>,
    pub warning: Option<String>,
    pub root_path: Option<String>,
    pub detected_languages: Vec<String>,
    pub project_markers: Vec<String>,
    pub config_path: Option<String>,
    pub doc_url: Option<String>,
    pub lsps: Vec<LspServerInfo>,
}
```

### `CodeCommandResponse`

```rust
pub enum CodeCommandResponse {
    Panel(CodePanelData),
    Prompt { text: String, label: Option<String> },
    Unknown(serde_json::Value),
}
```

`CodeCommandResponse::from_json(value: &serde_json::Value) -> CodeCommandResponse` inspects the `data` field and routes by shape. All Kiro JSON parsing stays in `cyril-core`.

## UI State (cyril-ui)

### Panel State

```rust
// In UiState
code_panel: Option<CodePanelData>,  // None = closed
```

No wrapper struct needed — the panel is read-only display. Methods: `show_code_panel(data)`, `close_code_panel()`, `code_panel() -> Option<&CodePanelData>`.

### TuiState Trait Additions

```rust
fn code_panel(&self) -> Option<&CodePanelData>;
fn code_intelligence_active(&self) -> bool;
```

## Code Panel Widget (cyril-ui)

`render_code_panel()` draws a centered overlay:

```
┌─ /code ──────────────────────────────┐
│ ✓ initialized — LSP servers ready    │
│                                      │
│ Workspace: /home/user/repos/cyril    │
│ Languages: rust                      │
│ Markers:   Cargo.toml                │
│                                      │
│ LSP Servers:                         │
│ ✓ rust-analyzer  (rust)  initialized │
│ ✗ gopls          (go)    failed      │
│ ○ pyright        (python)  —         │
│                                      │
│ Config: .kiro/settings/lsp.json      │
│                                      │
│ [r] refresh  [Esc] close             │
└──────────────────────────────────────┘
```

Status icons: `✓` green (initialized), `◐` yellow (initializing), `✗` red (failed), `○` dim (unknown). Duration appended when present: `initialized (44ms)`.

## Key Handling

Code panel slots into the overlay priority chain:

1. Global shortcuts (Ctrl+C, Ctrl+Q, Ctrl+M)
2. Approval overlay
3. Picker overlay
4. **Code panel overlay** ← new
5. Autocomplete
6. Normal input

When active:
- `Esc` → close panel
- `r` → refresh (sends `BridgeCommand::ExecuteCommand { command: "code", args: {} }`)
- All other keys → consumed

## App Wiring (cyril binary)

### CommandExecuted Handler

```rust
if command == "code" {
    match CodeCommandResponse::from_json(response) {
        CodeCommandResponse::Panel(data) => {
            if data.status == LspStatus::Initialized {
                self.session.set_code_intelligence_active(true);
            }
            self.ui_state.show_code_panel(data);
        }
        CodeCommandResponse::Prompt { text, label } => {
            let display = label.as_deref().unwrap_or("Code Intelligence");
            self.ui_state.add_system_message(format!("/{command}: {display}"));
            self.bridge.send(BridgeCommand::SendPrompt(text));
        }
        CodeCommandResponse::Unknown(value) => {
            let text = format_command_response(command, response);
            self.ui_state.add_command_output(command.clone(), text);
        }
    }
}
```

### Refresh

Pressing `r` in the panel sends `BridgeCommand::ExecuteCommand { command: "code", args: {} }`. The response arrives as another `CommandExecuted`, hits the `Panel` branch, and replaces the panel data. No new bridge command variant.

### Code Intelligence Active Indicator

**`SessionController`:** New `code_intelligence_active: bool` field (default `false`) with getter/setter.

**Set on two triggers:**
1. At session start — if `.kiro/settings/lsp.json` exists in the working directory
2. On `/code` panel response — when `data.status == LspStatus::Initialized`

**Toolbar rendering:** `✦ code intel` in dim accent color, shown only when active.

```
 ◆ claude-sonnet-4  │  agent  │  ✦ code intel  │  42% context
```

## File Changes

| Crate | File | Change |
|---|---|---|
| `cyril-core` | `types/code_panel.rs` (new) | `CodePanelData`, `LspServerInfo`, `LspStatus`, `CodeCommandResponse`, parsing |
| `cyril-core` | `types/mod.rs` | Add `pub mod code_panel` |
| `cyril-core` | `session.rs` | Add `code_intelligence_active` field + getter/setter |
| `cyril-ui` | `state.rs` | Add `code_panel: Option<CodePanelData>` + methods |
| `cyril-ui` | `traits.rs` | Add `code_panel()` and `code_intelligence_active()` to `TuiState` |
| `cyril-ui` | `widgets/code_panel.rs` (new) | `render_code_panel()` |
| `cyril-ui` | `widgets/mod.rs` | Add `pub mod code_panel` |
| `cyril-ui` | `widgets/toolbar.rs` | Render `✦ code intel` indicator |
| `cyril` | `app.rs` | Route `/code` responses, key handler for code panel, refresh, `.kiro` check |

## Design Decisions

**Why an overlay panel instead of inline text?** The LSP status view is interactive (refreshable) and tabular. Inline text can't be refreshed — once you scroll past, it's gone. The Kiro TUI uses a panel for the same reason.

**Why auto-send executePrompt?** The Kiro TUI does this silently. We add transparency (showing a system message first) but don't add friction (no confirmation step). The prompt is generated by Kiro, not user-editable.

**Why skip subcommand autocomplete?** The Kiro TUI doesn't do it either. Subcommands aren't in `AvailableCommandsUpdate` — they only appear in the `/help` response. Adding autocomplete would require either hardcoding known subcommands or parsing them from help data, both fragile.

**Why `Option<CodePanelData>` instead of a `CodePanelState` wrapper?** The panel is pure display — no cursor position, filter text, or selection state. A wrapper struct would be empty ceremony. If we add scrolling later (e.g., for many LSP servers), we can promote to a struct then.
