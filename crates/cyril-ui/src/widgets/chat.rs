use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::traits::{ChatMessage, ChatMessageKind, TrackedToolCall, TuiState};
use crate::widgets::markdown;

/// Render the chat area.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut lines: Vec<Line> = Vec::new();

    // Render committed messages (includes tool calls in chronological position)
    for msg in state.messages() {
        render_message(&mut lines, msg);
        lines.push(Line::default()); // spacing between messages
    }

    // Render streaming text
    let streaming = state.streaming_text();
    if !streaming.is_empty() {
        lines.push(Line::styled(
            "Kiro:",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
        let md_lines = markdown::render(streaming);
        lines.extend(md_lines);
    }

    // Render streaming thought
    if let Some(thought) = state.streaming_thought() {
        lines.push(Line::styled(
            format!("💭 {thought}"),
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    let visible_height = area.height as usize;

    let chat = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default());

    // Use line_count to get the wrapped height (accounts for long lines wrapping)
    let total_lines = chat.line_count(area.width);

    // Auto-scroll to bottom
    let scroll_offset = if total_lines > visible_height {
        total_lines.saturating_sub(visible_height)
    } else {
        0
    };

    let chat = chat.scroll((scroll_offset as u16, 0));

    frame.render_widget(chat, area);

    // Scrollbar
    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_message(lines: &mut Vec<Line>, msg: &ChatMessage) {
    match msg.kind() {
        ChatMessageKind::UserText(text) => {
            lines.push(Line::styled(
                "You:",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            for line in text.lines() {
                lines.push(Line::raw(format!("  {line}")));
            }
        }
        ChatMessageKind::AgentText(text) => {
            lines.push(Line::styled(
                "Kiro:",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ));
            let md_lines = markdown::render(text);
            lines.extend(md_lines);
        }
        ChatMessageKind::Thought(text) => {
            lines.push(Line::styled(
                format!("💭 {text}"),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        ChatMessageKind::ToolCall(tc) => {
            render_tool_call(lines, tc);
        }
        ChatMessageKind::Plan(plan) => {
            lines.push(Line::styled(
                "Plan:",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
            for entry in plan.entries() {
                let icon = match entry.status() {
                    cyril_core::types::PlanEntryStatus::Pending => "○",
                    cyril_core::types::PlanEntryStatus::InProgress => "◐",
                    cyril_core::types::PlanEntryStatus::Completed => "●",
                    cyril_core::types::PlanEntryStatus::Failed => "✗",
                };
                lines.push(Line::raw(format!("  {icon} {}", entry.title())));
            }
        }
        ChatMessageKind::System(text) => {
            let style = Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC);
            for line in text.lines() {
                lines.push(Line::styled(line.to_string(), style));
            }
        }
        ChatMessageKind::CommandOutput { command, text } => {
            lines.push(Line::styled(
                format!("/{command}:"),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
            for line in text.lines() {
                lines.push(Line::raw(format!("  {line}")));
            }
        }
    }
}

fn render_tool_call(lines: &mut Vec<Line>, tc: &TrackedToolCall) {
    use cyril_core::types::{ToolCallStatus, ToolKind};

    let status_icon = match tc.status() {
        ToolCallStatus::InProgress => "⟳",
        ToolCallStatus::Pending => "⏳",
        ToolCallStatus::Completed => "✓",
        ToolCallStatus::Failed => "✗",
    };

    let label = match tc.kind() {
        ToolKind::Read => {
            if let Some(path) = tc.primary_path() {
                format!("Read({path})")
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Write => {
            if let Some(path) = tc.primary_path() {
                format!("Edit({path})")
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Execute => {
            if let Some(cmd) = tc.command_text() {
                let display: String = cmd.chars().take(50).collect();
                if cmd.len() > 50 {
                    format!("Run({display}...)")
                } else {
                    format!("Run({display})")
                }
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Search => tc.title().to_string(),
        ToolKind::Think => "Thinking...".to_string(),
        ToolKind::Fetch => tc.title().to_string(),
        ToolKind::Other => tc.title().to_string(),
    };

    let color = match tc.status() {
        ToolCallStatus::Completed => Color::Green,
        ToolCallStatus::Failed => Color::Red,
        _ => Color::Yellow,
    };

    let kind_color = match tc.kind() {
        ToolKind::Read => Color::Blue,
        ToolKind::Write => Color::Magenta,
        ToolKind::Execute => Color::Yellow,
        ToolKind::Search => Color::Cyan,
        ToolKind::Think => Color::DarkGray,
        ToolKind::Fetch => Color::Cyan,
        ToolKind::Other => Color::White,
    };

    let mut header_spans = vec![
        Span::styled(format!("{status_icon} "), Style::default().fg(color)),
        Span::styled(label, Style::default().fg(kind_color)),
    ];

    if let Some((added, removed)) = compute_diff_summary(tc) {
        header_spans.push(Span::styled(
            format!("  +{added} -{removed}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    lines.push(Line::from(header_spans));

    if tc.status() == ToolCallStatus::Completed && tc.kind() == ToolKind::Write {
        render_diff_lines(lines, tc);
    }
}

/// Compute (added, removed) line counts from diff content using `similar`.
fn compute_diff_summary(tc: &TrackedToolCall) -> Option<(usize, usize)> {
    use similar::{ChangeTag, TextDiff};

    for content in tc.content() {
        if let cyril_core::types::ToolCallContent::Diff {
            old_text, new_text, ..
        } = content
        {
            let old = old_text.as_deref().unwrap_or("");
            let diff = TextDiff::from_lines(old, new_text);
            let mut added = 0usize;
            let mut removed = 0usize;
            for change in diff.iter_all_changes() {
                match change.tag() {
                    ChangeTag::Insert => added += 1,
                    ChangeTag::Delete => removed += 1,
                    ChangeTag::Equal => {}
                }
            }
            if added > 0 || removed > 0 {
                return Some((added, removed));
            }
        }
    }
    None
}

/// Render actual diff lines with line numbers for edit operations.
/// Uses the `similar` crate for proper diff computation with context lines.
fn render_diff_lines(lines: &mut Vec<Line>, tc: &TrackedToolCall) {
    use similar::{ChangeTag, TextDiff};

    const MAX_DIFF_LINES: usize = 20;

    for content in tc.content() {
        if let cyril_core::types::ToolCallContent::Diff {
            old_text, new_text, ..
        } = content
        {
            let old = old_text.as_deref().unwrap_or("");
            let diff = TextDiff::from_lines(old, new_text);
            let mut count = 0;

            for group in diff.grouped_ops(1) {
                for op in &group {
                    for change in diff.iter_changes(op) {
                        if count >= MAX_DIFF_LINES {
                            lines.push(Line::styled(
                                "      ...".to_string(),
                                Style::default().fg(Color::DarkGray),
                            ));
                            return;
                        }

                        let line_text = change.value().trim_end_matches('\n');

                        let (prefix, color) = match change.tag() {
                            ChangeTag::Delete => {
                                let line_no = change.old_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │- "), Color::Red)
                            }
                            ChangeTag::Insert => {
                                let line_no = change.new_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │+ "), Color::Green)
                            }
                            ChangeTag::Equal => {
                                let line_no = change.new_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │  "), Color::DarkGray)
                            }
                        };

                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(color)),
                            Span::styled(
                                line_text.to_string(),
                                if change.tag() == ChangeTag::Equal {
                                    Style::default().fg(Color::DarkGray)
                                } else {
                                    Style::default().fg(color)
                                },
                            ),
                        ]));

                        count += 1;
                    }
                }
            }

            // Only render first diff block
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use crate::traits::ChatMessage;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn chat_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn chat_renders_messages() {
        let state = MockTuiState {
            messages: vec![
                ChatMessage::user_text("Hello".into()),
                ChatMessage::agent_text("Hi there!".into()),
                ChatMessage::system("Session started".into()),
            ],
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn render_tool_call_with_diff_shows_line_numbers() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Editing main.rs".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "src/main.rs".into(),
                old_text: Some("fn main() {\n    println!(\"old\");\n}\n".into()),
                new_text: "fn main() {\n    println!(\"new\");\n    println!(\"extra\");\n}\n"
                    .into(),
            }])
            .with_locations(vec![ToolCallLocation {
                path: "src/main.rs".into(),
                line: Some(1),
            }]),
        );

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc);

        // Header should have label and diff summary
        let header = lines[0].to_string();
        assert!(
            header.contains("Edit"),
            "header should contain Edit label: {header}"
        );
        assert!(
            header.contains("+"),
            "header should contain diff summary: {header}"
        );

        // Should have diff lines with line numbers and │ separator
        let diff_lines: Vec<String> = lines[1..].iter().map(|l| l.to_string()).collect();
        let has_gutter = diff_lines.iter().any(|l| l.contains('│'));
        assert!(
            has_gutter,
            "diff lines should have │ gutter separator: {diff_lines:?}"
        );

        let has_add = diff_lines.iter().any(|l| l.contains("│+"));
        let has_del = diff_lines.iter().any(|l| l.contains("│-"));
        assert!(has_add, "should have added lines: {diff_lines:?}");
        assert!(has_del, "should have removed lines: {diff_lines:?}");
    }

    #[test]
    fn render_tool_call_diff_summary_counts() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "file.rs".into(),
                old_text: Some("line1\nline2\nline3\n".into()),
                new_text: "line1\nchanged\nline3\nnew_line\n".into(),
            }]),
        );

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc);

        // Header should show +2 -1 (one changed + one added = 2 inserts, 1 delete)
        let header = lines[0].to_string();
        assert!(header.contains('+'), "should show additions: {header}");
        assert!(header.contains('-'), "should show removals: {header}");
    }

    #[test]
    fn render_tool_call_no_diff_for_read() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading file.rs".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        ));

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc);

        // Read tool calls should only have a header, no diff lines
        assert_eq!(
            lines.len(),
            1,
            "read tool call should only have header line"
        );
    }

    #[test]
    fn render_tool_call_diff_respects_max_lines() {
        use cyril_core::types::*;

        // Create a large diff that exceeds MAX_DIFF_LINES
        let old_text: String = (0..30).map(|i| format!("old line {i}\n")).collect();
        let new_text: String = (0..30).map(|i| format!("new line {i}\n")).collect();

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "big.rs".into(),
                old_text: Some(old_text),
                new_text,
            }]),
        );

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc);

        // Should have header + at most 20 diff lines + "..." overflow
        let last_line = lines.last().map(|l| l.to_string()).unwrap_or_default();
        assert!(
            last_line.contains("..."),
            "large diff should show overflow indicator: {last_line}"
        );
        // Total lines should be capped (header + <=21 diff lines including overflow)
        assert!(
            lines.len() <= 23,
            "should be capped, got {} lines",
            lines.len()
        );
    }

    #[test]
    fn render_tool_call_smart_labels() {
        use cyril_core::types::*;

        // Read with location
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "src/main.rs".into(),
                line: None,
            }]),
        );
        let mut lines = Vec::new();
        render_tool_call(&mut lines, &tc);
        let header = lines[0].to_string();
        assert!(
            header.contains("Read(src/main.rs)"),
            "should show Read(path): {header}"
        );

        // Execute with command
        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_2"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            Some(serde_json::json!({"command": "cargo test"})),
        ));
        let mut lines = Vec::new();
        render_tool_call(&mut lines, &tc);
        let header = lines[0].to_string();
        assert!(
            header.contains("Run(cargo test)"),
            "should show Run(cmd): {header}"
        );
    }

    #[test]
    fn chat_renders_interleaved_text_and_tool_calls_in_order() {
        use cyril_core::types::*;

        // Simulate a committed turn: text → tool call → text
        let state = MockTuiState {
            messages: vec![
                ChatMessage::agent_text("I'll edit that file.".into()),
                ChatMessage::tool_call(TrackedToolCall::new(
                    ToolCall::new(
                        ToolCallId::new("tc_1"),
                        "Editing main.rs".into(),
                        ToolKind::Write,
                        ToolCallStatus::Completed,
                        None,
                    )
                    .with_content(vec![ToolCallContent::Diff {
                        path: "src/main.rs".into(),
                        old_text: Some("old".into()),
                        new_text: "new".into(),
                    }]),
                )),
                ChatMessage::agent_text("Done editing.".into()),
            ],
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        // Extract rendered text from the buffer
        let buffer = terminal.backend().buffer().clone();
        let rendered: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Verify order: first text appears before tool call, tool call before second text
        let first_text_pos = rendered
            .find("edit that file")
            .expect("first text should render");
        let tool_call_pos = rendered
            .find("Edit(")
            .or_else(|| rendered.find("main.rs"))
            .expect("tool call should render");
        let second_text_pos = rendered
            .find("Done editing")
            .expect("second text should render");

        assert!(
            first_text_pos < tool_call_pos,
            "first text ({first_text_pos}) should appear before tool call ({tool_call_pos})"
        );
        assert!(
            tool_call_pos < second_text_pos,
            "tool call ({tool_call_pos}) should appear before second text ({second_text_pos})"
        );
    }

    #[test]
    fn chat_renders_streaming() {
        let state = MockTuiState {
            streaming_text: "Streaming **markdown** content".into(),
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }
}
