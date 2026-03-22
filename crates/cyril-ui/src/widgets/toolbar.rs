use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::traits::{Activity, TuiState};

const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// Render the toolbar (top line).
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut parts: Vec<Span> = Vec::new();

    // Activity indicator
    match state.activity() {
        Activity::Idle | Activity::Ready => {}
        Activity::Sending | Activity::Waiting => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", SPINNER_CHARS[idx]),
                Style::default().fg(Color::Yellow),
            ));
        }
        Activity::Streaming => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", SPINNER_CHARS[idx]),
                Style::default().fg(Color::Green),
            ));
        }
        Activity::ToolRunning => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", SPINNER_CHARS[idx]),
                Style::default().fg(Color::Cyan),
            ));
        }
    }

    // Session label
    if let Some(label) = state.session_label() {
        parts.push(Span::styled(
            label.to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        parts.push(Span::styled(
            "No session",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Mode
    if let Some(mode) = state.current_mode() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            mode.to_string(),
            Style::default().fg(Color::Cyan),
        ));
    }

    // Model
    if let Some(model) = state.current_model() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            model.to_string(),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Elapsed time for active operations
    if let Some(elapsed) = state.activity_elapsed() {
        let secs = elapsed.as_secs();
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            format!("{secs}s"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(parts);
    let toolbar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 46)));

    frame.render_widget(toolbar, area);
}

/// Render the bottom status bar (context usage + credits).
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut parts: Vec<Span> = Vec::new();

    // Context usage gauge
    if let Some(pct) = state.context_usage() {
        let color = if pct > 90.0 {
            Color::Red
        } else if pct > 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        parts.push(Span::styled(
            format!("Context: {pct:.0}%"),
            Style::default().fg(color),
        ));
    }

    // Credit usage
    if let Some((used, limit)) = state.credit_usage() {
        if !parts.is_empty() {
            parts.push(Span::raw(" · "));
        }
        parts.push(Span::styled(
            format!("Credits: ${used:.2}/${limit:.2}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    if parts.is_empty() {
        parts.push(Span::styled("cyril", Style::default().fg(Color::DarkGray)));
    }

    let line = Line::from(parts);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 46)));

    frame.render_widget(bar, area);
}

fn spinner_index(state: &dyn TuiState) -> usize {
    state
        .activity_elapsed()
        .map(|d| (d.as_millis() / 80) as usize % SPINNER_CHARS.len())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn toolbar_renders_no_session() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
        // If we get here, rendering succeeded
    }

    #[test]
    fn toolbar_renders_with_session() {
        let mut state = MockTuiState::default();
        state.session_label = Some("my-session".into());
        state.current_mode = Some("code".into());
        state.current_model = Some("claude-sonnet-4".into());

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_renders_context_usage() {
        let mut state = MockTuiState::default();
        state.context_usage = Some(75.0);
        state.credit_usage = Some((5.25, 10.0));

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
    }
}
