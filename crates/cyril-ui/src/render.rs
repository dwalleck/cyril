use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

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

fn draw_inner(frame: &mut Frame, state: &dyn TuiState) {
    let area = frame.area();

    // Crew panel sizing is owned by the crew_panel widget (single source of truth).
    let crew_height = crate::widgets::crew_panel::height_for(state);

    let [toolbar_area, chat_area, crew_area, input_area, status_area] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(crew_height),
        Constraint::Length(5),
        Constraint::Length(1),
    ])
    .areas(area);

    crate::widgets::toolbar::render(frame, toolbar_area, state);
    crate::widgets::chat::render(frame, chat_area, state);
    if crew_height > 0 {
        crate::widgets::crew_panel::render(frame, crew_area, state);
    }
    crate::widgets::input::render(frame, input_area, state);
    crate::widgets::toolbar::render_status_bar(frame, status_area, state);

    // Overlays (rendered on top)
    if let Some(approval) = state.approval() {
        crate::widgets::approval::render(frame, area, approval);
    }
    if let Some(picker) = state.picker() {
        crate::widgets::picker::render(frame, area, picker);
    }
    if let Some(hooks) = state.hooks_panel() {
        crate::widgets::hooks_panel::render(frame, area, hooks);
    }
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
