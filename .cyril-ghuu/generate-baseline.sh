#!/usr/bin/env bash
set -euo pipefail

readonly PINNED_COMMIT=80f3ffa5a7ced20e33c9b98c782c08af704407d5
readonly ROOT="$(git rev-parse --show-toplevel)"
readonly OUTPUT="${1:-$ROOT/crates/cyril-ui/src/fixtures/conversation-theme-baseline.tsv}"
readonly WORKTREE="$(mktemp -d "$ROOT/target/cyril-ghuu-baseline-worktree.XXXXXX")"
readonly RAW_OUTPUT="$ROOT/target/cyril-ghuu-baseline-raw.txt"

cleanup() {
  git -C "$ROOT" worktree remove --force "$WORKTREE" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git -C "$ROOT" worktree add --detach "$WORKTREE" "$PINNED_COMMIT" >/dev/null

cat >> "$WORKTREE/crates/cyril-ui/src/render.rs" <<'RUST'

#[cfg(test)]
mod cyril_ghuu_baseline {
    use std::fmt::Write as _;
    use std::time::Duration;

    use cyril_core::types::{
        Plan, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
    };
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Constraint, Layout};
    use ratatui::style::Color;
    use ratatui::widgets::Paragraph;
    use ratatui::Terminal;

    use crate::traits::test_support::MockTuiState;
    use crate::traits::{
        Activity, ChatMessage, ChatMessageKind, SteerEchoStatus, Suggestion, TrackedToolCall,
    };

    fn steer(text: &str, status: SteerEchoStatus) -> ChatMessage {
        ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: text.to_string(),
                status,
            },
            timestamp: std::time::Instant::now(),
        }
    }

    fn message_state() -> MockTuiState {
        MockTuiState {
            messages: vec![
                ChatMessage::user_text("user".into()),
                ChatMessage::agent_text(String::new()),
                ChatMessage::thought("thought".into()),
                ChatMessage::plan(Plan::new(Vec::new())),
                ChatMessage::system("system".into()),
                ChatMessage::command_output("context".into(), String::new()),
                steer("queued", SteerEchoStatus::Queued),
                steer("applied", SteerEchoStatus::Applied),
                steer("cleared", SteerEchoStatus::Cleared),
                steer("unsupported", SteerEchoStatus::Unsupported),
            ],
            activity: Activity::ToolRunning,
            activity_elapsed: Some(Duration::from_secs(1)),
            ..MockTuiState::default()
        }
    }

    fn render_message_scene() -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        let state = message_state();
        terminal.draw(|frame| crate::widgets::chat::render(frame, frame.area(), &state))?;
        Ok(terminal.backend().buffer().clone())
    }

    fn tool_call(
        id: &str,
        title: &str,
        kind: ToolKind,
        status: ToolCallStatus,
    ) -> TrackedToolCall {
        TrackedToolCall::new(ToolCall::new(
            ToolCallId::new(id),
            title.to_string(),
            kind,
            status,
            None,
        ))
    }

    fn tool_states() -> (MockTuiState, MockTuiState) {
        let old_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("old-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let new_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("new-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let write = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("write"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "diff.rs".into(),
                old_text: Some(old_text),
                new_text,
            }])
            .with_locations(vec![ToolCallLocation {
                path: "diff.rs".into(),
                line: Some(1),
            }]),
        );
        let read = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("read"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Pending,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "read.rs".into(),
                line: None,
            }]),
        );
        let execute = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("execute"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "line-1\nline-2\nline-3\nline-4\nline-5\nline-6",
                "exit_status": 1
            }))),
        );
        let right_messages = vec![
            ChatMessage::tool_call(read),
            ChatMessage::tool_call(execute),
            ChatMessage::tool_call(tool_call(
                "search",
                "Search(marker)",
                ToolKind::Search,
                ToolCallStatus::InProgress,
            )),
            ChatMessage::tool_call(tool_call(
                "think",
                "think",
                ToolKind::Think,
                ToolCallStatus::Failed,
            )),
            ChatMessage::tool_call(tool_call(
                "fetch",
                "Fetch(url)",
                ToolKind::Fetch,
                ToolCallStatus::Pending,
            )),
            ChatMessage::tool_call(tool_call(
                "switch",
                "Switch(mode)",
                ToolKind::SwitchMode,
                ToolCallStatus::Completed,
            )),
            ChatMessage::tool_call(tool_call(
                "other",
                "Other(custom)",
                ToolKind::Other,
                ToolCallStatus::Failed,
            )),
        ];
        (
            MockTuiState {
                messages: vec![ChatMessage::tool_call(write)],
                ..MockTuiState::default()
            },
            MockTuiState {
                messages: right_messages,
                ..MockTuiState::default()
            },
        )
    }

    fn render_tool_scene() -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        let (left_state, right_state) = tool_states();
        terminal.draw(|frame| {
            let [left, right] = Layout::horizontal([
                Constraint::Length(40),
                Constraint::Length(40),
            ])
            .areas(frame.area());
            crate::widgets::chat::render(frame, left, &left_state);
            crate::widgets::chat::render(frame, right, &right_state);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn render_markdown_scene() -> anyhow::Result<Buffer> {
        const HEADINGS: &str = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        const STRUCTURE: &str = "- outer\n  - nested\n\n> quote 世界\n\n[repeat](https://example.com) [repeat](https://example.com)";
        const FORMATTING: &str = "| A | B |\n|---|---|\n| same | same |\n\ninline `code` and **bold** *italic* ~~strike~~\n\n---";
        const CODE: &str = "```rust\nfn syntax_rgb() -> u8 { 42 }\n```\n\n```mystery\nunknown_fallback 世界\n```\n\n```\nlanguage_absent\n```";
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| {
            let [left, right] = Layout::horizontal([
                Constraint::Length(40),
                Constraint::Length(40),
            ])
            .areas(frame.area());
            let [heading_area, structure_area, formatting_area] = Layout::vertical([
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Min(1),
            ])
            .areas(left);
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render(HEADINGS, 40)),
                heading_area,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render(STRUCTURE, 40)),
                structure_area,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render(FORMATTING, 40)),
                formatting_area,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render(CODE, 40)),
                right,
            );
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn input_state() -> MockTuiState {
        let suggestions = (0..21)
            .map(|index| {
                let text = match index {
                    7 | 8 => "duplicate".to_string(),
                    10 => "選択".to_string(),
                    11 => "with spaces".to_string(),
                    _ => format!("item-{index}"),
                };
                Suggestion {
                    text,
                    description: (index % 2 == 0).then(|| format!("description-{index}")),
                }
            })
            .collect();
        MockTuiState {
            input_text: "first\nUnicode 世界\nthird".into(),
            input_cursor: "first\nUnicode ".len(),
            autocomplete_suggestions: suggestions,
            autocomplete_selected: Some(10),
            ..MockTuiState::default()
        }
    }

    fn render_input_scene() -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        let state = input_state();
        assert_eq!(crate::widgets::input::height_for(&state), 5);
        assert_eq!(crate::widgets::suggestions::height_for(&state), 10);
        terminal.draw(|frame| {
            let [input_area, suggestions_area, _] = Layout::vertical([
                Constraint::Length(5),
                Constraint::Length(10),
                Constraint::Min(0),
            ])
            .areas(frame.area());
            crate::widgets::input::render(frame, input_area, &state);
            crate::widgets::suggestions::render(frame, suggestions_area, &state);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn normalize_color(color: Color) -> String {
        let rgb = match color {
            Color::Reset => return "DEFAULT".to_string(),
            Color::Black => (0x00, 0x00, 0x00),
            Color::Red => (0x80, 0x00, 0x00),
            Color::Green => (0x00, 0x80, 0x00),
            Color::Yellow => (0x80, 0x80, 0x00),
            Color::Blue => (0x00, 0x00, 0x80),
            Color::Magenta => (0x80, 0x00, 0x80),
            Color::Cyan => (0x00, 0x80, 0x80),
            Color::Gray => (0xc0, 0xc0, 0xc0),
            Color::DarkGray => (0x80, 0x80, 0x80),
            Color::LightRed => (0xff, 0x00, 0x00),
            Color::LightGreen => (0x00, 0xff, 0x00),
            Color::LightYellow => (0xff, 0xff, 0x00),
            Color::LightBlue => (0x00, 0x00, 0xff),
            Color::LightMagenta => (0xff, 0x00, 0xff),
            Color::LightCyan => (0x00, 0xff, 0xff),
            Color::White => (0xff, 0xff, 0xff),
            Color::Rgb(red, green, blue) => (red, green, blue),
            Color::Indexed(index) => return format!("INDEX:{index}"),
        };
        format!("RGB:{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
    }

    fn symbol_hex(symbol: &str) -> String {
        let mut encoded = String::with_capacity(symbol.len() * 2);
        for byte in symbol.as_bytes() {
            write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
        }
        encoded
    }

    fn normalized_rows(scene: &str, buffer: &Buffer) -> String {
        let mut output = String::new();
        for y in 0..24 {
            for x in 0..80 {
                let cell = buffer.cell((x, y)).expect("80x24 cell exists");
                writeln!(
                    &mut output,
                    "{scene}\t{x}\t{y}\t{}\t{}\t{}\t{}",
                    symbol_hex(cell.symbol()),
                    normalize_color(cell.fg),
                    normalize_color(cell.bg),
                    cell.modifier.bits(),
                )
                .expect("writing to String cannot fail");
            }
        }
        output
    }

    #[test]
    fn emit_baseline_scenes() -> anyhow::Result<()> {
        let first = render_message_scene()?;
        let second = render_message_scene()?;
        let first_rows = normalized_rows("messages", &first);
        assert_eq!(first_rows, normalized_rows("messages", &second));
        assert_eq!(first_rows.lines().count(), 1_920);

        let symbols = first
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "You:",
            "Kiro:",
            "thought",
            "Plan:",
            "system",
            "/context:",
            "queued",
            "applied",
            "cleared",
            "not supported",
            "Running... 1s",
        ] {
            assert!(symbols.contains(label), "missing message-scene label {label:?}");
        }

        assert_eq!(normalize_color(Color::Cyan), "RGB:008080");
        assert_eq!(normalize_color(Color::LightCyan), "RGB:00ffff");
        assert_eq!(normalize_color(Color::Reset), "DEFAULT");
        assert_eq!(normalize_color(Color::Rgb(1, 2, 3)), "RGB:010203");
        assert_eq!(normalize_color(Color::Indexed(42)), "INDEX:42");

        let first_tools = render_tool_scene()?;
        let second_tools = render_tool_scene()?;
        let tool_rows = normalized_rows("tools", &first_tools);
        assert_eq!(tool_rows, normalized_rows("tools", &second_tools));
        assert_eq!(tool_rows.lines().count(), 1_920);
        let tool_symbols = first_tools
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "Edit(diff.rs)",
            "│-",
            "│+",
            "│  ",
            "...",
            "Read(read.rs)",
            "Run(cargo test)",
            "Search(marker)",
            "Thinking...",
            "Fetch(url)",
            "Switch(mode)",
            "Other(custom)",
            "Exit: 1",
            "line-1",
            "line-5",
            "...1 more lines",
        ] {
            assert!(tool_symbols.contains(label), "missing tool-scene label {label:?}");
        }
        assert!(!tool_symbols.contains("line-6"));

        let first_markdown = render_markdown_scene()?;
        let second_markdown = render_markdown_scene()?;
        let markdown_rows = normalized_rows("markdown", &first_markdown);
        assert_eq!(
            markdown_rows,
            normalized_rows("markdown", &second_markdown)
        );
        assert_eq!(markdown_rows.lines().count(), 1_920);
        let markdown_symbols = first_markdown
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "H1",
            "H2",
            "H3",
            "H4",
            "H5",
            "H6",
            "outer",
            "nested",
            "quote",
            "世",
            "界",
            "repeat",
            "same",
            "inline",
            "`code`",
            "bold",
            "italic",
            "strike",
            "rust",
            "syntax_rgb",
            "mystery",
            "unknown_fallback",
            "language_absent",
        ] {
            assert!(
                markdown_symbols.contains(label),
                "missing markdown-scene label {label:?}"
            );
        }
        assert!(
            first_markdown
                .content()
                .iter()
                .any(|cell| matches!(cell.fg, Color::Rgb(_, _, _))),
            "known Rust syntax did not produce Syntect RGB"
        );

        let first_input = render_input_scene()?;
        let second_input = render_input_scene()?;
        let input_rows = normalized_rows("input", &first_input);
        assert_eq!(input_rows, normalized_rows("input", &second_input));
        assert_eq!(input_rows.lines().count(), 1_920);
        let input_symbols = first_input
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for label in [
            "first",
            "Unicode",
            "世",
            "界",
            "third",
            "█",
            "duplicate",
            "選",
            "択",
            "with spaces",
            "description-10",
            "▸",
        ] {
            assert!(
                input_symbols.contains(label),
                "missing input-scene label {label:?}"
            );
        }
        assert!(!input_symbols.contains("item-0"));
        assert!(!input_symbols.contains("item-20"));

        println!("BEGIN_CYRIL_GHUU_BASELINE");
        print!("{first_rows}{tool_rows}{markdown_rows}{input_rows}");
        println!("END_CYRIL_GHUU_BASELINE");
        Ok(())
    }
}
RUST

mkdir -p "$(dirname "$OUTPUT")" "$(dirname "$RAW_OUTPUT")"
CARGO_TARGET_DIR="$ROOT/target/cyril-ghuu-baseline" \
  cargo test --manifest-path "$WORKTREE/Cargo.toml" -p cyril-ui \
  render::cyril_ghuu_baseline::emit_baseline_scenes -- --exact --nocapture > "$RAW_OUTPUT"

{
  printf 'commit\t%s\n' "$PINNED_COMMIT"
  printf 'scene\tx\ty\tsymbol_hex\tforeground\tbackground\tmodifier_bits\n'
  awk '/BEGIN_CYRIL_GHUU_BASELINE/{capture=1; next} /END_CYRIL_GHUU_BASELINE/{capture=0} capture' "$RAW_OUTPUT"
} > "$OUTPUT"

data_rows=$(awk 'NR > 2 {count++} END {print count + 0}' "$OUTPUT")
if [[ "$data_rows" -ne 7680 ]]; then
  printf 'expected 7680 baseline cells, found %s\n' "$data_rows" >&2
  exit 1
fi
if [[ "$(head -n 1 "$OUTPUT")" != $'commit\t'"$PINNED_COMMIT" ]]; then
  printf 'baseline commit header mismatch\n' >&2
  exit 1
fi
printf 'generated %s cells at %s\n' "$data_rows" "$OUTPUT" >&2
