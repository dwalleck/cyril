use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

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
        let state = MockTuiState {
            input_text: "hello world".into(),
            input_cursor: 5,
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

    #[test]
    fn input_renders_with_suggestions() {
        // Suggestions are now rendered by the suggestions widget, not input.
        // This test verifies the input box renders fine when suggestions
        // are active in state.
        let state = MockTuiState {
            input_text: "/mo".into(),
            input_cursor: 3,
            autocomplete_suggestions: vec![crate::traits::Suggestion {
                text: "/model".into(),
                description: Some("Switch model".into()),
            }],
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
}
