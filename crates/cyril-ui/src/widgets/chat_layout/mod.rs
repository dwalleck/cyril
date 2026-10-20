//! Private physical-row layout and exact pinned Paragraph cell painting.
//!
//! Cell writes are adapted from the same pinned ratatui-widgets 0.3.0
//! `Paragraph::render_line` as the source-range wrapper:
//! <https://github.com/ratatui/ratatui/blob/0a2a7c0363a4806b0cf05c1915bf7cdd438f756c/ratatui-widgets/src/paragraph.rs>
//! The upstream copyright and MIT license notice are reproduced in `wrap.rs`.

pub(super) mod index;
pub(super) mod wrap;

use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Span;
use unicode_width::UnicodeWidthStr;

use wrap::WrappedRow;

/// Paint one already selected physical row. Like pinned Paragraph::render_line,
/// only a nonzero-width grapheme's start cell is written. In particular, wide
/// continuation cells retain the canvas style, and a final wide glyph may hang
/// beyond the nominal row width. Original span boundaries remain separate runs.
pub(super) fn paint_row<'a>(
    buffer: &mut Buffer,
    area: Rect,
    y: u16,
    row: &WrappedRow,
    mut source_text: impl FnMut(usize) -> &'a str,
) {
    let mut x = match row.alignment {
        Alignment::Left => 0,
        Alignment::Center => (area.width / 2).saturating_sub(row.width / 2),
        Alignment::Right => area.width.saturating_sub(row.width),
    };
    for run in &row.runs {
        let source = source_text(run.source);
        let text = &source[run.range.clone()];
        let span = Span::styled(text, run.style);
        for grapheme in span.styled_graphemes(Style::default()) {
            let width = grapheme.symbol.width() as u16;
            if width == 0 {
                continue;
            }
            buffer[(area.x + x, area.y + y)]
                .set_symbol(grapheme.symbol)
                .set_style(grapheme.style);
            x += width;
        }
    }
}
