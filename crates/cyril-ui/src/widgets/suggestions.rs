use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::theme::Theme;
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
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState, theme: &Theme) {
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
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    Style::default().fg(theme.muted),
                )
            } else {
                (
                    "  ",
                    Style::default().fg(theme.soft_accent),
                    Style::default().fg(theme.subdued),
                )
            };

            let mut spans = vec![Span::styled(format!("{prefix}{}", s.text), name_style)];
            if let Some(ref desc) = s.description {
                spans.push(Span::styled(format!("  {desc}"), desc_style));
            }

            let mut line = Line::from(spans);
            if is_selected {
                line.style = Style::default().bg(theme.inset_background);
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

    const EXPECTED_SUGGESTION_SHAPE_LABELS: [&str; 13] = [
        "suggestions/empty",
        "cardinality/one",
        "cardinality/ten",
        "cardinality/10000",
        "content/duplicate",
        "content/unicode",
        "content/spaces",
        "content/mixed-descriptions",
        "selection/none",
        "selection/first",
        "selection/middle",
        "selection/last",
        "selection/999",
    ];

    fn matrix_suggestion(index: usize) -> Suggestion {
        let text = match index {
            7 | 8 => "duplicate".into(),
            10 => "選択".into(),
            11 => "with spaces".into(),
            _ => format!("item-{index}"),
        };
        Suggestion {
            text,
            description: index
                .is_multiple_of(2)
                .then(|| format!("description-{index}")),
        }
    }

    fn rendered_suggestion_rows(state: &MockTuiState) -> anyhow::Result<Vec<String>> {
        let mut terminal = Terminal::new(TestBackend::new(80, 10))?;
        terminal.draw(|frame| render(frame, frame.area(), state, &state.theme))?;
        let buffer = terminal.backend().buffer();
        Ok((0..10)
            .map(|y| {
                (0..80)
                    .map(|x| {
                        buffer
                            .cell((x, y))
                            .map_or("", ratatui::buffer::Cell::symbol)
                    })
                    .collect::<String>()
                    .trim_end()
                    .to_string()
            })
            .collect())
    }

    fn expected_window(selected: Option<usize>, total: usize) -> (usize, usize) {
        let visible = total.min(MAX_VISIBLE);
        let start = match selected {
            Some(selected) if total > visible => {
                selected.saturating_sub(visible / 2).min(total - visible)
            }
            _ => 0,
        };
        (start, visible)
    }

    fn suggestion_shape_matrix() -> anyhow::Result<Vec<&'static str>> {
        macro_rules! record {
            ($passes:ident, $label:literal, $condition:expr) => {{
                anyhow::ensure!($condition, "suggestion shape {} failed", $label);
                $passes.push($label);
            }};
        }

        let mut passes = Vec::with_capacity(EXPECTED_SUGGESTION_SHAPE_LABELS.len());
        let empty = MockTuiState::default();
        let empty_rows = rendered_suggestion_rows(&empty)?;
        record!(
            passes,
            "suggestions/empty",
            height_for(&empty) == 0 && empty_rows.iter().all(String::is_empty)
        );

        let one = MockTuiState {
            autocomplete_suggestions: vec![matrix_suggestion(0)],
            autocomplete_selected: Some(0),
            ..Default::default()
        };
        let one_rows = rendered_suggestion_rows(&one)?;
        record!(
            passes,
            "cardinality/one",
            height_for(&one) == 1 && one_rows.iter().filter(|row| !row.is_empty()).count() == 1
        );

        let ten = MockTuiState {
            autocomplete_suggestions: (0..10).map(matrix_suggestion).collect(),
            autocomplete_selected: Some(9),
            ..Default::default()
        };
        let ten_rows = rendered_suggestion_rows(&ten)?;
        record!(
            passes,
            "cardinality/ten",
            height_for(&ten) == 10 && ten_rows.iter().filter(|row| !row.is_empty()).count() == 10
        );

        let mut large = MockTuiState {
            autocomplete_suggestions: (0..10_000).map(matrix_suggestion).collect(),
            autocomplete_selected: None,
            ..Default::default()
        };
        let initial_rows = rendered_suggestion_rows(&large)?;
        record!(
            passes,
            "cardinality/10000",
            height_for(&large) == 10
                && initial_rows.iter().filter(|row| !row.is_empty()).count() == 10
        );

        large.autocomplete_selected = Some(10);
        let content_rows = rendered_suggestion_rows(&large)?;
        record!(
            passes,
            "content/duplicate",
            content_rows
                .iter()
                .filter(|row| row.contains("duplicate"))
                .count()
                == 2
        );
        record!(
            passes,
            "content/unicode",
            content_rows.iter().any(|row| row.contains('選'))
                && content_rows.iter().any(|row| row.contains('択'))
        );
        record!(
            passes,
            "content/spaces",
            content_rows.iter().any(|row| row.contains("with spaces"))
        );
        record!(
            passes,
            "content/mixed-descriptions",
            content_rows
                .iter()
                .any(|row| row.contains("description-10"))
                && content_rows
                    .iter()
                    .any(|row| row.contains("with spaces") && !row.contains("description"))
        );

        for (selected, label) in [
            (None, "selection/none"),
            (Some(0), "selection/first"),
            (Some(5_000), "selection/middle"),
            (Some(9_999), "selection/last"),
            (Some(999), "selection/999"),
        ] {
            large.autocomplete_selected = selected;
            let rows = rendered_suggestion_rows(&large)?;
            let (start, visible) = expected_window(selected, 10_000);
            anyhow::ensure!(visible == 10, "{label} visible count drifted");
            let expected_first = matrix_suggestion(start).text;
            let expected_last = matrix_suggestion(start + visible - 1).text;
            anyhow::ensure!(
                rows[0].contains(&expected_first) && rows[9].contains(&expected_last),
                "{label} window expected {expected_first:?}..{expected_last:?}"
            );
            match selected {
                Some(selected) => {
                    let selected_row = selected - start;
                    anyhow::ensure!(
                        rows[selected_row].starts_with("▸ ")
                            && rows.iter().filter(|row| row.starts_with("▸ ")).count() == 1,
                        "{label} selected row drifted"
                    );
                }
                None => anyhow::ensure!(
                    rows.iter().all(|row| !row.starts_with("▸ ")),
                    "{label} unexpectedly selected a row"
                ),
            }
            passes.push(label);
        }

        Ok(passes)
    }

    #[test]
    fn every_autocomplete_shape_is_fenced() -> anyhow::Result<()> {
        let passes = suggestion_shape_matrix()?;
        assert_eq!(passes, EXPECTED_SUGGESTION_SHAPE_LABELS);
        Ok(())
    }

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
    fn selected_and_unselected_rows_use_marker_theme_roles() {
        let state = MockTuiState {
            autocomplete_suggestions: vec![
                Suggestion {
                    text: "plain".into(),
                    description: Some("description".into()),
                },
                Suggestion {
                    text: "selected".into(),
                    description: Some("detail".into()),
                },
            ],
            autocomplete_selected: Some(1),
            ..Default::default()
        };
        let backend = TestBackend::new(40, 2);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");
        let buffer = terminal.backend().buffer();

        assert_eq!(
            buffer.cell((2, 0)).map(|cell| cell.fg),
            Some(state.theme.soft_accent)
        );
        assert_eq!(
            buffer.cell((9, 0)).map(|cell| cell.fg),
            Some(state.theme.subdued)
        );
        assert_eq!(
            buffer.cell((2, 1)).map(|cell| cell.fg),
            Some(state.theme.text)
        );
        assert_eq!(
            buffer.cell((12, 1)).map(|cell| cell.fg),
            Some(state.theme.muted)
        );
        assert_eq!(
            buffer.cell((2, 1)).map(|cell| cell.bg),
            Some(state.theme.inset_background)
        );
    }

    fn baseline_suggestions() -> Vec<Suggestion> {
        (0..21)
            .map(|index| {
                let text = match index {
                    7 | 8 => "duplicate".to_string(),
                    10 => "選択".to_string(),
                    11 => "with spaces".to_string(),
                    _ => format!("item-{index}"),
                };
                Suggestion {
                    text,
                    description: (index % 2 == 0).then(|| format!("description-{index}")),
                }
            })
            .collect()
    }

    #[test]
    fn suggestion_shape_matches_pinned_baseline() -> anyhow::Result<()> {
        let state = MockTuiState {
            autocomplete_suggestions: baseline_suggestions(),
            autocomplete_selected: Some(10),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render(frame, frame.area(), &state, &state.theme))?;

        let expected = include_str!("../../tests/fixtures/conversation-theme-baseline.tsv")
            .lines()
            .skip(2)
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                let y = fields.get(2)?.parse::<u16>().ok()?;
                (fields.first() == Some(&"input") && (5..15).contains(&y)).then_some(fields)
            })
            .map(|fields| {
                Ok((
                    fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing suggestion symbol"))?
                        .to_string(),
                    fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing suggestion modifier"))?
                        .parse::<u16>()?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let actual = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| {
                let mut symbol = String::with_capacity(cell.symbol().len() * 2);
                for byte in cell.symbol().as_bytes() {
                    symbol.push(HEX[(byte >> 4) as usize] as char);
                    symbol.push(HEX[(byte & 0x0f) as usize] as char);
                }
                (symbol, cell.modifier.bits())
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), 800);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn center_window_handles_boundary_and_out_of_range_selections() {
        for (selected, first, last, marker) in [
            (None, "item-0", "item-9", None),
            (Some(0), "item-0", "item-9", Some("item-0")),
            (Some(10), "item-5", "item-14", Some("選択")),
            (Some(20), "with spaces", "item-20", Some("item-20")),
            (Some(999), "with spaces", "item-20", None),
        ] {
            let state = MockTuiState {
                autocomplete_suggestions: baseline_suggestions(),
                autocomplete_selected: selected,
                ..Default::default()
            };
            let backend = TestBackend::new(80, 10);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| render(frame, frame.area(), &state, &state.theme))
                .expect("draw");
            let buffer = terminal.backend().buffer();
            let rows = (0..10)
                .map(|y| {
                    (0..80)
                        .filter_map(|x| buffer.cell((x, y)))
                        .map(|cell| cell.symbol())
                        .collect::<String>()
                        .trim()
                        .to_string()
                })
                .collect::<Vec<_>>();

            assert_eq!(rows.len(), 10);
            assert!(
                rows[0].contains(first),
                "wrong first row for {selected:?}: {rows:?}"
            );
            assert!(
                rows[9].contains(last),
                "wrong last row for {selected:?}: {rows:?}"
            );
            let selected_rows = rows.iter().filter(|row| row.starts_with('▸')).count();
            assert_eq!(selected_rows, usize::from(marker.is_some()));
            if let Some(label) = marker {
                let selected_row = rows
                    .iter()
                    .find(|row| row.starts_with('▸'))
                    .expect("selected row");
                if label.is_ascii() {
                    assert!(selected_row.contains(label));
                } else {
                    assert!(
                        label
                            .chars()
                            .all(|character| selected_row.contains(character))
                    );
                }
            }
        }
    }

    #[test]
    fn ten_thousand_suggestions_still_render_only_ten_rows() {
        let state = MockTuiState {
            autocomplete_suggestions: (0..10_000)
                .map(|index| Suggestion {
                    text: format!("item-{index}"),
                    description: None,
                })
                .collect(),
            autocomplete_selected: Some(5_000),
            ..Default::default()
        };
        assert_eq!(height_for(&state), 10);
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");
        let text = buffer_text(&terminal, 10);
        assert!(text.contains("item-4995"));
        assert!(text.contains("▸ item-5000"));
        assert!(text.contains("item-5004"));
        assert!(!text.contains("item-4994"));
        assert!(!text.contains("item-5005"));
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, Rect::new(0, 0, 80, 0), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
