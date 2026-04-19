use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::palette;
use crate::traits::{ChatMessage, ChatMessageKind, TrackedToolCall, TuiState};
use crate::widgets::markdown;

/// Render the chat area. If a subagent is focused, renders the focused
/// subagent's stream instead of the main chat.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    // Drill-in: if a subagent is focused, render its stream instead.
    if let Some(focused) = state.subagent_ui().focused_stream() {
        render_subagent_drill_in(frame, area, state, focused);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Render committed messages (includes tool calls in chronological position)
    for msg in state.messages() {
        render_message(&mut lines, msg, area.width as usize);
        lines.push(Line::default()); // spacing between messages
    }

    // Render streaming text
    let streaming = state.streaming_text();
    if !streaming.is_empty() {
        lines.push(Line::styled(
            "Kiro:",
            Style::default()
                .fg(palette::AGENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ));
        let md_lines = markdown::render(streaming, area.width as usize);
        lines.extend(md_lines);
    }

    // Render streaming thought
    if let Some(thought) = state.streaming_thought() {
        lines.push(Line::styled(
            format!("💭 {thought}"),
            Style::default()
                .fg(palette::MUTED_GRAY)
                .add_modifier(Modifier::ITALIC),
        ));
    }

    // Activity indicator — visible in the chat area when the agent is busy
    // but not actively streaming text.
    render_activity_indicator(&mut lines, state);

    let visible_height = area.height as usize;

    let chat = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default());

    // Use line_count to get the wrapped height (accounts for long lines wrapping)
    let total_lines = chat.line_count(area.width);

    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll_offset = match state.chat_scroll_back() {
        None => max_scroll,
        Some(back) => max_scroll.saturating_sub(back),
    };

    if scroll_offset > u16::MAX as usize {
        tracing::warn!(scroll_offset, "scroll offset exceeds u16::MAX, clamping");
    }
    let scroll_clamped = scroll_offset.min(u16::MAX as usize) as u16;
    let chat = chat.scroll((scroll_clamped, 0));

    frame.render_widget(chat, area);

    // Scrollbar
    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Render a focused subagent's message stream in place of the main chat.
/// Shows a header with the subagent name and "[Esc] Back" hint.
fn render_subagent_drill_in(
    frame: &mut Frame,
    area: Rect,
    state: &dyn TuiState,
    stream: &crate::subagent_ui::SubagentStream,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Header bar with subagent name
    let focused_id = state.subagent_ui().focused_session_id();
    let name = focused_id
        .and_then(|id| state.subagent_tracker().get(id))
        .map(|info| info.session_name().to_string())
        .or_else(|| focused_id.map(|id| id.as_str().to_string()))
        .unwrap_or_else(|| "subagent".to_string());

    lines.push(Line::from(vec![
        Span::styled(
            format!("─── {name} "),
            Style::default()
                .fg(palette::USER_BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[Esc] Back", Style::default().fg(palette::MUTED_GRAY)),
    ]));
    lines.push(Line::default());

    // Render committed messages
    for msg in stream.messages() {
        render_message(&mut lines, msg, area.width as usize);
        lines.push(Line::default());
    }

    // Render streaming text
    let streaming = stream.streaming_text();
    if !streaming.is_empty() {
        lines.push(Line::styled(
            format!("{name}:"),
            Style::default()
                .fg(palette::AGENT_GREEN)
                .add_modifier(Modifier::BOLD),
        ));
        let md_lines = markdown::render(streaming, area.width as usize);
        lines.extend(md_lines);
    }

    let visible_height = area.height as usize;

    let chat = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default());

    let total_lines = chat.line_count(area.width);
    let scroll_offset = if total_lines > visible_height {
        total_lines.saturating_sub(visible_height)
    } else {
        0
    };

    let scroll_clamped = scroll_offset.min(u16::MAX as usize) as u16;
    let chat = chat.scroll((scroll_clamped, 0));
    frame.render_widget(chat, area);

    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn render_message(lines: &mut Vec<Line>, msg: &ChatMessage, width: usize) {
    match msg.kind() {
        ChatMessageKind::UserText(text) => {
            lines.push(Line::styled(
                "You:",
                Style::default()
                    .fg(palette::USER_BLUE)
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
                    .fg(palette::AGENT_GREEN)
                    .add_modifier(Modifier::BOLD),
            ));
            let md_lines = markdown::render(text, width);
            lines.extend(md_lines);
        }
        ChatMessageKind::Thought(text) => {
            lines.push(Line::styled(
                format!("💭 {text}"),
                Style::default()
                    .fg(palette::MUTED_GRAY)
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
                .fg(palette::SYSTEM_MAUVE)
                .add_modifier(Modifier::ITALIC);
            for line in text.lines() {
                lines.push(Line::styled(line.to_string(), style));
            }
        }
        ChatMessageKind::CommandOutput { command, text } => {
            lines.push(Line::styled(
                format!("/{command}:"),
                Style::default()
                    .fg(palette::USER_BLUE)
                    .add_modifier(Modifier::BOLD),
            ));
            for line in text.lines() {
                lines.push(Line::raw(format!("  {line}")));
            }
        }
    }
}

/// Render a live activity indicator at the bottom of chat content.
/// Shows a spinner + label + elapsed time when the agent is busy but not
/// actively streaming text (which is already visible).
fn render_activity_indicator(lines: &mut Vec<Line>, state: &dyn TuiState) {
    use crate::traits::Activity;

    let (label, color) = match state.activity() {
        Activity::Sending | Activity::Waiting => ("Thinking...", palette::MUTED_GRAY),
        Activity::ToolRunning => ("Running...", Color::Cyan),
        // Streaming text is already visible — no indicator needed.
        Activity::Streaming | Activity::Idle | Activity::Ready => return,
    };

    let elapsed_dur = state.activity_elapsed();
    let elapsed_secs = elapsed_dur.map(|d| d.as_secs()).unwrap_or(0);
    let spinner_idx = elapsed_dur
        .map(|d| {
            (d.as_millis() / palette::SPINNER_FRAME_MS) as usize % palette::SPINNER_CHARS.len()
        })
        .unwrap_or(0);

    lines.push(Line::from(vec![
        Span::styled(
            format!("{} ", palette::SPINNER_CHARS[spinner_idx]),
            Style::default().fg(color),
        ),
        Span::styled(
            format!("{label} {elapsed_secs}s"),
            Style::default().fg(color),
        ),
    ]));
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
        ToolKind::SwitchMode => tc.title().to_string(),
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
        ToolKind::SwitchMode => Color::Magenta,
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

    render_tool_output(lines, tc);
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

/// Render tool output (shell stdout, errors, file read summary).
///
/// Called after the header and diff rendering in `render_tool_call`. Skips
/// Write-kind tools since they already display diff content.
fn render_tool_output(lines: &mut Vec<Line>, tc: &TrackedToolCall) {
    use cyril_core::types::{ToolCallStatus, ToolKind};

    const MAX_OUTPUT_LINES: usize = 5;
    const INDENT: &str = "    ";

    // Failed tools: show error message
    if tc.status() == ToolCallStatus::Failed {
        if let Some(err) = tc.error_message() {
            lines.push(Line::styled(
                format!("{INDENT}Error: {err}"),
                Style::default().fg(Color::Red),
            ));
        }
        return;
    }

    // Only show output for completed tools
    if tc.status() != ToolCallStatus::Completed {
        return;
    }

    // Write tools already show diff content — skip output rendering
    if tc.kind() == ToolKind::Write {
        return;
    }

    // Execute: show exit code if non-zero
    if let Some(code) = tc.exit_code()
        && code != 0
    {
        lines.push(Line::styled(
            format!("{INDENT}Exit: {code}"),
            Style::default().fg(Color::Yellow),
        ));
    }

    // Read: show char count summary instead of full output
    if tc.kind() == ToolKind::Read {
        if let Some(text) = tc.output_text() {
            let chars = text.len();
            let summary = if chars < 1000 {
                format!("{chars} chars")
            } else {
                format!("{:.1}k chars", chars as f64 / 1000.0)
            };
            lines.push(Line::styled(
                format!("{INDENT}{summary}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        return;
    }

    // Other tools: show output preview
    if let Some(text) = tc.output_text() {
        let output_lines: Vec<&str> = text.lines().collect();
        let total = output_lines.len();
        if total == 0 {
            return;
        }

        let show = total.min(MAX_OUTPUT_LINES);
        for line_text in &output_lines[..show] {
            lines.push(Line::styled(
                format!("{INDENT}| {line_text}"),
                Style::default().fg(Color::DarkGray),
            ));
        }
        if total > show {
            lines.push(Line::styled(
                format!("{INDENT}...{} more lines", total - show),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::traits::test_support::MockTuiState;
    use crate::traits::{Activity, ChatMessage};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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

    #[test]
    fn chat_renders_drill_in_when_subagent_focused() {
        use cyril_core::types::{AgentMessage, Notification, SessionId};

        let mut state = MockTuiState::default();
        // Add a main session message that should NOT appear during drill-in
        state
            .messages
            .push(ChatMessage::agent_text("main session text".into()));

        // Register a subagent in the tracker
        let sub_info = cyril_core::types::SubagentInfo::new(
            SessionId::new("sub-1"),
            "reviewer",
            "code-reviewer",
            "query",
            cyril_core::types::SubagentStatus::Working {
                message: Some("Running".into()),
            },
        );
        state
            .subagent_tracker
            .apply_notification(&Notification::SubagentListUpdated {
                subagents: vec![sub_info],
                pending_stages: vec![],
            });

        // Push a message into the subagent stream
        let sid = SessionId::new("sub-1");
        state.subagent_ui.apply_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "subagent only text".into(),
                is_streaming: false,
            }),
        );
        state.subagent_ui.focus(sid);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer[(x as u16, y as u16)]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                })
            })
            .collect();

        assert!(
            text.contains("reviewer"),
            "drill-in header should show subagent name"
        );
        assert!(
            text.contains("[Esc] Back"),
            "drill-in should show back hint"
        );
        assert!(
            text.contains("subagent only text"),
            "drill-in should show subagent's messages"
        );
        assert!(
            !text.contains("main session text"),
            "drill-in should NOT show main session messages"
        );
    }

    #[test]
    fn chat_scroll_back_offsets_viewport() {
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(ChatMessage::agent_text(format!("Message {i}")));
        }

        // Follow mode — auto-scroll to bottom
        let state_follow = MockTuiState {
            messages: messages.clone(),
            chat_scroll_back: None,
            ..Default::default()
        };

        // Browse mode — scrolled up
        let state_browse = MockTuiState {
            messages,
            chat_scroll_back: Some(30),
            ..Default::default()
        };

        let backend_follow = TestBackend::new(80, 10);
        let mut term_follow = Terminal::new(backend_follow).expect("test terminal");
        term_follow
            .draw(|frame| render(frame, frame.area(), &state_follow))
            .expect("draw");

        let backend_browse = TestBackend::new(80, 10);
        let mut term_browse = Terminal::new(backend_browse).expect("test terminal");
        term_browse
            .draw(|frame| render(frame, frame.area(), &state_browse))
            .expect("draw");

        // Extract first line of each render to verify different content
        let follow_text: String = (0..80)
            .map(|x| {
                term_follow
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let browse_text: String = (0..80)
            .map(|x| {
                term_browse
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();

        assert_ne!(
            follow_text, browse_text,
            "follow mode and browse mode should show different content"
        );
    }

    #[test]
    fn chat_scroll_back_short_content_clamps_to_zero() {
        // When content fits in the viewport, browse mode should still render
        // correctly (offset clamps to 0, no underflow).
        let state = MockTuiState {
            messages: vec![ChatMessage::agent_text("Short".into())],
            chat_scroll_back: Some(100),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .expect("draw should not panic with scroll_back exceeding content");
    }

    #[test]
    fn activity_indicator_shown_for_sending() {
        use std::time::Duration;

        let state = MockTuiState {
            activity: Activity::Sending,
            activity_elapsed: Some(Duration::from_secs(5)),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
            })
            .collect();
        assert!(
            text.contains("Thinking"),
            "Sending state should show Thinking indicator"
        );
        assert!(text.contains("5s"), "should show elapsed seconds");
    }

    #[test]
    fn activity_indicator_not_shown_for_idle() {
        let state = MockTuiState {
            activity: Activity::Idle,
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
            })
            .collect();
        assert!(
            !text.contains("Thinking"),
            "Idle state should not show activity indicator"
        );
    }

    #[test]
    fn render_tool_output_shell_with_exit_code() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "cargo test".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "test result: FAILED\n2 tests failed",
                "exit_status": 1
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Exit: 1"), "should show non-zero exit code");
        assert!(text.contains("test result: FAILED"), "should show stdout");
    }

    #[test]
    fn render_tool_output_shell_zero_exit_code_hidden() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "cargo test".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "test result: ok",
                "exit_status": 0
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("Exit:"), "zero exit code should be hidden");
        assert!(text.contains("test result: ok"), "should show stdout");
    }

    #[test]
    fn render_tool_output_failed_shows_error() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "shell".into(),
                ToolKind::Execute,
                ToolCallStatus::Failed,
                None,
            )
            .with_raw_output(Some(serde_json::json!("Command timed out"))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("Error: Command timed out"),
            "should show error"
        );
    }

    #[test]
    fn render_tool_output_read_shows_char_count() {
        use cyril_core::types::*;

        let content = "a".repeat(2500);
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Read(main.rs)".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({"items": [{"Text": content}]}))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("2.5k chars"),
            "should show char count: got {text}"
        );
    }

    #[test]
    fn render_tool_output_read_small_file_shows_raw_count() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Read(small.rs)".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(
                serde_json::json!({"items": [{"Text": "hello world"}]}),
            )),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("11 chars"),
            "should show raw char count: got {text}"
        );
    }

    #[test]
    fn render_tool_output_write_skipped() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!("written ok"))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        assert!(
            lines.is_empty(),
            "Write tools should not render output (diff is shown instead)"
        );
    }

    #[test]
    fn render_tool_output_truncates_long_output() {
        use cyril_core::types::*;

        let long_output: String = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "shell".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "long-cmd"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": long_output,
                "exit_status": 0
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("...15 more lines"),
            "should show overflow indicator: got {text}"
        );
        // 5 visible lines + 1 overflow indicator = 6 total
        assert_eq!(lines.len(), 6, "should show 5 lines + overflow");
    }

    #[test]
    fn render_tool_output_in_progress_shows_nothing() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::InProgress,
            None,
        ));
        let mut lines = Vec::new();
        render_tool_output(&mut lines, &tc);
        assert!(
            lines.is_empty(),
            "in-progress tools should not render output"
        );
    }
}
