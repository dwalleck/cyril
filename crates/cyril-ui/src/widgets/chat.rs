use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};

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

    let total_lines = lines.len();
    let visible_height = area.height as usize;

    // Auto-scroll to bottom
    let scroll_offset = if total_lines > visible_height {
        total_lines.saturating_sub(visible_height)
    } else {
        0
    };

    let chat = Paragraph::new(lines)
        .scroll((scroll_offset as u16, 0))
        .block(Block::default());

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
            lines.push(Line::styled(
                text.clone(),
                Style::default()
                    .fg(Color::DarkGray)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
    }
}

fn render_tool_call(lines: &mut Vec<Line>, tc: &TrackedToolCall) {
    let status_icon = match tc.status() {
        cyril_core::types::ToolCallStatus::InProgress => "⟳",
        cyril_core::types::ToolCallStatus::Pending => "⏳",
        cyril_core::types::ToolCallStatus::Completed => "✓",
        cyril_core::types::ToolCallStatus::Failed => "✗",
    };

    let kind_label = match tc.kind() {
        cyril_core::types::ToolKind::Read => "Read",
        cyril_core::types::ToolKind::Write => "Edit",
        cyril_core::types::ToolKind::Execute => "Run",
        cyril_core::types::ToolKind::Other => "Tool",
    };

    let title = tc.title().unwrap_or(tc.name());
    let color = match tc.status() {
        cyril_core::types::ToolCallStatus::Completed => Color::Green,
        cyril_core::types::ToolCallStatus::Failed => Color::Red,
        _ => Color::Yellow,
    };

    lines.push(Line::from(vec![
        Span::styled(format!("{status_icon} "), Style::default().fg(color)),
        Span::styled(
            kind_label.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("({title})"), Style::default().fg(Color::DarkGray)),
    ]));
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
