use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::traits::{ApprovalPhase, ApprovalState};

/// Render the permission approval overlay (centered popup).
pub fn render(frame: &mut Frame, area: Rect, state: &ApprovalState) {
    match state.phase {
        ApprovalPhase::SelectOption => render_option_phase(frame, area, state),
        ApprovalPhase::SelectTrust => render_trust_phase(frame, area, state),
    }
}

fn render_option_phase(frame: &mut Frame, area: Rect, state: &ApprovalState) {
    let width = 60.min(area.width.saturating_sub(4));
    let height = (state.options.len() as u16 + 6).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::styled(
        &state.message,
        Style::default().fg(Color::Yellow),
    ));
    lines.push(Line::default());

    for (i, opt) in state.options.iter().enumerate() {
        let style = if i == state.selected {
            Style::default()
                .bg(Color::Rgb(50, 50, 70))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let prefix = if i == state.selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{}", opt.label), style));
    }

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                " Permission Required ",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(popup, popup_area);
}

fn render_trust_phase(frame: &mut Frame, area: Rect, state: &ApprovalState) {
    let width = 64.min(area.width.saturating_sub(4));
    // Each trust option: label line + display line + blank = 3 lines, plus header
    let content_lines = (state.trust_options.len() as u16 * 3) + 4;
    let height = content_lines.min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::styled(
        "Select trust level:",
        Style::default().fg(Color::Cyan),
    ));
    lines.push(Line::default());

    for (i, trust) in state.trust_options.iter().enumerate() {
        let style = if i == state.selected {
            Style::default()
                .bg(Color::Rgb(50, 50, 70))
                .fg(Color::White)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let prefix = if i == state.selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{}", trust.label), style));
        // Show the display string (pattern preview) dimmed below the label
        let display_style = if i == state.selected {
            Style::default()
                .bg(Color::Rgb(50, 50, 70))
                .fg(Color::DarkGray)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::styled(
            format!("    {}", trust.display),
            display_style,
        ));
    }

    let popup = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(Span::styled(
                " Always Allow — Choose Scope ",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    #[test]
    fn approval_renders() {
        let state = ApprovalState {
            tool_call: cyril_core::types::ToolCall::new(
                cyril_core::types::ToolCallId::new("tc_1"),
                "echo hello".into(),
                cyril_core::types::ToolKind::Execute,
                cyril_core::types::ToolCallStatus::Pending,
                None,
            ),
            message: "Allow execution?".into(),
            options: vec![
                cyril_core::types::PermissionOption {
                    id: "allow".into(),
                    label: "Allow Once".into(),
                    kind: cyril_core::types::PermissionOptionKind::AllowOnce,
                    is_destructive: false,
                },
                cyril_core::types::PermissionOption {
                    id: "reject".into(),
                    label: "Reject".into(),
                    kind: cyril_core::types::PermissionOptionKind::RejectOnce,
                    is_destructive: true,
                },
            ],
            trust_options: vec![],
            selected: 0,
            phase: ApprovalPhase::SelectOption,
            responder: tokio::sync::oneshot::channel().0,
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
