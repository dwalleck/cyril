use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::palette;
use crate::traits::TuiState;

const MAX_VISIBLE: usize = 10;

/// Compute the height needed for the suggestions panel.
/// Returns 0 when no suggestions are present, or the count capped at
/// `MAX_VISIBLE` (10) otherwise. Used directly as a layout `Constraint::Length`.
pub fn height_for(state: &dyn TuiState) -> u16 {
    let count = state.autocomplete_suggestions().len();
    if count == 0 {
        return 0;
    }
    count.min(MAX_VISIBLE) as u16
}

/// Render the inline suggestions panel.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let suggestions = state.autocomplete_suggestions();
    let selected = state.autocomplete_selected();
    if suggestions.is_empty() {
        return;
    }

    let total = suggestions.len();
    let visible = total.min(MAX_VISIBLE);

    // Center-scroll: keep the selected item near the middle of the viewport.
    // Clamped so the window never starts before 0 or extends past the end.
    // When nothing is selected or the list fits in one page, start at 0.
    let start = match selected {
        Some(sel) if total > visible => {
            let half = visible / 2;
            sel.saturating_sub(half).min(total - visible)
        }
        _ => 0,
    };

    let lines: Vec<Line> = suggestions[start..start + visible]
        .iter()
        .enumerate()
        .map(|(offset, s)| {
            let is_selected = Some(start + offset) == selected;

            let (prefix, name_style, desc_style) = if is_selected {
                (
                    "▸ ",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                    Style::default().fg(palette::MUTED_GRAY),
                )
            } else {
                (
                    "  ",
                    Style::default().fg(palette::USER_BLUE),
                    Style::default().fg(Color::DarkGray),
                )
            };

            let mut spans = vec![Span::styled(format!("{prefix}{}", s.text), name_style)];
            if let Some(ref desc) = s.description {
                spans.push(Span::styled(format!("  {desc}"), desc_style));
            }

            let mut line = Line::from(spans);
            if is_selected {
                line.style = Style::default().bg(palette::CODE_BLOCK_BG);
            }
            line
        })
        .collect();

    frame.render_widget(Paragraph::new(lines), area);
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::traits::Suggestion;
    use crate::traits::test_support::MockTuiState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn buffer_text(terminal: &Terminal<TestBackend>, rows: u16) -> String {
        let buffer = terminal.backend().buffer();
        (0..rows)
            .flat_map(|y| {
                (0..buffer.area.width).map(move |x| {
                    buffer
                        .cell((x, y))
                        .expect("cell coordinates within TestBackend bounds")
                        .symbol()
                        .to_string()
                })
            })
            .collect()
    }

    #[test]
    fn height_for_returns_zero_when_no_suggestions() {
        let state = MockTuiState::default();
        assert_eq!(height_for(&state), 0);
    }

    #[test]
    fn height_for_caps_at_max_visible() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        assert_eq!(height_for(&state), MAX_VISIBLE as u16);
    }

    #[test]
    fn height_for_matches_count_when_fewer_than_max() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion {
                    text: "/a".into(),
                    description: None,
                },
                Suggestion {
                    text: "/b".into(),
                    description: None,
                },
            ],
            ..Default::default()
        };
        assert_eq!(height_for(&state), 2);
    }

    #[test]
    fn render_shows_selected_item() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion {
                    text: "/model".into(),
                    description: Some("Switch model".into()),
                },
                Suggestion {
                    text: "/mode".into(),
                    description: Some("Switch mode".into()),
                },
                Suggestion {
                    text: "/new".into(),
                    description: None,
                },
            ],
            autocomplete_selected: Some(1),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 3);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, 3);
        assert!(text.contains("/model"), "should show /model");
        assert!(
            text.contains("▸ /mode"),
            "selection indicator should be on the selected /mode row"
        );
    }

    #[test]
    fn render_shows_descriptions() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![Suggestion {
                text: "/model".into(),
                description: Some("Switch model".into()),
            }],
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, 1);
        assert!(text.contains("Switch model"), "should show description");
    }

    #[test]
    fn render_scrolls_to_selected_middle() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(15),
            ..Default::default()
        };
        let backend = TestBackend::new(80, MAX_VISIBLE as u16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, MAX_VISIBLE as u16);
        assert!(
            text.contains("▸ /cmd15"),
            "should show selected item /cmd15 when scrolled"
        );
        // Center-scroll: sel=15, half=5, start=10, window [10..20]
        assert!(
            text.contains("/cmd10"),
            "window should start at /cmd10 for centered selection 15"
        );
        assert!(
            !text.contains("/cmd9 "),
            "items before the window should not be visible"
        );
    }

    #[test]
    fn render_scrolls_to_last_item() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(19),
            ..Default::default()
        };
        let backend = TestBackend::new(80, MAX_VISIBLE as u16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, MAX_VISIBLE as u16);
        assert!(text.contains("/cmd19"), "should show last item /cmd19");
    }

    #[test]
    fn render_no_panic_with_empty_suggestions() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 0);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, Rect::new(0, 0, 80, 0), &state);
            })
            .expect("draw should not panic with no suggestions");
    }

    #[test]
    fn file_suggestions_render_without_description() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion {
                    text: "@src/main.rs".into(),
                    description: None,
                },
                Suggestion {
                    text: "@src/lib.rs".into(),
                    description: None,
                },
            ],
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 2);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, 2);
        assert!(text.contains("@src/main.rs"), "should show file suggestion");
    }

    #[test]
    fn render_with_none_selected_shows_no_indicator() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion {
                    text: "/model".into(),
                    description: Some("Switch model".into()),
                },
                Suggestion {
                    text: "/mode".into(),
                    description: None,
                },
            ],
            autocomplete_selected: None,
            ..Default::default()
        };
        let backend = TestBackend::new(80, 2);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, 2);
        assert!(text.contains("/model"), "should show first item");
        assert!(text.contains("/mode"), "should show second item");
        assert!(
            !text.contains("▸"),
            "no selection indicator when selected is None"
        );
    }

    #[test]
    fn render_scrolls_at_exact_boundary() {
        // selected = MAX_VISIBLE (10): center-scroll puts it mid-viewport.
        // half=5, start=10-5=5, window [5..15], /cmd10 centered at row 5.
        let state = MockTuiState {
            autocomplete_suggestions: (0..20)
                .map(|i| Suggestion {
                    text: format!("/cmd{i}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(MAX_VISIBLE),
            ..Default::default()
        };
        let backend = TestBackend::new(80, MAX_VISIBLE as u16);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");

        let text = buffer_text(&terminal, MAX_VISIBLE as u16);
        assert!(
            text.contains("▸ /cmd10"),
            "selected /cmd10 should be visible and centered"
        );
        assert!(text.contains("/cmd5 "), "window should start at /cmd5");
        assert!(!text.contains("/cmd4 "), "/cmd4 should have scrolled out");
    }
}
