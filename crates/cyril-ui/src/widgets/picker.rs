use ratatui::prelude::*;
use ratatui::widgets::{
    Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::traits::PickerState;
use crate::widgets::modal;

/// Maximum option rows the popup shows at once, regardless of terminal size.
const MAX_VISIBLE_OPTIONS: usize = 15;

/// The option window: which contiguous slice of `filtered_indices` is drawn.
///
/// Selection-centered and clamped (`start = clamp(selected - rows/2, 0,
/// n - rows)`), so the selected row is always inside the window
/// (cyril-cc5e claims C1/C2; formula model-checked over 57k cases in
/// `.cyril-cc5e/window-model-check.py`). An out-of-range `selected` yields
/// a valid window with no marked row — the state machine maintains
/// `selected < filtered_indices.len()`, so this is a sanity fallback, not
/// a contract callers may lean on.
fn option_window(n: usize, selected: usize, option_rows: usize) -> (usize, usize) {
    let rows = n.min(option_rows);
    if rows == 0 {
        return (0, 0);
    }
    let start = selected.saturating_sub(rows / 2).min(n - rows);
    (start, rows)
}

/// Render the picker overlay (centered popup).
pub fn render(frame: &mut Frame, area: Rect, state: &PickerState) {
    let n = state.filtered_indices.len();
    let desired_rows = n.min(MAX_VISIBLE_OPTIONS);
    // Reserved whenever ANY option has a description (not just the selected
    // one) so popup height stays constant while navigating.
    let desc_reserve = usize::from(state.options.iter().any(|o| o.description.is_some()));
    // 4 = top/bottom border + filter line + blank line.
    let desired_height = u16::try_from(desired_rows + desc_reserve + 4).unwrap_or(u16::MAX);
    let popup_area = modal::centered(area, 80, desired_height);

    let inner_height = popup_area.height.saturating_sub(2) as usize;
    let option_rows = inner_height.saturating_sub(2 + desc_reserve);
    let (start, rows) = option_window(n, state.selected, option_rows);

    frame.render_widget(Clear, popup_area);

    let mut lines: Vec<Line> = Vec::new();

    // Filter input
    lines.push(Line::from(vec![
        Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
        Span::styled(&state.filter, Style::default().fg(Color::White)),
        Span::styled("█", Style::default().fg(Color::White)),
    ]));
    lines.push(Line::default());

    // Options within the selection-centered window
    for (offset, &option_idx) in state.filtered_indices[start..start + rows]
        .iter()
        .enumerate()
    {
        let display_idx = start + offset;
        if let Some(opt) = state.options.get(option_idx) {
            let is_selected = display_idx == state.selected;
            let prefix = if is_selected { "▸ " } else { "  " };
            let current_marker = if opt.is_current { " ✓" } else { "" };

            let label_style = if is_selected {
                Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            let detail_style = if is_selected {
                Style::default()
                    .bg(Color::Rgb(50, 50, 70))
                    .fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            let mut spans = vec![Span::styled(
                format!("{prefix}{}{current_marker}", opt.label),
                label_style,
            )];

            // Show group (e.g., credit tier) if available
            if let Some(ref group) = opt.group {
                spans.push(Span::styled(format!("  {group}"), detail_style));
            }

            lines.push(Line::from(spans));

            // Show description on a second line for the selected item
            if is_selected && let Some(ref desc) = opt.description {
                lines.push(Line::styled(
                    format!("    {desc}"),
                    Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC),
                ));
            }
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

    // Display-only overflow indicator (cyril-cc5e C4): key handling is
    // untouched — the scrollbar mirrors the option window, nothing more.
    if n > rows {
        let mut scrollbar_state = ScrollbarState::new(n).position(start);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, popup_area, &mut scrollbar_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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
