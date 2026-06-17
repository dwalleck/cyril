use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::traits::TuiState;

/// Minimum input height (3 content rows + 2 borders) — preserves the prior look
/// for single-line input.
const MIN_HEIGHT: u16 = 5;
/// Maximum input height (10 content rows + 2 borders) so a large paste can't
/// crowd out the chat area; content beyond this scrolls within the box.
const MAX_HEIGHT: u16 = 12;

/// Height (including borders) the input box needs for its current content.
///
/// Grows with the number of newline-separated lines so pasted multi-line text is
/// visible, clamped to `[MIN_HEIGHT, MAX_HEIGHT]`. Width-wrapping of long single
/// lines is handled within the available rows by the `Wrap` in `render`.
pub fn height_for(state: &dyn TuiState) -> u16 {
    let lines = state
        .input_text()
        .split('\n')
        .count()
        .min(MAX_HEIGHT as usize) as u16;
    (lines + 2).clamp(MIN_HEIGHT, MAX_HEIGHT)
}

/// Render the input area, displaying newlines as real rows and placing the
/// cursor block at the byte cursor.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let text = state.input_text();
    let cursor = state.input_cursor().min(text.len());
    let (before, after) = text.split_at(cursor);

    // `split('\n')` yields at least one segment for any string (including ""),
    // so the last()/[0] accesses below never panic.
    let before_segments: Vec<&str> = before.split('\n').collect();
    let after_segments: Vec<&str> = after.split('\n').collect();

    let mut lines: Vec<Line> = Vec::new();

    // Whole lines entirely above the cursor's row.
    for seg in &before_segments[..before_segments.len() - 1] {
        lines.push(Line::from(Span::raw(*seg)));
    }

    // The cursor's row: text before the cursor on this line, the cursor block,
    // then text after the cursor up to the next newline.
    let before_tail = before_segments[before_segments.len() - 1];
    let after_head = after_segments[0];
    lines.push(Line::from(vec![
        Span::raw(before_tail),
        Span::styled("\u{2588}", Style::default().fg(Color::White)),
        Span::raw(after_head),
    ]));

    // Whole lines entirely below the cursor's row.
    for seg in &after_segments[1..] {
        lines.push(Line::from(Span::raw(*seg)));
    }

    let input_widget = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

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

    /// Flatten a `TestBackend` buffer into one trimmed string per row.
    fn buffer_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buffer = terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    .trim()
                    .to_string()
            })
            .collect()
    }

    #[test]
    fn height_for_grows_with_lines_and_clamps() {
        let single = MockTuiState {
            input_text: "one line".into(),
            ..Default::default()
        };
        assert_eq!(height_for(&single), MIN_HEIGHT);

        let four = MockTuiState {
            input_text: "a\nb\nc\nd".into(),
            ..Default::default()
        };
        assert_eq!(height_for(&four), 6); // 4 lines + 2 borders

        let huge = MockTuiState {
            input_text: "x\n".repeat(50),
            ..Default::default()
        };
        assert_eq!(height_for(&huge), MAX_HEIGHT);
    }

    #[test]
    fn multiline_input_renders_each_line_on_its_own_row() {
        let state = MockTuiState {
            input_text: "line1\nline2\nline3".into(),
            input_cursor: 0,
            ..Default::default()
        };

        let backend = TestBackend::new(40, 8);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state))
            .expect("draw");

        let rows = buffer_rows(&terminal);
        // Each pasted line must land on a distinct row (newlines are real row
        // breaks now, not swallowed into one concatenated line).
        let row_of = |needle: &str| rows.iter().position(|r| r.contains(needle));
        let (r1, r2, r3) = (row_of("line1"), row_of("line2"), row_of("line3"));
        assert!(
            r1.is_some() && r2.is_some() && r3.is_some(),
            "all three lines must render: {rows:?}"
        );
        assert!(
            r1 < r2 && r2 < r3,
            "lines must be on increasing rows: {rows:?}"
        );
    }
}
