use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

use crate::traits::TuiState;

/// Draw the full TUI frame. Panic-safe wrapper with fallback rendering.
pub fn draw(frame: &mut Frame, state: &dyn TuiState) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_inner(frame, state);
    }));
    if result.is_err() {
        draw_fallback(frame);
    }
}

fn draw_inner(frame: &mut Frame, _state: &dyn TuiState) {
    let area = frame.area();
    // Placeholder layout — widgets will be added in Phase 7
    let [_toolbar_area, chat_area, _input_area, _status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .areas(area);

    let placeholder = Paragraph::new("cyril v2 — widgets coming soon");
    frame.render_widget(placeholder, chat_area);
}

fn draw_fallback(frame: &mut Frame) {
    let text = Paragraph::new("Render error — press Ctrl+C to quit");
    frame.render_widget(text, frame.area());
}

#[cfg(test)]
mod tests {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn draw_fallback_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                super::draw_fallback(frame);
            })
            .expect("draw should succeed");
    }

    #[test]
    fn draw_with_mock_state_does_not_panic() {
        use crate::traits::test_support::MockTuiState;

        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                super::draw(frame, &state);
            })
            .expect("draw should succeed");
    }
}
