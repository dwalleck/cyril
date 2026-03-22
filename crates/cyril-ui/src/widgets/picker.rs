use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::traits::PickerState;

/// Render the picker overlay (centered popup).
pub fn render(frame: &mut Frame, area: Rect, state: &PickerState) {
    let width = 60.min(area.width.saturating_sub(4));
    let visible = state.filtered_indices.len().min(15);
    let height = (visible as u16 + 5).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();

    // Filter input
    lines.push(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&state.filter, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::White)),
    ]));
    lines.push(Line::default());

    // Options
    for (display_idx, &option_idx) in state.filtered_indices.iter().enumerate().take(visible) {
        if let Some(opt) = state.options.get(option_idx) {
            let style = if display_idx == state.selected {
                Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };

            let prefix = if display_idx == state.selected {
                "▸ "
            } else {
                "  "
            };
            let current_marker = if opt.is_current { " ✓" } else { "" };

            lines.push(Line::styled(
                format!("{prefix}{}{current_marker}", opt.label),
                style,
            ));
        }
    }

    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                format!(" {} ", state.title),
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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn picker_renders() {
        let state = PickerState {
            title: "Select Model".into(),
            options: vec![
                cyril_core::types::CommandOption {
                    label: "Claude Sonnet".into(),
                    value: "claude-sonnet-4".into(),
                    description: None,
                    group: None,
                    is_current: true,
                },
                cyril_core::types::CommandOption {
                    label: "Claude Haiku".into(),
                    value: "claude-haiku-4.5".into(),
                    description: None,
                    group: None,
                    is_current: false,
                },
            ],
            filter: String::new(),
            filtered_indices: vec![0, 1],
            selected: 0,
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
