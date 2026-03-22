use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::traits::{ChatMessage, ChatMessageKind, TrackedToolCall, TuiState};
use crate::widgets::markdown;

/// Render the chat area.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut lines: Vec<Line> = Vec::new();

    // Render committed messages
    for msg in state.messages() {
        render_message(&mut lines, msg);
        lines.push(Line::default()); // spacing between messages
    }

    // Render active tool calls
    for tc in state.active_tool_calls() {
        render_tool_call(&mut lines, tc);
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
                tc.title().unwrap_or(tc.name()).to_string()
            }
        }
        ToolKind::Write => {
            if let Some(path) = tc.primary_path() {
                format!("Edit({path})")
            } else {
                tc.title().unwrap_or(tc.name()).to_string()
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
                tc.title().unwrap_or(tc.name()).to_string()
            }
        }
        ToolKind::Search => tc.title().unwrap_or("Search").to_string(),
        ToolKind::Think => "Thinking...".to_string(),
        ToolKind::Fetch => tc.title().unwrap_or("Fetch").to_string(),
        ToolKind::Other => tc.title().unwrap_or(tc.name()).to_string(),
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
