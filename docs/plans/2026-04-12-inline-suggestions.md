# Inline Suggestions Panel — Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the small overlay autocomplete dropdown with an inline suggestions panel below the input area, showing all matching commands with descriptions, filtering as the user types, and scrolling support.

**Architecture:** Add a new layout region between the input area and the status bar that dynamically grows from 0 to N rows when suggestions are active. The existing autocomplete state machine (selection, navigation, filtering) stays unchanged — only the rendering changes. Remove the overlay-based `render_autocomplete` from `input.rs` and render suggestions as a first-class layout area in `render.rs`.

**Tech Stack:** Rust, ratatui 0.30 (Layout constraints, Paragraph widget)

---

## Current State

- `render.rs:23-30` — Layout: toolbar / chat / crew / input(5) / status(1)
- `input.rs:33-39` — Calls `render_autocomplete()` as an overlay on top of chat
- `input.rs:42-87` — `render_autocomplete()`: shows first 8 suggestions in a popup above input, no scrolling, no descriptions
- `state.rs` — `autocomplete_suggestions`, `autocomplete_selected`, `autocomplete_prev/next`, `handle_autocomplete_key`
- `state.rs:43` — `command_names: Vec<String>` — stores only names, no descriptions
- `app.rs:38` — Maps `c.name().to_string()` dropping descriptions when building the name list
- `traits.rs:218` — `Suggestion { text, description }` — already has description field, but it's always `None` for slash commands

## Key Facts

- **ACP provides descriptions**: `cmd.description()` returns `&str` on `Command` trait (line 107 of `commands/mod.rs`). `PromptInfo::description()` returns `Option<&str>` (line 71 of `types/prompt.rs`).
- **`@file` autocomplete is unchanged**: File completions use a different code path in `update_autocomplete` (line ~745 of `state.rs`). They populate `Suggestion { description: None }` and will render fine in the new panel without descriptions. Do NOT modify the file completion path.
- **Autocomplete state machine is unchanged**: `autocomplete_next()`, `autocomplete_prev()`, `handle_autocomplete_key()`, `accept_autocomplete()` — all work by index into `autocomplete_suggestions`. The selection index already ranges over ALL suggestions (not capped at 8). Only the old RENDERER was broken (`.take(8)` with no sliding window).

## Files Involved

- **Modify:** `crates/cyril-ui/src/state.rs` — Change `command_names: Vec<String>` to `command_info: Vec<(String, String)>` carrying `(name, description)`
- **Modify:** `crates/cyril/src/app.rs` — Pass `(name, description)` pairs when populating command info
- **Modify:** `crates/cyril-ui/src/render.rs` — Add suggestions area to layout
- **Modify:** `crates/cyril-ui/src/widgets/input.rs` — Remove `render_autocomplete`, render input only
- **Create:** `crates/cyril-ui/src/widgets/suggestions.rs` — New widget for inline suggestions panel
- **Modify:** `crates/cyril-ui/src/widgets/mod.rs` — Register new module

---

### Task 0: Plumb command descriptions into autocomplete suggestions

**Files:**
- Modify: `crates/cyril-ui/src/state.rs`
- Modify: `crates/cyril/src/app.rs`

**Step 1: Change `command_names` to carry descriptions**

In `state.rs`, change the field (line 43):
```rust
// Before:
command_names: Vec<String>,
// After:
command_info: Vec<(String, String)>,  // (name, description)
```

Update `new()` (line ~201) to initialize as `command_info: Vec::new()`.

Change `set_command_names` (line ~660) to:
```rust
pub fn set_command_info(&mut self, info: Vec<(String, String)>) {
    self.command_info = info;
}
```

Update `update_autocomplete` (line ~723, the slash command branch) to use descriptions:
```rust
// In the slash command branch — currently:
self.autocomplete_suggestions = self
    .command_names
    .iter()
    .filter(|name| name.to_lowercase().contains(&query))
    .map(|name| Suggestion {
        text: format!("/{name}"),
        description: None,
    })
    .collect();

// Change to:
self.autocomplete_suggestions = self
    .command_info
    .iter()
    .filter(|(name, _)| name.to_lowercase().contains(&query))
    .map(|(name, desc)| Suggestion {
        text: format!("/{name}"),
        description: Some(desc.clone()),
    })
    .collect();
```

**DO NOT modify** the `@file` completion branch (~line 745). It uses `FileCompleter` and stays unchanged.

**Step 2: Update callers in `app.rs`**

In `App::new()` (line ~36-41), change:
```rust
// Before:
let names: Vec<String> = commands
    .all_commands()
    .iter()
    .map(|c| c.name().to_string())
    .collect();
ui_state.set_command_names(names);

// After:
let info: Vec<(String, String)> = commands
    .all_commands()
    .iter()
    .map(|c| (c.name().to_string(), c.description().to_string()))
    .collect();
ui_state.set_command_info(info);
```

In the `CommandsUpdated` notification handler (line ~237-253), change:
```rust
// Before:
let mut names: Vec<String> = self
    .commands
    .all_commands()
    .iter()
    .map(|cmd| cmd.name().to_string())
    .collect();
for prompt in prompt_list {
    names.push(prompt.name().to_string());
}
self.ui_state.set_command_names(names);

// After:
let mut info: Vec<(String, String)> = self
    .commands
    .all_commands()
    .iter()
    .map(|cmd| (cmd.name().to_string(), cmd.description().to_string()))
    .collect();
for prompt in prompt_list {
    let desc = prompt.description().unwrap_or("").to_string();
    info.push((prompt.name().to_string(), desc));
}
self.ui_state.set_command_info(info);
```

Note: `Command::description()` returns `&str` (always present). `PromptInfo::description()` returns `Option<&str>` (may be absent — use `unwrap_or("")`).

**Step 3: Verify**

```sh
cargo test
```

**Step 4: Commit**

---

### Task 1: Create the suggestions widget module

**Files:**
- Create: `crates/cyril-ui/src/widgets/suggestions.rs`
- Modify: `crates/cyril-ui/src/widgets/mod.rs`

**Step 1: Create the widget with a `height_for` sizing function and `render` function**

The widget shows filtered suggestions in a borderless list below the input. Each suggestion is one line: the command name + description. The selected item is highlighted. A sliding window ensures the selected item is always visible.

```rust
// suggestions.rs
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::palette;
use crate::traits::TuiState;

const MAX_VISIBLE: usize = 10;

/// Compute the height needed for the suggestions panel.
/// Returns 0 when no suggestions are active.
pub fn height_for(state: &dyn TuiState) -> u16 {
    let count = state.autocomplete_suggestions().len();
    if count == 0 {
        return 0;
    }
    count.min(MAX_VISIBLE) as u16
}

/// Render the inline suggestions panel.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let suggestions = state.autocomplete_suggestions();
    let selected = state.autocomplete_selected();
    if suggestions.is_empty() {
        return;
    }

    let total = suggestions.len();
    let visible = total.min(MAX_VISIBLE);

    // Sliding window: keep selected item visible.
    // The window starts at 0 and shifts forward as selection moves past
    // the visible range. When selection moves back, window shifts back.
    let sel = selected.unwrap_or(0);
    let start = if sel >= visible {
        sel - visible + 1
    } else {
        0
    };
    let end = (start + visible).min(total);

    let mut lines: Vec<Line> = Vec::new();
    for i in start..end {
        let s = &suggestions[i];
        let is_selected = Some(i) == selected;

        let mut spans = Vec::new();

        // Selection indicator
        let prefix = if is_selected { "▸ " } else { "  " };

        // Command name
        let name_style = if is_selected {
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(palette::USER_BLUE)
        };
        spans.push(Span::styled(format!("{prefix}{}", s.text), name_style));

        // Description (if available)
        if let Some(ref desc) = s.description {
            let desc_style = if is_selected {
                Style::default().fg(palette::MUTED_GRAY)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            spans.push(Span::styled(format!("  {desc}"), desc_style));
        }

        let mut line = Line::from(spans);
        if is_selected {
            line.style = Style::default().bg(palette::CODE_BLOCK_BG);
        }
        lines.push(line);
    }

    let panel = Paragraph::new(lines);
    frame.render_widget(panel, area);
}
```

**Step 2: Register the module in `widgets/mod.rs`**

Add `pub mod suggestions;` to the module file.

**Step 3: Verify compilation**

```sh
cargo check -p cyril-ui
```

**Step 4: Commit**

---

### Task 2: Update the layout to include the suggestions area

**Files:**
- Modify: `crates/cyril-ui/src/render.rs`

**Step 1: Add suggestions height computation and layout area**

In `draw_inner`, compute the suggestions height and add it between input and status bar:

```rust
fn draw_inner(frame: &mut Frame, state: &dyn TuiState) {
    let area = frame.area();

    let crew_height = crate::widgets::crew_panel::height_for(state);
    let suggestions_height = crate::widgets::suggestions::height_for(state);

    let [toolbar_area, chat_area, crew_area, input_area, suggestions_area, status_area] =
        Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(5),
            Constraint::Length(crew_height),
            Constraint::Length(5),
            Constraint::Length(suggestions_height),
            Constraint::Length(1),
        ])
        .areas(area);

    crate::widgets::toolbar::render(frame, toolbar_area, state);
    crate::widgets::chat::render(frame, chat_area, state);
    if crew_height > 0 {
        crate::widgets::crew_panel::render(frame, crew_area, state);
    }
    crate::widgets::input::render(frame, input_area, state);
    if suggestions_height > 0 {
        crate::widgets::suggestions::render(frame, suggestions_area, state);
    }
    crate::widgets::toolbar::render_status_bar(frame, status_area, state);

    // Overlays (rendered on top — unchanged)
    if let Some(approval) = state.approval() {
        crate::widgets::approval::render(frame, area, approval);
    }
    if let Some(picker) = state.picker() {
        crate::widgets::picker::render(frame, area, picker);
    }
    if let Some(hooks) = state.hooks_panel() {
        crate::widgets::hooks_panel::render(frame, area, hooks);
    }
}
```

When `suggestions_height` is 0 (no active suggestions), the layout is identical to the current one — the area has 0 height and the chat area uses all flexible space.

**Step 2: Verify compilation and test**

```sh
cargo test -p cyril-ui
```

**Step 3: Commit**

---

### Task 3: Remove the old overlay autocomplete from input.rs

**Files:**
- Modify: `crates/cyril-ui/src/widgets/input.rs`

**Step 1: Remove `render_autocomplete` function and the call to it**

Delete the entire `render_autocomplete` function (lines 42-87) and remove the call block from `render` (lines 33-39):

```rust
// DELETE these lines from render():
    // Render autocomplete dropdown if active
    let suggestions = state.autocomplete_suggestions();
    let selected = state.autocomplete_selected();

    if !suggestions.is_empty() {
        render_autocomplete(frame, area, suggestions, selected);
    }
```

The `render` function should end after `frame.render_widget(input_widget, area);`.

Also remove unused imports: `Clear` is no longer needed.

**Step 2: Update the `input_renders_with_suggestions` test**

The test previously rendered with suggestions and expected the overlay. Now suggestions are rendered by a separate widget in a separate area. Update the test to verify the input renders correctly when suggestions are present (autocomplete state exists but the input widget itself just shows the text box):

```rust
#[test]
fn input_renders_with_suggestions() {
    // Suggestions are now rendered by the suggestions widget, not input.
    // This test just verifies the input box renders fine when suggestions
    // are active in state.
    let state = MockTuiState {
        input_text: "/mo".into(),
        input_cursor: 3,
        autocomplete_suggestions: vec![
            crate::traits::Suggestion {
                text: "/model".into(),
                description: Some("Switch model".into()),
            },
        ],
        autocomplete_selected: Some(0),
        ..Default::default()
    };

    let backend = TestBackend::new(80, 5);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| {
            render(frame, frame.area(), &state);
        })
        .expect("draw");
}
```

**Step 3: Verify**

```sh
cargo test -p cyril-ui
```

**Step 4: Commit**

---

### Task 4: Add tests for the suggestions widget

**Files:**
- Modify: `crates/cyril-ui/src/widgets/suggestions.rs`

**Step 1: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use crate::traits::Suggestion;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn height_for_returns_zero_when_no_suggestions() {
        let state = MockTuiState::default();
        assert_eq!(height_for(&state), 0);
    }

    #[test]
    fn height_for_caps_at_max_visible() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        assert_eq!(height_for(&state), MAX_VISIBLE as u16);
    }

    #[test]
    fn height_for_matches_count_when_fewer_than_max() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion { text: "/a".into(), description: None },
                Suggestion { text: "/b".into(), description: None },
            ],
            ..Default::default()
        };
        assert_eq!(height_for(&state), 2);
    }

    #[test]
    fn render_shows_selected_item() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion { text: "/model".into(), description: Some("Switch model".into()) },
                Suggestion { text: "/mode".into(), description: Some("Switch mode".into()) },
                Suggestion { text: "/new".into(), description: None },
            ],
            autocomplete_selected: Some(1),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, frame.area(), &state);
        }).expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..3).flat_map(|y| {
            (0..80).map(move |x|
                buffer.cell((x, y)).map(|c| c.symbol().to_string()).unwrap_or_default()
            )
        }).collect();
        assert!(text.contains("/model"), "should show /model");
        assert!(text.contains("/mode"), "should show /mode");
        assert!(text.contains("▸"), "should show selection indicator");
    }

    #[test]
    fn render_shows_descriptions() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion { text: "/model".into(), description: Some("Switch model".into()) },
            ],
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, frame.area(), &state);
        }).expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..80).map(|x|
            buffer.cell((x, 0)).map(|c| c.symbol().to_string()).unwrap_or_default()
        ).collect();
        assert!(text.contains("Switch model"), "should show description");
    }

    #[test]
    fn render_scrolls_to_selected_middle() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(15),
            ..Default::default()
        };
        let backend = TestBackend::new(80, MAX_VISIBLE as u16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, frame.area(), &state);
        }).expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..MAX_VISIBLE).flat_map(|y| {
            (0..80).map(move |x|
                buffer.cell((x, y as u16)).map(|c| c.symbol().to_string()).unwrap_or_default()
            )
        }).collect();
        assert!(text.contains("/cmd15"), "should show selected item /cmd15 when scrolled");
        assert!(!text.contains("/cmd0"), "should NOT show /cmd0 when scrolled to 15");
    }

    #[test]
    fn render_scrolls_to_last_item() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(19),  // last item
            ..Default::default()
        };
        let backend = TestBackend::new(80, MAX_VISIBLE as u16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, frame.area(), &state);
        }).expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..MAX_VISIBLE).flat_map(|y| {
            (0..80).map(move |x|
                buffer.cell((x, y as u16)).map(|c| c.symbol().to_string()).unwrap_or_default()
            )
        }).collect();
        assert!(text.contains("/cmd19"), "should show last item /cmd19");
    }

    #[test]
    fn render_no_panic_with_empty_suggestions() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 0);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, Rect::new(0, 0, 80, 0), &state);
        }).expect("draw should not panic with no suggestions");
    }

    #[test]
    fn file_suggestions_render_without_description() {
        // @file completions have description: None — they should render
        // fine without a description span.
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion { text: "@src/main.rs".into(), description: None },
                Suggestion { text: "@src/lib.rs".into(), description: None },
            ],
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 2);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal.draw(|frame| {
            render(frame, frame.area(), &state);
        }).expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..2).flat_map(|y| {
            (0..80).map(move |x|
                buffer.cell((x, y)).map(|c| c.symbol().to_string()).unwrap_or_default()
            )
        }).collect();
        assert!(text.contains("@src/main.rs"), "should show file suggestion");
    }
}
```

**Step 2: Verify**

```sh
cargo test -p cyril-ui -- suggestions
```

**Step 3: Commit**

---

### Task 5: Full verification

**Step 1: Run full test suite**

```sh
cargo test
```

All tests must pass, including:
- Existing autocomplete state tests (selection, navigation, accept)
- New suggestions widget tests (height, render, scroll, descriptions)
- Input widget tests (no longer renders autocomplete overlay)
- Render tests (layout includes suggestions area)

**Step 2: Manual testing**

```sh
cargo run
```

Test:
- Type `/` — suggestions panel appears below input with all commands + descriptions
- Type `/mo` — list filters to matching commands only
- Arrow down past the 10th item — panel scrolls, selected item stays visible
- Arrow down to last item — still visible, no panic
- Arrow up back to top — panel scrolls back
- Enter — accepts the selected command, panel disappears, input filled
- Esc — dismisses suggestions, panel disappears
- Type `@` — file completion still works (no descriptions, same panel)
- Layout: chat area should shrink when panel appears, status bar stays at bottom
- Overlays (approval dialog, picker) still render on top correctly

**Step 3: Commit**

---

## Verification Checklist

1. `cargo build` — compiles clean, no warnings
2. `cargo test` — all tests pass
3. `@file` autocomplete unchanged — works without descriptions
4. Slash command suggestions now show descriptions from ACP
5. Sliding window: all N commands accessible via arrow keys, not capped at 8
6. Layout: suggestions panel is 0-height when inactive, no visual change
7. Layout: suggestions panel grows dynamically, chat area shrinks
8. Overlays render on top of suggestions panel correctly
