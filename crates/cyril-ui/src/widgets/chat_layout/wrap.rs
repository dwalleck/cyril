//! Source-range layout matching Ratatui's pinned `Paragraph` with `Wrap { trim: false }`.
//!
//! Only row/run descriptors survive preparation. Source strings remain with their caller, and
//! visible painting can resolve any row directly without walking its logical-line prefix.
//!
//! The wrapping routine is adapted from `WordWrapper` in ratatui-widgets 0.3.0,
//! `ratatui-widgets/src/reflow.rs`, revision 0a2a7c0363a4806b0cf05c1915bf7cdd438f756c
//! (the published crate's `.cargo_vcs_info.json`). Upstream source and license:
//! <https://github.com/ratatui/ratatui/blob/0a2a7c0363a4806b0cf05c1915bf7cdd438f756c/ratatui-widgets/src/reflow.rs>
//! <https://github.com/ratatui/ratatui/blob/0a2a7c0363a4806b0cf05c1915bf7cdd438f756c/LICENSE>
//!
//! The fidelity reference is ratatui-widgets 0.3.2 at published revision
//! e665c36cb14752a61cd777fbd06dbef8474f2add, paired with ratatui-core 0.1.2:
//! <https://github.com/ratatui/ratatui/blob/e665c36cb14752a61cd777fbd06dbef8474f2add/ratatui-widgets/src/reflow.rs>
//! Borrowed `Span::styled_graphemes` symbols use `CellWidth` for cached physical columns.
//! Raw `Span::width`/`Line::width` estimates do not model this released painter's halfwidth
//! dakuten/handakuten handling. Original span boundaries and source bytes remain unchanged.
//!
//! Narrow parity exception: after upstream emits a row, its carried word/whitespace can
//! already fill the next row. Emit that retained prefix before another positive-width glyph
//! would start outside the row. All valid starts retain pinned behavior, including a final
//! wide glyph overhanging the right edge; zero-width symbols and whitespace policy are unchanged.
//!
//! The MIT License (MIT)
//!
//! Copyright (c) 2016-2022 Florian Dehau
//! Copyright (c) 2023-2025 The Ratatui Developers
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy
//! of this software and associated documentation files (the "Software"), to deal
//! in the Software without restriction, including without limitation the rights
//! to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
//! copies of the Software, and to permit persons to whom the Software is
//! furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all
//! copies or substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
//! IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
//! FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
//! AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
//! SOFTWARE.

use std::collections::VecDeque;
use std::ops::Range;

use ratatui::buffer::CellWidth;
use ratatui::layout::Alignment;
use ratatui::style::Style;
use ratatui::text::Span;

#[cfg(test)]
thread_local! {
    static WRAPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn work_count() -> usize {
    WRAPS.with(std::cell::Cell::get)
}

/// A borrowed piece of a caller-owned source, starting at that source's byte `offset`.
#[derive(Debug)]
pub(crate) struct SourceSpan<'a> {
    pub source: usize,
    pub offset: usize,
    pub text: &'a str,
    pub style: Style,
}

/// A logical line with the effective line style and resolved alignment of its Paragraph.
#[derive(Debug)]
pub(crate) struct SourceLine<'a> {
    pub spans: Vec<SourceSpan<'a>>,
    pub style: Style,
    pub alignment: Alignment,
}

/// Contiguous bytes in a source, with the effective `line.style.patch(span.style)`.
///
/// Adjacent runs may intentionally share source/style: an original span boundary must survive
/// because joining spans can change Unicode grapheme segmentation. Consumers must iterate each
/// run separately, skip zero-width graphemes, and preserve Paragraph's start-cell-only painting.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct RowRun {
    pub source: usize,
    pub range: Range<usize>,
    pub style: Style,
    pub width: u16,
}

/// A directly addressable physical row, with no owned text or persistent grapheme metadata.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct WrappedRow {
    pub runs: Vec<RowRun>,
    /// Upstream can exceed the target width by a final wide grapheme; do not clamp this value.
    pub width: u16,
    pub alignment: Alignment,
}

// Preparation-only metadata. Span identity prevents coalescing across segmentation boundaries;
// separate whitespace graphemes are needed for upstream's exact end-of-row whitespace removal.
struct PendingGrapheme {
    span: usize,
    run: RowRun,
}

/// Wrap logical lines with pinned `WordWrapper` behavior, except unsafe carried-prefix starts.
///
/// Width zero emits no rows, while each empty/control-only/overwide-only logical line at a
/// nonzero width emits one empty row. This deliberately preserves upstream's incidental
/// whitespace and wide-symbol behavior rather than substituting character wrapping.
pub(crate) fn wrap_lines(lines: &[SourceLine<'_>], width: u16) -> Vec<WrappedRow> {
    wrap_lines_with_carry::<true>(lines, width)
}

// The false instantiation is used only by tests to classify the original reference's unsafe
// positive starts, never as an independent correctness oracle for the repaired layout.
fn wrap_lines_with_carry<const REPAIR_CARRY: bool>(
    lines: &[SourceLine<'_>],
    width: u16,
) -> Vec<WrappedRow> {
    #[cfg(test)]
    WRAPS.with(|count| count.set(count.get() + 1));
    let mut rows = Vec::new();
    if width == 0 {
        return rows;
    }

    // Reuse temporary allocations across logical lines. They never escape this preparation.
    let mut pending_line = Vec::new();
    let mut pending_word = Vec::new();
    let mut pending_whitespace: VecDeque<PendingGrapheme> = VecDeque::new();

    for line in lines {
        let first_row = rows.len();
        let mut line_width = 0;
        let mut word_width = 0;
        let mut whitespace_width = 0;
        let mut non_whitespace_previous = false;

        for (span_index, source) in line.spans.iter().enumerate() {
            let span = Span::styled(source.text, source.style);
            for grapheme in span.styled_graphemes(line.style) {
                let is_whitespace = grapheme.is_whitespace();
                let symbol_width = grapheme.symbol.cell_width();

                // Match upstream: skipped symbols do not change the previous-whitespace state.
                if symbol_width > width {
                    continue;
                }

                let word_found = non_whitespace_previous && is_whitespace;
                let untrimmed_overflow =
                    pending_line.is_empty() && word_width + whitespace_width + symbol_width > width;
                if word_found || untrimmed_overflow {
                    pending_line.extend(pending_whitespace.drain(..));
                    line_width += whitespace_width;
                    pending_line.append(&mut pending_word);
                    line_width += word_width;
                    whitespace_width = 0;
                    word_width = 0;
                }

                let line_full = line_width >= width;
                let pending_word_overflow =
                    symbol_width > 0 && line_width + whitespace_width + word_width >= width;
                if line_full || pending_word_overflow {
                    let mut remaining_width = width.saturating_sub(line_width);
                    finish_row(&mut rows, &mut pending_line, line.alignment);
                    line_width = 0;

                    // The whitespace fitting the previous row is discarded even with trim=false.
                    while let Some(front) = pending_whitespace.front() {
                        let front_width = front.run.width;
                        if front_width > remaining_width {
                            break;
                        }
                        whitespace_width -= front_width;
                        remaining_width -= front_width;
                        pending_whitespace.pop_front();
                    }
                    if is_whitespace && pending_whitespace.is_empty() {
                        continue;
                    }
                }

                if REPAIR_CARRY
                    && symbol_width > 0
                    && word_width
                        >= width
                            .saturating_sub(line_width)
                            .saturating_sub(whitespace_width)
                {
                    pending_line.extend(pending_whitespace.drain(..));
                    pending_line.append(&mut pending_word);
                    finish_row(&mut rows, &mut pending_line, line.alignment);
                    line_width = 0;
                    whitespace_width = 0;
                    word_width = 0;
                }

                // ratatui-core 0.1.2 Span::styled_graphemes returns borrowed grapheme subslices,
                // filtering any grapheme containing char::is_control. Summing returned lengths
                // would therefore produce wrong offsets after tabs/CRLF/etc. Address subtraction
                // recovers the actual subslice position without unsafe code or retained pointers.
                let local_start = grapheme.symbol.as_ptr().addr() - source.text.as_ptr().addr();
                let start = source.offset + local_start;
                let pending = PendingGrapheme {
                    span: span_index,
                    run: RowRun {
                        source: source.source,
                        range: start..start + grapheme.symbol.len(),
                        style: grapheme.style,
                        width: symbol_width,
                    },
                };
                if is_whitespace {
                    whitespace_width += symbol_width;
                    pending_whitespace.push_back(pending);
                } else {
                    word_width += symbol_width;
                    pending_word.push(pending);
                }
                non_whitespace_previous = !is_whitespace;
            }
        }

        pending_line.extend(pending_whitespace.drain(..));
        pending_line.append(&mut pending_word);
        if !pending_line.is_empty() || rows.len() == first_row {
            finish_row(&mut rows, &mut pending_line, line.alignment);
        }
    }
    rows
}

fn finish_row(
    rows: &mut Vec<WrappedRow>,
    pending: &mut Vec<PendingGrapheme>,
    alignment: Alignment,
) {
    let mut runs: Vec<RowRun> = Vec::new();
    let mut previous_span = None;
    let mut width = 0;
    for grapheme in pending.drain(..) {
        let run = grapheme.run;
        width += run.width;
        if previous_span == Some(grapheme.span)
            && let Some(previous) = runs.last_mut()
            && previous.source == run.source
            && previous.range.end == run.range.start
            && previous.style == run.style
        {
            previous.range.end = run.range.end;
            previous.width += run.width;
        } else {
            runs.push(run);
        }
        previous_span = Some(grapheme.span);
    }
    rows.push(WrappedRow {
        runs,
        width,
        alignment,
    });
}

#[cfg(test)]
pub(crate) use tests::{assert_repaired_rows, legacy_reference_is_unsafe, repaired_window};

#[cfg(test)]
mod tests {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::{Color, Modifier};
    use ratatui::text::{Line, Text};
    use ratatui::widgets::{Paragraph, Widget, Wrap};

    use super::*;

    fn source_line(source: usize, text: &str) -> SourceLine<'_> {
        SourceLine {
            spans: vec![SourceSpan {
                source,
                offset: 0,
                text,
                style: Style::default(),
            }],
            style: Style::default(),
            alignment: Alignment::Left,
        }
    }

    fn reference<'a>(lines: &[SourceLine<'a>]) -> Paragraph<'a> {
        let lines: Vec<_> = lines
            .iter()
            .map(|line| {
                let spans: Vec<_> = line
                    .spans
                    .iter()
                    .map(|span| Span::styled(span.text, span.style))
                    .collect();
                Line::from(spans)
                    .style(line.style)
                    .alignment(line.alignment)
            })
            .collect();
        Paragraph::new(Text::from(lines))
            .style(background())
            .wrap(Wrap { trim: false })
    }

    fn background() -> Style {
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::ITALIC)
    }

    /// Classify only positive starts outside the unrepaired released 0.3.2 reference's row.
    /// Source-derived mode uses its physical widths and original span segmentation; real
    /// Paragraph checks its row count. This does not prove the corrected rows' correctness.
    pub(crate) fn legacy_reference_is_unsafe(lines: &[SourceLine<'_>], width: u16) -> bool {
        let legacy = wrap_lines_with_carry::<false>(lines, width);
        assert_eq!(legacy.len(), reference(lines).line_count(width));
        legacy.iter().any(|row| {
            let mut x = 0;
            for run in &row.runs {
                let text = lines
                    .iter()
                    .flat_map(|line| &line.spans)
                    .find_map(|span| {
                        if span.source != run.source {
                            return None;
                        }
                        let start = run.range.start.checked_sub(span.offset)?;
                        let end = run.range.end.checked_sub(span.offset)?;
                        span.text.get(start..end)
                    })
                    .expect("legacy run belongs to an original source span");
                let span = Span::raw(text);
                for grapheme in span.styled_graphemes(Style::default()) {
                    let advance = usize::from(grapheme.symbol.cell_width());
                    if advance > 0 && x >= usize::from(width) {
                        return true;
                    }
                    x += advance;
                }
            }
            false
        })
    }

    pub(crate) fn assert_repaired_rows<'a>(
        lines: &[SourceLine<'_>],
        rows: &[WrappedRow],
        source_text: impl Fn(usize) -> &'a str,
        width: u16,
    ) {
        if width == 0 {
            assert!(rows.is_empty());
            return;
        }
        let mut original = Vec::new();
        for line in lines {
            for source in &line.spans {
                let span = Span::styled(source.text, source.style);
                for grapheme in span.styled_graphemes(line.style) {
                    if grapheme.symbol.cell_width() <= width {
                        original.push((
                            grapheme.symbol.to_string(),
                            grapheme.style,
                            grapheme.is_whitespace(),
                            line.alignment,
                        ));
                    }
                }
            }
        }
        let mut retained = Vec::new();
        for row in rows {
            let mut measured = 0;
            let offset = match row.alignment {
                Alignment::Left => 0,
                Alignment::Center => (width / 2).saturating_sub(row.width / 2),
                Alignment::Right => width.saturating_sub(row.width),
            };
            for run in &row.runs {
                let text = source_text(run.source)
                    .get(run.range.clone())
                    .expect("retained ranges are original UTF-8 source bytes");
                let span = Span::styled(text, run.style);
                let mut run_width = 0;
                for grapheme in span.styled_graphemes(Style::default()) {
                    let advance = usize::from(grapheme.symbol.cell_width());
                    if advance > 0 {
                        assert!(
                            usize::from(offset) + measured < usize::from(width),
                            "positive glyph {:?} starts outside width {width}",
                            grapheme.symbol
                        );
                    }
                    measured += advance;
                    run_width += advance;
                    retained.push((
                        grapheme.symbol.to_string(),
                        grapheme.style,
                        grapheme.is_whitespace(),
                        row.alignment,
                    ));
                }
                assert_eq!(usize::from(run.width), run_width);
            }
            assert_eq!(usize::from(row.width), measured);
        }
        // U+200B is whitespace in the pinned library, so it follows the same discard policy.
        // Non-whitespace zero-width graphemes, by contrast, must survive with their effective style.
        assert_eq!(
            retained
                .iter()
                .filter(|grapheme| !grapheme.2)
                .collect::<Vec<_>>(),
            original
                .iter()
                .filter(|grapheme| !grapheme.2)
                .collect::<Vec<_>>(),
            "eligible non-whitespace graphemes, effective styles and alignment are conserved"
        );
        let mut remaining = original.iter();
        for grapheme in &retained {
            loop {
                let next = remaining
                    .next()
                    .expect("no invented or reordered source grapheme");
                if next == grapheme {
                    break;
                }
                assert!(next.2, "only pinned whitespace may be discarded");
            }
        }
        assert!(remaining.all(|grapheme| grapheme.2));
    }

    /// Real Paragraph paints each explicitly selected corrected row. This is only a cell/style
    /// oracle: source conservation and literal goldens below establish the repaired wrapping.
    pub(crate) fn repaired_window<'a>(
        rows: &[WrappedRow],
        source_text: impl Fn(usize) -> &'a str,
        start: usize,
        area: Rect,
        background: Style,
    ) -> Buffer {
        let mut expected = Buffer::empty(area);
        expected.set_style(area, background);
        if area.is_empty() {
            return expected;
        }
        for (y, row) in rows
            .iter()
            .skip(start)
            .take(usize::from(area.height))
            .enumerate()
        {
            let spans: Vec<_> = row
                .runs
                .iter()
                .map(|run| Span::styled(&source_text(run.source)[run.range.clone()], run.style))
                .collect();
            let paragraph = Paragraph::new(Line::from(spans).alignment(row.alignment))
                .style(background)
                .wrap(Wrap { trim: false });
            assert_eq!(
                paragraph.line_count(area.width),
                1,
                "explicit row stays one row"
            );
            paragraph.render(
                Rect::new(area.x, area.y + y as u16, area.width, 1),
                &mut expected,
            );
        }
        expected
    }

    // A descriptor sink, not a second wrapping oracle: emit only the selected source ranges.
    // Its cell writes follow pinned paragraph.rs::render_line, including ignored zero-width
    // graphemes and untouched continuation cells. The independent expected Buffer comes from
    // the real wrapped Paragraph, never from this helper or reconstructed expected strings.
    fn paint_window(rows: &[WrappedRow], sources: &[&str], start: usize, area: Rect) -> Buffer {
        let mut buffer = Buffer::empty(area);
        buffer.set_style(area, background());
        if area.is_empty() {
            return buffer;
        }
        for (y, row) in rows
            .iter()
            .skip(start)
            .take(usize::from(area.height))
            .enumerate()
        {
            super::super::paint_row(&mut buffer, area, y as u16, row, |source| sources[source]);
        }
        buffer
    }

    fn assert_window(
        rows: &[WrappedRow],
        sources: &[&str],
        paragraph: &Paragraph<'_>,
        width: u16,
        start: usize,
        height: u16,
    ) {
        let area = Rect::new(3, 2, width, height);
        let mut expected = Buffer::empty(area);
        let Ok(scroll) = u16::try_from(start) else {
            panic!("reference window must fit Paragraph scroll");
        };
        paragraph
            .clone()
            .scroll((scroll, 0))
            .render(area, &mut expected);
        assert_eq!(
            paint_window(rows, sources, start, area),
            expected,
            "width={width}, start={start}, height={height}"
        );
    }

    fn assert_matches(lines: &[SourceLine<'_>], sources: &[&str], width: u16) {
        let rows = wrap_lines(lines, width);
        let paragraph = reference(lines);
        let unsafe_reference = legacy_reference_is_unsafe(lines, width);
        if unsafe_reference {
            assert_repaired_rows(lines, &rows, |source| sources[source], width);
        } else {
            assert_eq!(rows.len(), paragraph.line_count(width), "width={width}");
        }
        for row in &rows {
            let mut row_width = 0;
            for run in &row.runs {
                let Some(text) = sources[run.source].get(run.range.clone()) else {
                    panic!("run bytes must remain UTF-8 boundaries");
                };
                let span = Span::raw(text);
                let actual_width: usize = span
                    .styled_graphemes(Style::default())
                    .map(|grapheme| usize::from(grapheme.symbol.cell_width()))
                    .sum();
                assert_eq!(usize::from(run.width), actual_width);
                row_width += actual_width;
            }
            assert_eq!(usize::from(row.width), row_width);
        }
        for start in [
            0,
            1,
            rows.len() / 2,
            rows.len().saturating_sub(3),
            rows.len(),
            rows.len() + 1,
        ] {
            for height in [0, 1, 2, 5] {
                if unsafe_reference {
                    let area = Rect::new(3, 2, width, height);
                    assert_eq!(
                        paint_window(&rows, sources, start, area),
                        repaired_window(&rows, |source| sources[source], start, area, background()),
                        "repaired width={width}, start={start}, height={height}"
                    );
                } else {
                    assert_window(&rows, sources, &paragraph, width, start, height);
                }
            }
        }
    }

    #[test]
    fn legacy_classifier_detects_only_actual_unsafe_positive_starts() {
        for text in ["a界a", "a🌞b", "a\u{3000}b", "\u{301}a界b", "\u{903}界b"] {
            let lines = [source_line(0, text)];
            assert!(legacy_reference_is_unsafe(&lines, 2), "{text:?}");
            let rows = wrap_lines(&lines, 2);
            assert_repaired_rows(&lines, &rows, |_| text, 2);
        }
        for (text, width) in [
            ("界界", 3), // Legal final overhang is NOT an unsafe positive start.
            ("a界", 2),
            ("ordinary ASCII words", 4),
            ("e\u{301} αβ", 3),
            ("\u{301}A\u{200b}", 4),
            ("👩‍💻🙂", 4),
            ("", 0),
        ] {
            assert!(
                !legacy_reference_is_unsafe(&[source_line(0, text)], width),
                "{text:?}"
            );
        }
        // Original span segmentation matters: splitting a combining sequence is not a reason
        // to exempt it, while changing styles around the carried wide glyph still is unsafe.
        let text = "a\u{301}b";
        let mut split = source_line(0, "");
        split.spans = [0..1, 1..3, 3..4]
            .into_iter()
            .map(|range| SourceSpan {
                source: 0,
                offset: range.start,
                text: &text[range],
                style: Style::default(),
            })
            .collect();
        assert!(!legacy_reference_is_unsafe(std::slice::from_ref(&split), 2));
        assert_matches(&[split], &[text], 2);
        let line = SourceLine {
            spans: vec![
                SourceSpan {
                    source: 0,
                    offset: 0,
                    text: "a",
                    style: Style::default(),
                },
                SourceSpan {
                    source: 1,
                    offset: 0,
                    text: "界",
                    style: Style::default().fg(Color::Yellow),
                },
                SourceSpan {
                    source: 2,
                    offset: 0,
                    text: "b",
                    style: Style::default(),
                },
            ],
            style: Style::default().add_modifier(Modifier::BOLD),
            alignment: Alignment::Right,
        };
        assert!(legacy_reference_is_unsafe(std::slice::from_ref(&line), 2));
        assert_matches(&[line], &["a", "界", "b"], 2);
    }

    #[test]
    fn carried_prefix_repair_has_literal_rows_cells_styles_and_windows() {
        type CarryFixture<'a> = (&'a str, &'a [&'a str], &'a [[&'a str; 2]]);
        let fixtures: &[CarryFixture<'_>] = &[
            (
                "a界a",
                &["a", "界", "a"],
                &[["a", " "], ["界", " "], ["a", " "]],
            ),
            (
                "a🌞b",
                &["a", "🌞", "b"],
                &[["a", " "], ["🌞", " "], ["b", " "]],
            ),
            (
                "a\u{3000}b",
                &["a", "\u{3000}", "b"],
                &[["a", " "], ["\u{3000}", " "], ["b", " "]],
            ),
            (
                "\u{301}a界b",
                &["\u{301}a", "界", "b"],
                &[["a", " "], ["界", " "], ["b", " "]],
            ),
            (
                "\u{903}界b",
                &["\u{903}", "界", "b"],
                &[["\u{903}", " "], ["界", " "], ["b", " "]],
            ),
            // ZWSP fits the prior row and is discarded by the original whitespace policy;
            // the full-width whitespace above does not fit and must survive the repair.
            (
                "a\u{200b}界b",
                &["a", "界", "b"],
                &[["a", " "], ["界", " "], ["b", " "]],
            ),
        ];
        for &(text, expected_rows, expected_cells) in fixtures {
            let mut line = source_line(0, text);
            line.style = Style::default()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD);
            line.spans[0].style = Style::default()
                .fg(Color::Yellow)
                .remove_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED);
            let style = line.style.patch(line.spans[0].style);
            let rows = wrap_lines(std::slice::from_ref(&line), 2);
            let actual_rows: Vec<String> = rows
                .iter()
                .map(|row| {
                    row.runs
                        .iter()
                        .map(|run| &text[run.range.clone()])
                        .collect()
                })
                .collect();
            assert_eq!(actual_rows, expected_rows, "{text:?}");
            assert!(
                legacy_reference_is_unsafe(std::slice::from_ref(&line), 2),
                "{text:?}"
            );
            assert_repaired_rows(std::slice::from_ref(&line), &rows, |_| text, 2);
            for (row, expected) in rows.iter().zip(expected_rows) {
                let expected_width: usize = Span::raw(*expected)
                    .styled_graphemes(Style::default())
                    .map(|grapheme| usize::from(grapheme.symbol.cell_width()))
                    .sum();
                assert_eq!(usize::from(row.width), expected_width);
                assert!(row.runs.iter().all(|run| run.style == style));
            }
            for start in 0..=expected_rows.len() + 1 {
                for height in [0, 1, 2, 5] {
                    let area = Rect::new(3, 2, 2, height);
                    let mut expected = Buffer::empty(area);
                    expected.set_style(area, background());
                    for (y, cells) in expected_cells
                        .iter()
                        .skip(start)
                        .take(usize::from(height))
                        .enumerate()
                    {
                        for (x, symbol) in cells.iter().enumerate() {
                            expected[(3 + x as u16, 2 + y as u16)].set_symbol(symbol);
                        }
                        expected[(3, 2 + y as u16)].set_style(style);
                    }
                    assert_eq!(
                        paint_window(&rows, &[text], start, area),
                        expected,
                        "{text:?}, start={start}, height={height}"
                    );
                }
            }
            assert_matches(std::slice::from_ref(&line), &[text], 2);
        }
    }

    #[test]
    fn released_cell_width_has_literal_rows_cells_styles_and_windows() {
        struct Fixture<'a> {
            text: &'a str,
            width: u16,
            rows: &'a [&'a str],
            row_widths: &'a [u16],
            cells: &'a [&'a [&'a str]],
            unsafe_reference: bool,
        }
        let fixtures = [
            Fixture {
                text: "ｶﾞZ",
                width: 3,
                rows: &["ｶﾞZ"],
                row_widths: &[3],
                cells: &[&["ｶﾞ", " ", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ﾊﾟZ",
                width: 3,
                rows: &["ﾊﾟZ"],
                row_widths: &[3],
                cells: &[&["ﾊﾟ", " ", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "aﾞZ",
                width: 3,
                rows: &["aﾞZ"],
                row_widths: &[3],
                cells: &[&["aﾞ", " ", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "1ﾟZ",
                width: 3,
                rows: &["1ﾟZ"],
                row_widths: &[3],
                cells: &[&["1ﾟ", " ", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ｶﾞZ",
                width: 2,
                rows: &["ｶﾞ", "Z"],
                row_widths: &[2, 1],
                cells: &[&["ｶﾞ", " "], &["Z", " "]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ｶﾞZ",
                width: 1,
                rows: &["Z"],
                row_widths: &[1],
                cells: &[&["Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "あﾞZ",
                width: 3,
                rows: &["あﾞ", "Z"],
                row_widths: &[3, 1],
                cells: &[&["あﾞ", " ", " "], &["Z", " ", " "]],
                unsafe_reference: false,
            },
            Fixture {
                text: "aあﾞZ",
                width: 3,
                rows: &["a", "あﾞ", "Z"],
                row_widths: &[1, 3, 1],
                cells: &[&["a", " ", " "], &["あﾞ", " ", " "], &["Z", " ", " "]],
                unsafe_reference: true,
            },
            Fixture {
                text: "aｶﾞZ",
                width: 2,
                rows: &["a", "ｶﾞ", "Z"],
                row_widths: &[1, 2, 1],
                cells: &[&["a", " "], &["ｶﾞ", " "], &["Z", " "]],
                unsafe_reference: true,
            },
            Fixture {
                text: "aﾊﾟZ",
                width: 2,
                rows: &["a", "ﾊﾟ", "Z"],
                row_widths: &[1, 2, 1],
                cells: &[&["a", " "], &["ﾊﾟ", " "], &["Z", " "]],
                unsafe_reference: true,
            },
            Fixture {
                text: "ｶﾞｶﾞ",
                width: 3,
                rows: &["ｶﾞｶﾞ"],
                row_widths: &[4],
                cells: &[&["ｶﾞ", " ", "ｶﾞ"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ｶﾞｶﾞZ",
                width: 3,
                rows: &["ｶﾞ", "ｶﾞZ"],
                row_widths: &[2, 3],
                cells: &[&["ｶﾞ", " ", " "], &["ｶﾞ", " ", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "界界",
                width: 3,
                rows: &["界界"],
                row_widths: &[4],
                cells: &[&["界", " ", "界"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ｶ\u{3099}Z",
                width: 2,
                rows: &["ｶ\u{3099}Z"],
                row_widths: &[2],
                cells: &[&["ｶ\u{3099}", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "ﾊ\u{309a}Z",
                width: 2,
                rows: &["ﾊ\u{309a}Z"],
                row_widths: &[2],
                cells: &[&["ﾊ\u{309a}", "Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "\u{ff9e}Z",
                width: 1,
                rows: &["\u{ff9e}", "Z"],
                row_widths: &[1, 1],
                cells: &[&["\u{ff9e}"], &["Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "\u{ff9f}Z",
                width: 1,
                rows: &["\u{ff9f}", "Z"],
                row_widths: &[1, 1],
                cells: &[&["\u{ff9f}"], &["Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "\u{3099}Z",
                width: 1,
                rows: &["\u{3099}Z"],
                row_widths: &[1],
                cells: &[&["Z"]],
                unsafe_reference: false,
            },
            Fixture {
                text: "\u{309a}Z",
                width: 1,
                rows: &["\u{309a}Z"],
                row_widths: &[1],
                cells: &[&["Z"]],
                unsafe_reference: false,
            },
        ];
        for fixture in fixtures {
            let mut line = source_line(0, fixture.text);
            line.style = Style::default()
                .fg(Color::Cyan)
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD);
            line.spans[0].style = Style::default()
                .fg(Color::Yellow)
                .remove_modifier(Modifier::BOLD)
                .add_modifier(Modifier::UNDERLINED);
            let style = line.style.patch(line.spans[0].style);
            let rows = wrap_lines(std::slice::from_ref(&line), fixture.width);
            let actual_rows: Vec<String> = rows
                .iter()
                .map(|row| {
                    row.runs
                        .iter()
                        .map(|run| &fixture.text[run.range.clone()])
                        .collect()
                })
                .collect();
            assert_eq!(actual_rows, fixture.rows, "{:?}", fixture.text);
            assert_eq!(
                rows.iter().map(|row| row.width).collect::<Vec<_>>(),
                fixture.row_widths,
                "{:?}",
                fixture.text
            );
            assert_eq!(
                legacy_reference_is_unsafe(std::slice::from_ref(&line), fixture.width),
                fixture.unsafe_reference,
                "{:?}",
                fixture.text
            );
            assert_repaired_rows(
                std::slice::from_ref(&line),
                &rows,
                |_| fixture.text,
                fixture.width,
            );
            assert!(
                rows.iter()
                    .flat_map(|row| &row.runs)
                    .all(|run| run.style == style)
            );
            assert_eq!(fixture.cells.len(), fixture.rows.len());
            for start in 0..=fixture.rows.len() + 1 {
                for height in [0, 1, 2, 5] {
                    let area = Rect::new(3, 2, fixture.width, height);
                    let mut expected = Buffer::empty(area);
                    expected.set_style(area, background());
                    for (y, cells) in fixture
                        .cells
                        .iter()
                        .skip(start)
                        .take(usize::from(height))
                        .enumerate()
                    {
                        assert_eq!(cells.len(), usize::from(fixture.width));
                        for (x, symbol) in cells.iter().enumerate() {
                            let cell = &mut expected[(3 + x as u16, 2 + y as u16)];
                            cell.set_symbol(symbol);
                            if *symbol != " " {
                                cell.set_style(style);
                            }
                        }
                    }
                    assert_eq!(
                        paint_window(&rows, &[fixture.text], start, area),
                        expected,
                        "{:?}, start={start}, height={height}",
                        fixture.text
                    );
                }
            }
            assert_matches(std::slice::from_ref(&line), &[fixture.text], fixture.width);
        }
    }

    #[test]
    fn released_cell_width_preserves_original_span_boundaries_and_styles() {
        struct Fixture<'a> {
            text: &'a str,
            first: &'a str,
            width: u16,
            cells: [&'a str; 3],
        }
        let fixtures = [
            Fixture {
                text: "ｶﾞZ",
                first: "ｶ",
                width: 3,
                cells: ["ｶ", "ﾞ", "Z"],
            },
            Fixture {
                text: "ﾊﾟZ",
                first: "ﾊ",
                width: 3,
                cells: ["ﾊ", "ﾟ", "Z"],
            },
            Fixture {
                text: "ｶ\u{3099}Z",
                first: "ｶ",
                width: 2,
                cells: ["ｶ", "Z", " "],
            },
            Fixture {
                text: "ﾊ\u{309a}Z",
                first: "ﾊ",
                width: 2,
                cells: ["ﾊ", "Z", " "],
            },
        ];
        for fixture in fixtures {
            let source = format!("prefix:{}:suffix", fixture.text);
            let start = "prefix:".len();
            let split = start + fixture.first.len();
            let end = start + fixture.text.len();
            let line_style = Style::default()
                .bg(Color::Black)
                .add_modifier(Modifier::BOLD);
            let first_style = Style::default().fg(Color::Cyan);
            let second_style = Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::UNDERLINED);
            let mut line = SourceLine {
                spans: vec![
                    SourceSpan {
                        source: 0,
                        offset: start,
                        text: &source[start..split],
                        style: first_style,
                    },
                    SourceSpan {
                        source: 0,
                        offset: split,
                        text: &source[split..end],
                        style: second_style,
                    },
                ],
                style: line_style,
                alignment: Alignment::Left,
            };
            let rows = wrap_lines(std::slice::from_ref(&line), 3);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].width, fixture.width);
            assert_eq!(rows[0].runs.len(), 2, "original span boundary survives");
            assert_eq!(rows[0].runs[0].range, start..split);
            assert_eq!(rows[0].runs[1].range, split..end);
            assert_eq!(
                &source[start..end],
                fixture.text,
                "source bytes are untouched"
            );
            let area = Rect::new(3, 2, 3, 1);
            let mut expected = Buffer::empty(area);
            expected.set_style(area, background());
            for (x, symbol) in fixture.cells.iter().enumerate() {
                let cell = &mut expected[(3 + x as u16, 2)];
                cell.set_symbol(symbol);
                if *symbol != " " {
                    cell.set_style(line_style.patch(if x == 0 {
                        first_style
                    } else {
                        second_style
                    }));
                }
            }
            assert_eq!(paint_window(&rows, &[source.as_str()], 0, area), expected);
            for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
                line.alignment = alignment;
                for width in [0, 1, 2, 3, 4, 7] {
                    assert_matches(std::slice::from_ref(&line), &[source.as_str()], width);
                }
            }
        }
    }

    #[test]
    fn exact_paragraph_cells_for_whitespace_unicode_controls_and_windows() {
        let fixtures = [
            "",
            " ",
            "                   ",
            "  indented words  with  trailing spaces   ",
            "word                     next",
            "abcdefghijklmno",
            "a界",
            "界界",
            "界界a",
            "界 a  界界  b ",
            "👩‍💻🙂👨‍👩‍👧‍👦🇬🇧🇺🇸 words",
            "e\u{301} a\u{308}\u{301} o\u{308}x",
            "ｶﾞZ ﾊﾟZ aﾞZ 1ﾟZ あﾞZ 紅ﾟZ",
            "aｶﾞZ aﾊﾟZ ｶﾞｶﾞZ",
            "ｶ\u{3099}Z ﾊ\u{309a}Z",
            "\u{ff9e}Z \u{ff9f}Z \u{3099}Z \u{309a}Z",
            "\u{301}\u{200d}\u{fe0f}abc\u{200b}def\u{200b}ghi",
            "ab\u{a0}cd ef\u{a0}\u{a0}gh",
            "ab\u{200b}cd\u{200b}\u{200b}ef",
            "\u{2003}\u{3000}abc\u{2002} def\u{3000}",
            "\0\t\r\n\u{7f}\u{85}",
            "\tA\0B\r\n界\u{7f}e\u{301}\u{85}Z",
            "🇬\0🇧 a\t\u{301}b",
            "| column | wide 界 | trailing |",
            "    let value = function(argument);    ",
        ];
        for text in fixtures {
            for alignment in [Alignment::Left, Alignment::Center, Alignment::Right] {
                let mut line = source_line(0, text);
                line.alignment = alignment;
                line.style = Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD);
                for width in [0, 1, 2, 3, 4, 7, 16, 40] {
                    assert_matches(std::slice::from_ref(&line), &[text], width);
                }
            }
        }
        let lines = [
            source_line(0, "alpha beta  "),
            source_line(1, ""),
            source_line(2, "界  gamma"),
        ];
        for width in [0, 1, 2, 5, 12] {
            assert_matches(&lines, &["alpha beta  ", "", "界  gamma"], width);
            assert_matches(&[], &[], width);
        }
    }

    #[test]
    fn span_boundaries_styles_and_nonzero_offsets_match_paragraph() {
        let source = "prefix:ab\u{301}🇬🇧👩‍💻  words\t界 :suffix";
        let body = &source["prefix:".len()..source.len() - " :suffix".len()];
        let boundaries = [
            0,
            "ab".len(),
            "ab\u{301}🇬".len(),
            "ab\u{301}🇬🇧👩".len(),
            "ab\u{301}🇬🇧👩‍".len(),
            body.len(),
        ];
        for styled in [false, true] {
            let spans = boundaries
                .windows(2)
                .enumerate()
                .map(|(index, bounds)| SourceSpan {
                    source: 0,
                    offset: "prefix:".len() + bounds[0],
                    text: &body[bounds[0]..bounds[1]],
                    style: if styled && index.is_multiple_of(2) {
                        Style::default()
                            .fg(Color::Yellow)
                            .remove_modifier(Modifier::BOLD)
                            .add_modifier(Modifier::UNDERLINED)
                    } else {
                        Style::default()
                    },
                })
                .collect();
            let line = SourceLine {
                spans,
                style: Style::default()
                    .fg(Color::Blue)
                    .bg(Color::Black)
                    .add_modifier(Modifier::BOLD),
                alignment: Alignment::Center,
            };
            for width in 0..=16 {
                assert_matches(std::slice::from_ref(&line), &[source], width);
            }
        }
    }

    #[test]
    fn compact_runs_skip_filtered_control_bytes_without_losing_source_offsets() {
        let source = "prefix\t\r\n\0ab\u{7f}界\u{85}e\u{301} suffix";
        let offset = "prefix".len();
        let end = source.len() - " suffix".len();
        let style = Style::default().fg(Color::Green);
        let lines = [SourceLine {
            spans: vec![SourceSpan {
                source: 0,
                offset,
                text: &source[offset..end],
                style,
            }],
            style: Style::default().bg(Color::Black),
            alignment: Alignment::Right,
        }];
        let rows = wrap_lines(&lines, 40);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].width, 5);
        assert_eq!(rows[0].alignment, Alignment::Right);
        assert_eq!(rows[0].runs.len(), 3);
        let expected = ["ab", "界", "e\u{301}"];
        for (run, text) in rows[0].runs.iter().zip(expected) {
            assert_eq!(&source[run.range.clone()], text);
            let Some(start) = source.find(text) else {
                panic!("expected text must occur in the original fixture");
            };
            assert_eq!(run.range.start, start);
            assert_eq!(run.style, lines[0].style.patch(style));
        }
        assert_matches(&lines, &[source], 40);
    }

    #[test]
    fn pinned_overhang_zero_width_and_whitespace_have_positive_cell_assertions() {
        let source = "界界";
        let mut line = source_line(0, source);
        line.style = Style::default().fg(Color::Cyan).bg(Color::Black);
        let rows = wrap_lines(std::slice::from_ref(&line), 3);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].width, 4);
        let area = Rect::new(3, 2, 3, 1);
        let actual = paint_window(&rows, &[source], 0, area);
        let mut expected = Buffer::empty(area);
        reference(std::slice::from_ref(&line)).render(area, &mut expected);
        assert_eq!(actual, expected);
        assert_eq!(actual[(3, 2)].symbol(), "界");
        assert_eq!(actual[(5, 2)].symbol(), "界");
        assert_eq!(actual[(4, 2)].symbol(), " ");
        assert_eq!(actual[(4, 2)].bg, Color::DarkGray);
        assert_eq!(actual[(3, 2)].bg, Color::Black);

        let source = "\u{301}A\u{200b}";
        let rows = wrap_lines(&[source_line(0, source)], 4);
        let actual = paint_window(&rows, &[source], 0, Rect::new(3, 2, 4, 1));
        assert_eq!(rows[0].width, 1);
        assert_eq!(actual[(3, 2)].symbol(), "A");
        assert_eq!(actual[(4, 2)].symbol(), " ");
        assert_matches(&[source_line(0, source)], &[source], 4);

        // These incidental trim=false results are part of the pinned reference, not a policy
        // to preserve all whitespace or to trim all whitespace.
        for (source, width, expected) in [
            ("ab   ", 4, vec!["ab"]),
            ("ab   ", 5, vec!["ab   "]),
            ("  ab", 3, vec!["  a", "b"]),
        ] {
            let rows = wrap_lines(&[source_line(0, source)], width);
            let actual: Vec<String> = rows
                .iter()
                .map(|row| {
                    row.runs
                        .iter()
                        .map(|run| &source[run.range.clone()])
                        .collect()
                })
                .collect();
            assert_eq!(actual, expected);
            assert_matches(&[source_line(0, source)], &[source], width);
        }
    }

    #[test]
    fn ordinary_rows_are_coalesced_without_copied_text() {
        let source = "alpha beta gamma delta ".repeat(100);
        let lines = [source_line(0, &source)];
        let rows = wrap_lines(&lines, 23);
        assert!(rows.len() > 50);
        for row in &rows {
            assert_eq!(row.runs.len(), 1);
            let run = &row.runs[0];
            assert!(source.get(run.range.clone()).is_some());
        }
        assert_matches(&lines, &[&source], 23);
    }

    #[test]
    fn deterministic_mixed_grapheme_sequences_match_real_paragraph() {
        let alphabet = [
            "a", "b", " ", "  ", "界", "🙂", "e\u{301}", "\u{301}", "\u{200b}", "\u{a0}", "\t",
            "\r\n",
        ];
        let mut seed = 1_u32;
        for case in 0..128 {
            let mut text = String::new();
            for _ in 0..24 {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                text.push_str(alphabet[(seed as usize) % alphabet.len()]);
            }
            let mut line = source_line(0, &text);
            line.alignment = match case % 3 {
                0 => Alignment::Left,
                1 => Alignment::Center,
                _ => Alignment::Right,
            };
            line.spans[0].style = Style::default()
                .fg(Color::Magenta)
                .add_modifier(Modifier::REVERSED);
            for width in [1, 2, 3, 5, 11] {
                assert_matches(std::slice::from_ref(&line), &[&text], width);
            }
        }
    }

    #[test]
    fn huge_logical_line_selects_deep_rows_including_beyond_u16_scroll() {
        let source =
            "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".repeat(20_000);
        let width = 17_u16;
        let lines = [source_line(0, &source)];
        let rows = wrap_lines(&lines, width);
        let paragraph = reference(&lines);
        assert_eq!(rows.len(), paragraph.line_count(width));
        assert!(rows.len() > usize::from(u16::MAX) + 1_000);
        for start in [0, 32_767, 65_530] {
            assert_window(&rows, &[&source], &paragraph, width, start, 7);
        }

        // Paragraph's u16 scroll cannot name these rows. An unbroken ASCII word has exact
        // width-sized source boundaries, so use the real Paragraph on the corresponding suffix
        // as the independent reference instead of narrowing/clamping the requested row index.
        for start in [65_536, rows.len() - 7, rows.len() - 1] {
            assert_eq!(rows[start].runs[0].range.start, start * usize::from(width));
            let suffix = &source[start * usize::from(width)..];
            let area = Rect::new(3, 2, width, 7);
            let mut expected = Buffer::empty(area);
            reference(&[source_line(0, suffix)]).render(area, &mut expected);
            assert_eq!(paint_window(&rows, &[&source], start, area), expected);
        }
        assert!(rows.iter().all(|row| row.runs.len() == 1));
    }
}
