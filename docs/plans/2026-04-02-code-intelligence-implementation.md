# Code Intelligence Support — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Support Kiro CLI v1.29+ code intelligence — `/code` status panel, `executePrompt` auto-send, and toolbar indicator.

**Architecture:** Three-layer change following Cyril's existing crate boundaries. New types in `cyril-core`, new overlay state + widget in `cyril-ui`, response routing + key handling in `cyril` binary. No new ACP wire protocol — everything routes through existing `CommandExecuted` notification.

**Tech Stack:** Rust, ratatui (TUI framework), serde_json (response parsing)

**Design doc:** `docs/plans/2026-04-02-code-intelligence-design.md`

---

### Task 1: Add CodePanelData types and response parser (cyril-core)

**Files:**
- Create: `crates/cyril-core/src/types/code_panel.rs`
- Modify: `crates/cyril-core/src/types/mod.rs:1-19`

**Step 1: Write the test for response parsing**

Add to the bottom of `crates/cyril-core/src/types/code_panel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_panel_response() {
        let response = json!({
            "success": true,
            "message": "Code intelligence status",
            "data": {
                "status": "initialized",
                "message": "LSP servers ready",
                "rootPath": "/home/user/project",
                "detectedLanguages": ["rust"],
                "projectMarkers": ["Cargo.toml"],
                "configPath": ".kiro/settings/lsp.json",
                "docUrl": "https://kiro.dev/docs/cli/code-intelligence/",
                "lsps": [
                    {
                        "name": "rust-analyzer",
                        "languages": ["rust"],
                        "status": "initialized",
                        "initDurationMs": 44
                    }
                ]
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.status, LspStatus::Initialized);
                assert_eq!(data.detected_languages, vec!["rust"]);
                assert_eq!(data.lsps.len(), 1);
                assert_eq!(data.lsps[0].name, "rust-analyzer");
                assert_eq!(data.lsps[0].status, LspStatus::Initialized);
                assert_eq!(data.lsps[0].init_duration_ms, Some(44));
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn parse_prompt_response() {
        let response = json!({
            "success": true,
            "data": {
                "executePrompt": "Analyze the codebase...",
                "label": "Code Summary"
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Prompt { text, label } => {
                assert_eq!(text, "Analyze the codebase...");
                assert_eq!(label, Some("Code Summary".into()));
            }
            other => panic!("Expected Prompt, got {other:?}"),
        }
    }

    #[test]
    fn parse_unknown_response() {
        let response = json!({
            "success": true,
            "message": "Something else happened"
        });
        let result = CodeCommandResponse::from_json(&response);
        assert!(matches!(result, CodeCommandResponse::Unknown(_)));
    }

    #[test]
    fn parse_initializing_status() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initializing",
                "message": "Starting LSP servers...",
                "detectedLanguages": [],
                "projectMarkers": [],
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.status, LspStatus::Initializing);
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn parse_warning_field() {
        let response = json!({
            "success": true,
            "data": {
                "status": "initialized",
                "warning": "pyright not found on PATH",
                "detectedLanguages": ["python"],
                "projectMarkers": ["requirements.txt"],
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        match result {
            CodeCommandResponse::Panel(data) => {
                assert_eq!(data.warning, Some("pyright not found on PATH".into()));
            }
            other => panic!("Expected Panel, got {other:?}"),
        }
    }

    #[test]
    fn prompt_takes_priority_over_panel() {
        let response = json!({
            "success": true,
            "data": {
                "executePrompt": "Do something...",
                "status": "initialized",
                "lsps": []
            }
        });
        let result = CodeCommandResponse::from_json(&response);
        assert!(matches!(result, CodeCommandResponse::Prompt { .. }));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p cyril-core code_panel`
Expected: Compilation error — module and types don't exist yet.

**Step 3: Implement the types and parser**

Create `crates/cyril-core/src/types/code_panel.rs`:

```rust
/// LSP server status as reported by Kiro's code intelligence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LspStatus {
    Initialized,
    Initializing,
    Failed,
    Unknown(String),
}

/// A single LSP server entry from the /code panel response.
#[derive(Debug, Clone)]
pub struct LspServerInfo {
    pub name: String,
    pub languages: Vec<String>,
    pub status: LspStatus,
    pub init_duration_ms: Option<u64>,
}

/// Parsed data from a /code status response.
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

/// The three shapes a /code CommandExecuted response can take.
#[derive(Debug)]
pub enum CodeCommandResponse {
    /// Show the status panel overlay.
    Panel(CodePanelData),
    /// Auto-send a prompt to the agent.
    Prompt { text: String, label: Option<String> },
    /// Unknown shape — fall through to generic formatting.
    Unknown(serde_json::Value),
}

impl CodeCommandResponse {
    /// Parse a `CommandExecuted` response JSON for the `/code` command.
    ///
    /// Routes by data shape:
    /// - `data.executePrompt` exists → Prompt (checked first — takes priority)
    /// - `data.status` exists → Panel
    /// - anything else → Unknown
    pub fn from_json(response: &serde_json::Value) -> Self {
        let data = match response.get("data") {
            Some(d) if !d.is_null() => d,
            _ => return Self::Unknown(response.clone()),
        };

        // Prompt path — check first (takes priority if both shapes present)
        if let Some(prompt) = data.get("executePrompt").and_then(|p| p.as_str()) {
            let label = data
                .get("label")
                .and_then(|l| l.as_str())
                .map(String::from);
            return Self::Prompt {
                text: prompt.to_string(),
                label,
            };
        }

        // Panel path
        if let Some(status_str) = data.get("status").and_then(|s| s.as_str()) {
            let status = parse_lsp_status(status_str);
            let lsps = data
                .get("lsps")
                .and_then(|l| l.as_array())
                .map(|arr| arr.iter().filter_map(parse_lsp_server).collect())
                .unwrap_or_default();

            return Self::Panel(CodePanelData {
                status,
                message: data.get("message").and_then(|m| m.as_str()).map(String::from),
                warning: data.get("warning").and_then(|w| w.as_str()).map(String::from),
                root_path: data
                    .get("rootPath")
                    .and_then(|r| r.as_str())
                    .map(String::from),
                detected_languages: data
                    .get("detectedLanguages")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                project_markers: data
                    .get("projectMarkers")
                    .and_then(|d| d.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default(),
                config_path: data
                    .get("configPath")
                    .and_then(|c| c.as_str())
                    .map(String::from),
                doc_url: data
                    .get("docUrl")
                    .and_then(|d| d.as_str())
                    .map(String::from),
                lsps,
            });
        }

        Self::Unknown(response.clone())
    }
}

fn parse_lsp_status(s: &str) -> LspStatus {
    match s {
        "initialized" => LspStatus::Initialized,
        "initializing" => LspStatus::Initializing,
        "failed" => LspStatus::Failed,
        other => LspStatus::Unknown(other.to_string()),
    }
}

fn parse_lsp_server(value: &serde_json::Value) -> Option<LspServerInfo> {
    let name = value.get("name")?.as_str()?.to_string();
    let languages = value
        .get("languages")
        .and_then(|l| l.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let status = value
        .get("status")
        .and_then(|s| s.as_str())
        .map(parse_lsp_status)
        .unwrap_or(LspStatus::Unknown("missing".into()));
    let init_duration_ms = value.get("initDurationMs").and_then(|d| d.as_u64());

    Some(LspServerInfo {
        name,
        languages,
        status,
        init_duration_ms,
    })
}
```

**Step 4: Register the module in `types/mod.rs`**

Add after line 1 (`pub mod command;`):

```rust
pub mod code_panel;
```

And add re-exports after line 16 (`pub use session::...`):

```rust
pub use code_panel::{CodeCommandResponse, CodePanelData, LspServerInfo, LspStatus};
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p cyril-core code_panel`
Expected: All 6 tests pass.

**Step 6: Commit**

```bash
git add crates/cyril-core/src/types/code_panel.rs crates/cyril-core/src/types/mod.rs
git commit -m "feat: add CodePanelData types and response parser"
```

---

### Task 2: Add code_intelligence_active to SessionController (cyril-core)

**Files:**
- Modify: `crates/cyril-core/src/session.rs:3-26` (struct + constructor)

**Step 1: Write the test**

Add to `crates/cyril-core/src/session.rs` (inside existing `#[cfg(test)] mod tests` if present, or create one):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_intelligence_defaults_to_false() {
        let session = SessionController::new();
        assert!(!session.code_intelligence_active());
    }

    #[test]
    fn set_code_intelligence_active() {
        let mut session = SessionController::new();
        session.set_code_intelligence_active(true);
        assert!(session.code_intelligence_active());
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p cyril-core -- code_intelligence`
Expected: Compilation error — method doesn't exist.

**Step 3: Implement**

In `crates/cyril-core/src/session.rs`:

Add field to `SessionController` struct (after line 11, before closing `}`):
```rust
    code_intelligence_active: bool,
```

Add to `new()` constructor (after line 24, before closing `}`):
```rust
            code_intelligence_active: false,
```

Add getter (after line 59):
```rust
    pub fn code_intelligence_active(&self) -> bool {
        self.code_intelligence_active
    }
```

Add setter (after line 77):
```rust
    pub fn set_code_intelligence_active(&mut self, active: bool) {
        self.code_intelligence_active = active;
    }
```

**Step 4: Run tests to verify they pass**

Run: `cargo test -p cyril-core -- code_intelligence`
Expected: Both tests pass.

**Step 5: Commit**

```bash
git add crates/cyril-core/src/session.rs
git commit -m "feat: add code_intelligence_active to SessionController"
```

---

### Task 3: Add code panel state to UiState and TuiState trait (cyril-ui)

**Files:**
- Modify: `crates/cyril-ui/src/state.rs:23-66` (UiState struct)
- Modify: `crates/cyril-ui/src/traits.rs:19-56` (TuiState trait)
- Modify: `crates/cyril-ui/src/traits.rs:240-358` (MockTuiState)

**Step 1: Write the test**

Add to the `#[cfg(test)] mod tests` block in `crates/cyril-ui/src/state.rs`:

```rust
    #[test]
    fn code_panel_lifecycle() {
        use cyril_core::types::{CodePanelData, LspStatus};

        let mut state = UiState::new(500);
        assert!(state.code_panel().is_none());
        assert!(!state.has_code_panel());

        let data = CodePanelData {
            status: LspStatus::Initialized,
            message: Some("LSP servers ready".into()),
            warning: None,
            root_path: Some("/home/user/project".into()),
            detected_languages: vec!["rust".into()],
            project_markers: vec!["Cargo.toml".into()],
            config_path: Some(".kiro/settings/lsp.json".into()),
            doc_url: None,
            lsps: vec![],
        };

        state.show_code_panel(data);
        assert!(state.has_code_panel());
        assert!(state.code_panel().is_some());

        state.close_code_panel();
        assert!(!state.has_code_panel());
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-ui -- code_panel_lifecycle`
Expected: Compilation error — methods don't exist.

**Step 3: Implement UiState changes**

In `crates/cyril-ui/src/state.rs`:

Add field to `UiState` struct (after `picker: Option<PickerState>,` at line 55):
```rust
    code_panel: Option<cyril_core::types::CodePanelData>,
```

Initialize in `new()` (find `picker: None,` and add after it):
```rust
            code_panel: None,
```

Add methods (group with the existing overlay methods near `show_picker`/`picker_cancel`):
```rust
    pub fn show_code_panel(&mut self, data: cyril_core::types::CodePanelData) {
        self.code_panel = Some(data);
    }

    pub fn close_code_panel(&mut self) {
        self.code_panel = None;
    }

    pub fn has_code_panel(&self) -> bool {
        self.code_panel.is_some()
    }

    pub fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData> {
        self.code_panel.as_ref()
    }
```

**Step 4: Add to TuiState trait**

In `crates/cyril-ui/src/traits.rs`, add to the trait (after `fn picker()` at line 45):
```rust
    fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData>;
    fn code_intelligence_active(&self) -> bool;
```

Implement in the `TuiState for UiState` impl block (find the file that implements TuiState for UiState — likely `state.rs`). Add:
```rust
    fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData> {
        self.code_panel.as_ref()
    }
    fn code_intelligence_active(&self) -> bool {
        false // UiState doesn't own this — App reads it from SessionController
    }
```

**Step 5: Update MockTuiState**

In `crates/cyril-ui/src/traits.rs`, add to `MockTuiState` struct (after `picker` field at line 257):
```rust
        pub code_panel: Option<cyril_core::types::CodePanelData>,
        pub code_intelligence_active: bool,
```

Add to `Default` impl (after `picker: None,` at line 284):
```rust
                code_panel: None,
                code_intelligence_active: false,
```

Add to `TuiState for MockTuiState` impl (after `picker()` method):
```rust
        fn code_panel(&self) -> Option<&cyril_core::types::CodePanelData> {
            self.code_panel.as_ref()
        }
        fn code_intelligence_active(&self) -> bool {
            self.code_intelligence_active
        }
```

**Step 6: Run tests to verify they pass**

Run: `cargo test -p cyril-ui -- code_panel_lifecycle`
Expected: PASS

Run: `cargo check` to verify all existing tests still compile.

**Step 7: Commit**

```bash
git add crates/cyril-ui/src/state.rs crates/cyril-ui/src/traits.rs
git commit -m "feat: add code panel overlay state to UiState and TuiState"
```

---

### Task 4: Create code panel widget (cyril-ui)

**Files:**
- Create: `crates/cyril-ui/src/widgets/code_panel.rs`
- Modify: `crates/cyril-ui/src/widgets/mod.rs`

**Step 1: Write the render test**

Add at the bottom of `crates/cyril-ui/src/widgets/code_panel.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cyril_core::types::{CodePanelData, LspServerInfo, LspStatus};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn sample_panel_data() -> CodePanelData {
        CodePanelData {
            status: LspStatus::Initialized,
            message: Some("LSP servers ready".into()),
            warning: None,
            root_path: Some("/home/user/repos/cyril".into()),
            detected_languages: vec!["rust".into()],
            project_markers: vec!["Cargo.toml".into()],
            config_path: Some(".kiro/settings/lsp.json".into()),
            doc_url: None,
            lsps: vec![
                LspServerInfo {
                    name: "rust-analyzer".into(),
                    languages: vec!["rust".into()],
                    status: LspStatus::Initialized,
                    init_duration_ms: Some(44),
                },
                LspServerInfo {
                    name: "pyright".into(),
                    languages: vec!["python".into()],
                    status: LspStatus::Failed,
                    init_duration_ms: None,
                },
            ],
        }
    }

    #[test]
    fn code_panel_renders_without_panic() {
        let data = sample_panel_data();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &data);
            })
            .expect("draw");
    }

    #[test]
    fn code_panel_renders_with_warning() {
        let mut data = sample_panel_data();
        data.warning = Some("pyright not found on PATH".into());
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &data);
            })
            .expect("draw");
    }

    #[test]
    fn code_panel_renders_empty_lsps() {
        let data = CodePanelData {
            status: LspStatus::Initializing,
            message: Some("Detecting workspace...".into()),
            warning: None,
            root_path: None,
            detected_languages: vec![],
            project_markers: vec![],
            config_path: None,
            doc_url: None,
            lsps: vec![],
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &data);
            })
            .expect("draw");
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test -p cyril-ui -- code_panel`
Expected: Compilation error — module doesn't exist.

**Step 3: Register the module**

In `crates/cyril-ui/src/widgets/mod.rs`, add after line 2 (`pub mod chat;`):
```rust
pub mod code_panel;
```

**Step 4: Implement the widget**

Create `crates/cyril-ui/src/widgets/code_panel.rs`:

```rust
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use cyril_core::types::{CodePanelData, LspStatus};

/// Render the code intelligence panel as a centered overlay.
pub fn render(frame: &mut Frame, area: Rect, data: &CodePanelData) {
    let mut lines: Vec<Line> = Vec::new();

    // Status line
    let (icon, color) = status_style(&data.status);
    let mut status_spans = vec![Span::styled(
        format!("{icon} {}", status_label(&data.status)),
        Style::default().fg(color),
    )];
    if let Some(ref msg) = data.message {
        status_spans.push(Span::styled(
            format!(" — {msg}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    lines.push(Line::from(status_spans));

    // Warning
    if let Some(ref warning) = data.warning {
        lines.push(Line::default());
        lines.push(Line::styled(
            format!("⚠ {warning}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Workspace info
    if data.root_path.is_some()
        || !data.detected_languages.is_empty()
        || !data.project_markers.is_empty()
    {
        lines.push(Line::default());

        if let Some(ref root) = data.root_path {
            lines.push(Line::from(vec![
                Span::styled("Workspace: ", Style::default().fg(Color::Cyan)),
                Span::styled(root.as_str(), Style::default().fg(Color::DarkGray)),
            ]));
        }
        if !data.detected_languages.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Languages: ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    data.detected_languages.join(", "),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
        if !data.project_markers.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("Markers:   ", Style::default().fg(Color::Cyan)),
                Span::styled(
                    data.project_markers.join(", "),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    // LSP servers
    if !data.lsps.is_empty() {
        lines.push(Line::default());
        lines.push(Line::styled(
            "LSP Servers:",
            Style::default().fg(Color::Cyan),
        ));

        let max_name_len = data.lsps.iter().map(|l| l.name.len()).max().unwrap_or(8);

        for lsp in &data.lsps {
            let (lsp_icon, lsp_color) = status_style(&lsp.status);
            let langs = format!("({})", lsp.languages.join(", "));
            let duration = lsp
                .init_duration_ms
                .map(|ms| format!(" ({ms}ms)"))
                .unwrap_or_default();

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{lsp_icon} {:width$}", lsp.name, width = max_name_len),
                    Style::default().fg(lsp_color),
                ),
                Span::styled(
                    format!("  {langs:16}"),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}{duration}", status_label(&lsp.status)),
                    Style::default().fg(lsp_color),
                ),
            ]));
        }
    }

    // Config path
    if let Some(ref config) = data.config_path {
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Config: ", Style::default().fg(Color::DarkGray)),
            Span::styled(config.as_str(), Style::default().fg(Color::Cyan)),
        ]));
    }

    // Footer
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("[r]", Style::default().fg(Color::Cyan)),
        Span::styled(" refresh  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Esc]", Style::default().fg(Color::Cyan)),
        Span::styled(" close", Style::default().fg(Color::DarkGray)),
    ]));

    // Size and position
    let content_width = lines
        .iter()
        .map(|l| l.width())
        .max()
        .unwrap_or(30) as u16
        + 4; // padding inside border
    let width = content_width.clamp(40, 80).min(area.width.saturating_sub(4));
    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(4)); // +2 for border
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                " /code ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}

fn status_style(status: &LspStatus) -> (&'static str, Color) {
    match status {
        LspStatus::Initialized => ("✓", Color::Green),
        LspStatus::Initializing => ("◐", Color::Yellow),
        LspStatus::Failed => ("✗", Color::Red),
        LspStatus::Unknown(_) => ("○", Color::DarkGray),
    }
}

fn status_label(status: &LspStatus) -> &str {
    match status {
        LspStatus::Initialized => "initialized",
        LspStatus::Initializing => "initializing",
        LspStatus::Failed => "failed",
        LspStatus::Unknown(s) => s.as_str(),
    }
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test -p cyril-ui -- code_panel`
Expected: All 3 tests pass.

**Step 6: Commit**

```bash
git add crates/cyril-ui/src/widgets/code_panel.rs crates/cyril-ui/src/widgets/mod.rs
git commit -m "feat: add code panel overlay widget"
```

---

### Task 5: Add code panel to render pipeline (cyril-ui)

**Files:**
- Modify: `crates/cyril-ui/src/render.rs:32-38`

**Step 1: Add code panel overlay to draw_inner**

In `crates/cyril-ui/src/render.rs`, after the picker overlay block (line 37-38), add:

```rust
    if let Some(code_panel) = state.code_panel() {
        crate::widgets::code_panel::render(frame, area, code_panel);
    }
```

**Step 2: Run existing render tests**

Run: `cargo test -p cyril-ui -- draw`
Expected: All existing tests pass (MockTuiState defaults `code_panel` to None).

**Step 3: Commit**

```bash
git add crates/cyril-ui/src/render.rs
git commit -m "feat: render code panel overlay in draw pipeline"
```

---

### Task 6: Add toolbar indicator (cyril-ui)

**Files:**
- Modify: `crates/cyril-ui/src/widgets/toolbar.rs:62-69`

**Step 1: Write the test**

Add to `crates/cyril-ui/src/widgets/toolbar.rs` tests module:

```rust
    #[test]
    fn toolbar_renders_code_intel_indicator() {
        let state = MockTuiState {
            session_label: Some("my-session".into()),
            code_intelligence_active: true,
            ..Default::default()
        };

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
        // Rendering succeeded with code intel active — no panic
    }
```

**Step 2: Run test to verify it fails**

Run: `cargo test -p cyril-ui -- code_intel_indicator`
Expected: Compilation error — `code_intelligence_active` field not on MockTuiState (this will already exist after Task 3).

Actually this should compile after Task 3. Run and verify it passes.

**Step 3: Add indicator to toolbar render**

In `crates/cyril-ui/src/widgets/toolbar.rs`, after the Model block (after line 69, before the elapsed time block at line 71), add:

```rust
    // Code intelligence indicator
    if state.code_intelligence_active() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            "✦ code intel",
            Style::default().fg(Color::Cyan),
        ));
    }
```

**Step 4: Run all toolbar tests**

Run: `cargo test -p cyril-ui -- toolbar`
Expected: All tests pass.

**Step 5: Commit**

```bash
git add crates/cyril-ui/src/widgets/toolbar.rs
git commit -m "feat: show code intelligence indicator in toolbar"
```

---

### Task 7: Wire up app.rs — response routing, key handling, and code_intelligence_active (cyril)

**Files:**
- Modify: `crates/cyril/src/app.rs:206-229` (CommandExecuted handler)
- Modify: `crates/cyril/src/app.rs:264-274` (key handling overlay chain)
- Modify: `crates/cyril/src/app.rs:448-559` (format_command_response — no changes, just for reference)

This is the wiring task — it connects everything together. There are three changes.

**Step 1: Add import**

At the top of `crates/cyril/src/app.rs`, ensure this import exists:

```rust
use cyril_core::types::{CodeCommandResponse, LspStatus};
```

**Step 2: Replace CommandExecuted handler**

Replace the block at lines 206-229 with:

```rust
        // Handle command execution response
        if let Notification::CommandExecuted { ref command, ref response } = notification {
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
                        self.ui_state
                            .add_system_message(format!("/code: {display}"));
                        // Auto-send the generated prompt to the agent
                        let session_id = self.session.id().cloned();
                        if let Some(id) = session_id {
                            self.ui_state.add_user_message(&text);
                            self.session.set_status(SessionStatus::Busy);
                            self.ui_state.set_activity(Activity::Sending);
                            let _ = self
                                .bridge_sender
                                .send(BridgeCommand::SendPrompt {
                                    session_id: id,
                                    content_blocks: vec![text],
                                })
                                .await;
                        }
                    }
                    CodeCommandResponse::Unknown(_) => {
                        let text = format_command_response(command, response);
                        self.ui_state.add_command_output(command.clone(), text);
                    }
                }
            } else {
                let text = format_command_response(command, response);
                self.ui_state.add_command_output(command.clone(), text);

                // WORKAROUND(Kiro v1.28.0): extract model from /model response
                if command == "model" {
                    if let Some(model_id) = response
                        .get("data")
                        .and_then(|d| d.get("model"))
                        .and_then(|m| m.get("id"))
                        .and_then(|id| id.as_str())
                    {
                        self.ui_state.set_current_model(Some(model_id.to_string()));
                    }
                }
            }

            self.redraw_needed = true;
        }
```

Note: The `Prompt` branch needs to be `async` since it sends via the bridge. Check if `handle_notification` is already `async`. If not, the bridge send can use `try_send` or the sender can be cloned. Match the pattern used elsewhere in the file for sending bridge commands from notification handlers.

**Step 3: Add code panel to key handling overlay chain**

In `crates/cyril/src/app.rs`, after the picker overlay check (after line 274), add:

```rust
        if self.ui_state.has_code_panel() {
            self.handle_code_panel_key(key).await?;
            self.redraw_needed = true;
            return Ok(());
        }
```

**Step 4: Implement handle_code_panel_key**

Add a new method to `App` (near `handle_approval_key` and `handle_picker_key`):

```rust
    async fn handle_code_panel_key(&mut self, key: KeyEvent) -> cyril_core::Result<()> {
        match key.code {
            KeyCode::Esc => self.ui_state.close_code_panel(),
            KeyCode::Char('r') => {
                // Refresh: re-execute /code command
                if let Some(id) = self.session.id().cloned() {
                    self.bridge_sender
                        .send(BridgeCommand::ExecuteCommand {
                            command: "code".into(),
                            session_id: id,
                            args: serde_json::json!({}),
                        })
                        .await?;
                }
            }
            _ => {} // Consume all other keys
        }
        Ok(())
    }
```

**Step 5: Add optimistic .kiro detection**

In the `CommandsUpdated` handler block (lines 174-184), add an optimistic check for `.kiro/settings/lsp.json` after the command registration:

```rust
        if let Notification::CommandsUpdated(ref cmds) = notification {
            self.commands.register_agent_commands(cmds);
            let names: Vec<String> = self
                .commands
                .all_commands()
                .iter()
                .map(|cmd| cmd.name().to_string())
                .collect();
            self.ui_state.set_command_names(names);

            // Optimistic code intelligence detection
            if std::path::Path::new(".kiro/settings/lsp.json").exists() {
                self.session.set_code_intelligence_active(true);
            }
        }
```

**Step 6: Wire code_intelligence_active through to TuiState**

The `TuiState` implementation on `App` (or wherever `App` implements the trait) needs to return `self.session.code_intelligence_active()` for the `code_intelligence_active()` method. Find the `TuiState for App` impl or the delegation layer and add:

```rust
    fn code_intelligence_active(&self) -> bool {
        self.session.code_intelligence_active()
    }
```

If `App` doesn't implement `TuiState` directly (it delegates to `UiState`), then the delegation in `draw()` needs to pass the value. Check how `current_model` gets from `SessionController` → toolbar rendering. Follow the same pattern.

**Step 7: Run full build and tests**

Run: `cargo check` then `cargo test`
Expected: Full compilation. All existing tests pass.

**Step 8: Commit**

```bash
git add crates/cyril/src/app.rs
git commit -m "feat: wire code intelligence response routing and key handling"
```

---

### Task 8: Integration test — full /code lifecycle

**Files:**
- Modify: `crates/cyril/tests/event_routing.rs` (or create new test file)

**Step 1: Write integration tests**

```rust
#[test]
fn code_command_panel_response_opens_overlay() {
    use cyril_core::types::*;
    use cyril_ui::state::UiState;
    use cyril_ui::traits::TuiState;

    let mut ui = UiState::new(500);

    // Simulate a /code CommandExecuted notification
    let notification = Notification::CommandExecuted {
        command: "code".into(),
        response: serde_json::json!({
            "success": true,
            "data": {
                "status": "initialized",
                "message": "Ready",
                "detectedLanguages": ["rust"],
                "projectMarkers": ["Cargo.toml"],
                "lsps": [{
                    "name": "rust-analyzer",
                    "languages": ["rust"],
                    "status": "initialized"
                }]
            }
        }),
    };

    // UiState.apply_notification won't handle code routing
    // (that's App's job), so test the types directly
    let response = &serde_json::json!({
        "success": true,
        "data": {
            "status": "initialized",
            "message": "Ready",
            "detectedLanguages": ["rust"],
            "projectMarkers": ["Cargo.toml"],
            "lsps": [{
                "name": "rust-analyzer",
                "languages": ["rust"],
                "status": "initialized"
            }]
        }
    });

    match CodeCommandResponse::from_json(response) {
        CodeCommandResponse::Panel(data) => {
            assert_eq!(data.status, LspStatus::Initialized);
            ui.show_code_panel(data);
        }
        _ => panic!("Expected Panel response"),
    }

    assert!(ui.has_code_panel());
    assert!(ui.code_panel().is_some());
    let panel = ui.code_panel().unwrap();
    assert_eq!(panel.lsps.len(), 1);
    assert_eq!(panel.lsps[0].name, "rust-analyzer");

    // Close it
    ui.close_code_panel();
    assert!(!ui.has_code_panel());
}

#[test]
fn code_command_prompt_response_detected() {
    use cyril_core::types::*;

    let response = serde_json::json!({
        "success": true,
        "data": {
            "executePrompt": "Summarize the codebase architecture...",
            "label": "Code Summary"
        }
    });

    match CodeCommandResponse::from_json(&response) {
        CodeCommandResponse::Prompt { text, label } => {
            assert!(text.contains("Summarize"));
            assert_eq!(label, Some("Code Summary".into()));
        }
        _ => panic!("Expected Prompt response"),
    }
}

#[test]
fn code_intelligence_active_set_on_initialized_panel() {
    use cyril_core::session::SessionController;
    use cyril_core::types::*;

    let mut session = SessionController::new();
    assert!(!session.code_intelligence_active());

    let response = serde_json::json!({
        "success": true,
        "data": {
            "status": "initialized",
            "detectedLanguages": ["rust"],
            "projectMarkers": [],
            "lsps": []
        }
    });

    if let CodeCommandResponse::Panel(data) = CodeCommandResponse::from_json(&response) {
        if data.status == LspStatus::Initialized {
            session.set_code_intelligence_active(true);
        }
    }

    assert!(session.code_intelligence_active());
}
```

**Step 2: Run the tests**

Run: `cargo test -- code_command`
Expected: All pass.

**Step 3: Commit**

```bash
git add crates/cyril/tests/event_routing.rs
git commit -m "test: add integration tests for code intelligence lifecycle"
```

---

## Summary

| Task | What | Crate | Files |
|------|------|-------|-------|
| 1 | Types + parser | cyril-core | `types/code_panel.rs`, `types/mod.rs` |
| 2 | SessionController field | cyril-core | `session.rs` |
| 3 | UiState + TuiState | cyril-ui | `state.rs`, `traits.rs` |
| 4 | Code panel widget | cyril-ui | `widgets/code_panel.rs`, `widgets/mod.rs` |
| 5 | Render pipeline | cyril-ui | `render.rs` |
| 6 | Toolbar indicator | cyril-ui | `widgets/toolbar.rs` |
| 7 | App wiring | cyril | `app.rs` |
| 8 | Integration tests | cyril | `tests/event_routing.rs` |

Tasks 1-2 are independent. Tasks 3-6 depend on Task 1. Task 7 depends on all prior tasks. Task 8 can be written alongside Task 7.
