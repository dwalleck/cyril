use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::theme::Theme;
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

/// Char-wrapped visual layout of the input text (cyril-a14l C2/C3).
///
/// Inserts the cursor block at `cursor` (a byte offset; clamped to the text
/// length and floored to a char boundary — flooring is defined behavior,
/// not an error path, so out-of-range cursors from a stale state still
/// render), splits on `'\n'`, and char-wraps each logical line at `width`
/// cells (`width` is clamped to ≥ 1). East-Asian wide chars take 2 cells
/// and move whole to the next row rather than straddling the boundary;
/// combining marks take 0 cells and never force a wrap. Returns the visual
/// rows plus the index of the row carrying the cursor block — tracked
/// structurally during the wrap, so text that itself contains `█` cannot
/// confuse it. Independently oracled by `.cyril-a14l/oracle-input-wrap.py`
/// via `tests/fixtures/input-wrap-oracle.tsv`.
pub fn wrapped_rows(text: &str, cursor: usize, width: usize) -> (Vec<String>, usize) {
    use unicode_width::UnicodeWidthChar;

    let width = width.max(1);
    let mut cursor = cursor.min(text.len());
    while cursor > 0 && !text.is_char_boundary(cursor) {
        cursor -= 1;
    }
    let block_index = text[..cursor].chars().count();
    let decorated = format!("{}\u{2588}{}", &text[..cursor], &text[cursor..]);

    let mut rows: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut used = 0usize;
    let mut cursor_row = 0usize;
    for (index, character) in decorated.chars().enumerate() {
        if character == '\n' {
            rows.push(std::mem::take(&mut current));
            used = 0;
            continue;
        }
        let cells = character.width().unwrap_or(0);
        if cells > 0 && used + cells > width && !current.is_empty() {
            rows.push(std::mem::take(&mut current));
            used = 0;
        }
        if index == block_index {
            cursor_row = rows.len();
        }
        current.push(character);
        used += cells;
    }
    rows.push(current);
    (rows, cursor_row)
}

/// Render the input area, displaying newlines as real rows and placing the
/// cursor block at the byte cursor.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState, theme: &Theme) {
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
        Span::styled("\u{2588}", Style::default().fg(theme.text)),
        Span::raw(after_head),
    ]));

    // Whole lines entirely below the cursor's row.
    for seg in &after_segments[1..] {
        lines.push(Line::from(Span::raw(*seg)));
    }

    let input_widget = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.subdued))
            .title(Span::styled(
                " > ",
                Style::default().fg(theme.accent_quinary),
            )),
    );

    frame.render_widget(input_widget, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const EXPECTED_INPUT_SHAPE_LABELS: [&str; 9] = [
        "input/empty",
        "input/spaces",
        "input/multiline",
        "input/unicode",
        "cursor/start",
        "cursor/middle",
        "cursor/end",
        "cursor/beyond",
        "stress/100-kib-unicode-multiline",
    ];

    fn render_input_buffer(
        text: &str,
        cursor: usize,
        width: u16,
        height: u16,
    ) -> anyhow::Result<ratatui::buffer::Buffer> {
        let state = MockTuiState {
            input_text: text.into(),
            input_cursor: cursor,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| render(frame, frame.area(), &state, &state.theme))?;
        Ok(terminal.backend().buffer().clone())
    }

    fn expected_row_symbols(text: &str, cursor: usize) -> Vec<Vec<String>> {
        use unicode_width::UnicodeWidthChar;

        let cursor = cursor.min(text.len());
        let (before, after) = text.split_at(cursor);
        let decorated = format!("{before}█{after}");
        decorated
            .split('\n')
            .map(|line| {
                line.chars()
                    .flat_map(|character| {
                        let mut cells = vec![character.to_string()];
                        cells.extend(std::iter::repeat_n(
                            " ".to_string(),
                            character.width().unwrap_or(0).saturating_sub(1),
                        ));
                        cells
                    })
                    .collect()
            })
            .collect()
    }

    fn small_input_matches_oracle(text: &str, cursor: usize) -> anyhow::Result<bool> {
        let buffer = render_input_buffer(text, cursor, 40, 8)?;
        let expected = expected_row_symbols(text, cursor);
        for (row, symbols) in expected.iter().enumerate() {
            for x in 0..38usize {
                let expected_symbol = symbols.get(x).map_or(" ", String::as_str);
                let actual = buffer
                    .cell(((x + 1) as u16, (row + 1) as u16))
                    .ok_or_else(|| anyhow::anyhow!("missing input cell ({x},{row})"))?;
                if actual.symbol() != expected_symbol {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn input_shape_matrix() -> anyhow::Result<Vec<&'static str>> {
        let mut passes = Vec::with_capacity(EXPECTED_INPUT_SHAPE_LABELS.len());
        for (text, cursor, label) in [
            ("", 0, "input/empty"),
            ("   ", 1, "input/spaces"),
            ("one\ntwo", "one\n".len(), "input/multiline"),
            ("A世界B", "A世".len(), "input/unicode"),
            ("abc", 0, "cursor/start"),
            ("abc", 1, "cursor/middle"),
            ("abc", 3, "cursor/end"),
            ("abc", usize::MAX, "cursor/beyond"),
        ] {
            anyhow::ensure!(
                small_input_matches_oracle(text, cursor)?,
                "shape {label} failed"
            );
            passes.push(label);
        }

        let mut large = "世界\n".repeat((100 * 1024) / "世界\n".len());
        large.push_str("世a");
        anyhow::ensure!(large.len() == 100 * 1024, "100 KiB fixture size drifted");
        let mut middle = large.len() / 2;
        while !large.is_char_boundary(middle) {
            middle -= 1;
        }
        let start = render_input_buffer(&large, 0, 80, 10)?;
        let _middle = render_input_buffer(&large, middle, 80, 10)?;
        let end = render_input_buffer(&large, large.len(), 80, 10)?;
        let beyond = render_input_buffer(&large, usize::MAX, 80, 10)?;
        anyhow::ensure!(
            start
                .content()
                .iter()
                .filter(|cell| cell.symbol() == "█")
                .count()
                == 1,
            "100 KiB start cursor was not visible"
        );
        anyhow::ensure!(end == beyond, "cursor beyond length did not clamp to end");
        anyhow::ensure!(
            large.lines().count() > 10_000,
            "100 KiB multiline fixture lost rows"
        );
        passes.push("stress/100-kib-unicode-multiline");
        Ok(passes)
    }

    #[test]
    fn every_message_input_shape_is_fenced() -> anyhow::Result<()> {
        let passes = input_shape_matrix()?;
        assert_eq!(passes, EXPECTED_INPUT_SHAPE_LABELS);
        Ok(())
    }

    #[test]
    fn input_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
                render(frame, frame.area(), &state, &state.theme);
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
    fn input_chrome_uses_marker_theme_roles() {
        let state = MockTuiState {
            input_text: "marker".into(),
            input_cursor: 3,
            ..Default::default()
        };
        let backend = TestBackend::new(40, 5);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");
        let cells = terminal.backend().buffer().content();
        let color_of = |symbol: &str| {
            cells
                .iter()
                .find(|cell| cell.symbol() == symbol)
                .map(|cell| cell.fg)
        };

        assert_eq!(color_of("█"), Some(state.theme.text));
        assert_eq!(color_of(">"), Some(state.theme.accent_quinary));
        assert_eq!(color_of("┌"), Some(state.theme.subdued));
    }

    #[test]
    fn input_shape_matches_pinned_baseline() -> anyhow::Result<()> {
        let state = MockTuiState {
            input_text: "first\nUnicode 世界\nthird".into(),
            input_cursor: "first\nUnicode ".len(),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 5);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| render(frame, frame.area(), &state, &state.theme))?;

        let expected = include_str!("../../tests/fixtures/conversation-theme-baseline.tsv")
            .lines()
            .skip(2)
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                let y = fields.get(2)?.parse::<u16>().ok()?;
                (fields.first() == Some(&"input") && y < 5).then_some(fields)
            })
            .map(|fields| {
                Ok((
                    fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing input symbol"))?
                        .to_string(),
                    fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing input modifier"))?
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

        assert_eq!(actual.len(), 400);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn cursor_boundaries_preserve_ascii_unicode_and_multiline_rows() {
        for (text, cursor) in [
            ("", 0),
            ("ascii", 0),
            ("ascii", 2),
            ("ascii", 5),
            ("ascii", usize::MAX),
            ("世界", 0),
            ("世界", "世".len()),
            ("世界", "世界".len()),
            ("first\n世界\nthird", "first\n世".len()),
        ] {
            let state = MockTuiState {
                input_text: text.into(),
                input_cursor: cursor,
                ..Default::default()
            };
            let backend = TestBackend::new(40, 8);
            let mut terminal = Terminal::new(backend).expect("test terminal");
            terminal
                .draw(|frame| render(frame, frame.area(), &state, &state.theme))
                .expect("draw");
            let buffer = terminal.backend().buffer();
            assert_eq!(
                buffer
                    .content()
                    .iter()
                    .filter(|cell| cell.symbol() == "█")
                    .count(),
                1
            );
            for character in text.chars().filter(|character| *character != '\n') {
                assert!(
                    buffer
                        .content()
                        .iter()
                        .any(|cell| cell.symbol() == character.to_string()),
                    "missing {character:?} for cursor {cursor}"
                );
            }
        }
    }

    fn oracle_case_text(case: &str) -> anyhow::Result<String> {
        Ok(match case {
            "empty" => String::new(),
            "ascii" => "ascii".into(),
            "long-line" => "ab ".repeat(100),
            "draft-10" => (1..=10)
                .map(|index| format!("draft-{index}"))
                .collect::<Vec<_>>()
                .join("\n"),
            "wide" => "世界".repeat(40),
            "wide-straddle" => "ab世cd".into(),
            "combining" => "a\u{0301}bc".into(),
            other => anyhow::bail!("unknown oracle case {other}"),
        })
    }

    /// cyril-a14l C2/C3 (slice 5): every (case, width, cursor) row of the
    /// committed Python-oracle fixture matches `wrapped_rows` exactly —
    /// rows AND cursor row. The fixture was generated before this function
    /// was written (`.cyril-a14l/oracle-input-wrap.py`).
    #[test]
    fn wrap_rows_match_python_oracle() -> anyhow::Result<()> {
        let fixture = include_str!("../../tests/fixtures/input-wrap-oracle.tsv");
        let mut checked = 0usize;
        for line in fixture.lines().skip(1) {
            let fields: Vec<&str> = line.split('\t').collect();
            anyhow::ensure!(fields.len() == 5, "malformed fixture line: {line:?}");
            let text = oracle_case_text(fields[0])?;
            let width: usize = fields[1].parse()?;
            let cursor: usize = fields[2].parse()?;
            let expected_cursor_row: usize = fields[3].parse()?;
            let expected_rows: Vec<&str> = fields[4].split('\u{1f}').collect();

            let (rows, cursor_row) = wrapped_rows(&text, cursor, width);
            anyhow::ensure!(
                rows == expected_rows && cursor_row == expected_cursor_row,
                "case {}/{width}/{cursor}: got ({rows:?}, {cursor_row}), oracle says \
                 ({expected_rows:?}, {expected_cursor_row})",
                fields[0]
            );
            checked += 1;
        }
        assert_eq!(checked, 95, "oracle fixture row count drifted");
        Ok(())
    }

    /// A draft that itself contains `█` must not confuse cursor tracking —
    /// the cursor row is structural, not glyph-searched. Cursor at the end
    /// of "█x\ny": inserted block lands on row 1, not the literal █ on row 0.
    #[test]
    fn literal_block_char_does_not_hijack_cursor_row() {
        let text = "\u{2588}x\ny";
        let (rows, cursor_row) = wrapped_rows(text, text.len(), 10);
        assert_eq!(rows, vec!["\u{2588}x", "y\u{2588}"]);
        assert_eq!(cursor_row, 1);
    }

    /// Out-of-range and mid-char cursors are defined behavior: clamp to the
    /// end, floor to a char boundary.
    #[test]
    fn cursor_clamps_and_floors_to_char_boundary() {
        let (rows_max, row_max) = wrapped_rows("ab", usize::MAX, 10);
        assert_eq!(rows_max, vec!["ab\u{2588}"]);
        assert_eq!(row_max, 0);
        // "世" is 3 bytes; byte 1 floors to 0 → block precedes it.
        let (rows_mid, _) = wrapped_rows("世", 1, 10);
        assert_eq!(rows_mid, vec!["\u{2588}世"]);
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
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
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
