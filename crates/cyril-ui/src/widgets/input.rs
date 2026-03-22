use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

use crate::traits::TuiState;

/// Render the input area.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let text = state.input_text();
    let cursor = state.input_cursor();

    // Build display text with cursor indicator
    let (before, after) = if cursor <= text.len() {
        (&text[..cursor], &text[cursor..])
    } else {
        (text, "")
    };

    let line = Line::from(vec![
        Span::raw(before),
        Span::styled("\u{2588}", Style::default().fg(Color::White)),
        Span::raw(after),
    ]);

    let input_widget = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(" > ", Style::default().fg(Color::Cyan))),
    );

    frame.render_widget(input_widget, area);

    // Render autocomplete dropdown if active
    let suggestions = state.autocomplete_suggestions();
    let selected = state.autocomplete_selected();

    if !suggestions.is_empty() {
        render_autocomplete(frame, area, suggestions, selected);
    }
}

fn render_autocomplete(
    frame: &mut Frame,
    input_area: Rect,
    suggestions: &[crate::traits::Suggestion],
    selected: Option<usize>,
) {
    let max_visible = 8.min(suggestions.len());
    let width = suggestions
        .iter()
        .map(|s| s.text.len())
        .max()
        .unwrap_or(10)
        .min(input_area.width as usize - 4)
        .max(10) as u16
        + 4;

    // Position dropdown above the input area
    let height = max_visible as u16 + 2; // +2 for borders
    let y = input_area.y.saturating_sub(height);
    let dropdown_area = Rect::new(input_area.x + 1, y, width, height);

    // Clear background
    frame.render_widget(Clear, dropdown_area);

    let items: Vec<Line> = suggestions
        .iter()
        .enumerate()
        .take(max_visible)
        .map(|(i, s)| {
            let style = if Some(i) == selected {
                Style::default().bg(Color::Rgb(50, 50, 70)).fg(Color::White)
            } else {
                Style::default().fg(Color::Gray)
            };
            Line::styled(&s.text, style)
        })
        .collect();

    let dropdown = Paragraph::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(dropdown, dropdown_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn input_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn input_renders_with_text() {
        let mut state = MockTuiState::default();
        state.input_text = "hello world".into();
        state.input_cursor = 5;

        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn input_renders_with_suggestions() {
        let mut state = MockTuiState::default();
        state.input_text = "/mo".into();
        state.input_cursor = 3;
        state.autocomplete_suggestions = vec![
            crate::traits::Suggestion {
                text: "/model".into(),
                description: Some("Switch model".into()),
            },
            crate::traits::Suggestion {
                text: "/mode".into(),
                description: Some("Switch mode".into()),
            },
        ];
        state.autocomplete_selected = Some(0);

        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                let area = Rect::new(0, 15, 80, 5);
                render(frame, area, &state);
            })
            .expect("draw");
    }
}
