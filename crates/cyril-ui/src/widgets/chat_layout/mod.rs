//! Private prepared-block layout, indexed windows and exact pinned Paragraph painting.
//!
//! Cell writes are adapted from the same pinned ratatui-widgets 0.3.0
//! `Paragraph::render_line` as the source-range wrapper:
//! <https://github.com/ratatui/ratatui/blob/0a2a7c0363a4806b0cf05c1915bf7cdd438f756c/ratatui-widgets/src/paragraph.rs>
//! The upstream copyright and MIT license notice are reproduced in `wrap.rs`.
//!
//! The current reference is released ratatui-widgets 0.3.2, revision
//! e665c36cb14752a61cd777fbd06dbef8474f2add, paired with ratatui-core 0.1.2:
//! <https://github.com/ratatui/ratatui/blob/e665c36cb14752a61cd777fbd06dbef8474f2add/ratatui-widgets/src/paragraph.rs>
//! Advance by `CellWidth` on each borrowed grapheme, matching the wrapper's physical-width
//! cache. Span/line raw width estimates and input scalar wrapping are separate contracts.

pub(super) mod index;
pub(super) mod wrap;

use std::collections::VecDeque;

use ratatui::buffer::{Buffer, CellWidth};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::text::Span;

use index::RowHeights;
use wrap::{SourceLine, SourceSpan, WrappedRow, wrap_lines};

/// Presentation supplies source tokens and styling; layout owns physical preparation.
pub(super) struct LogicalLine {
    pub spans: Vec<(usize, Style)>,
    pub style: Style,
    pub alignment: Alignment,
}

#[derive(Default)]
pub(super) struct PreparedRows {
    rows: Vec<WrappedRow>,
}

pub(super) fn prepare<'a>(
    lines: &[LogicalLine],
    width: u16,
    mut source_text: impl FnMut(usize) -> &'a str,
) -> PreparedRows {
    let lines: Vec<_> = lines
        .iter()
        .map(|line| SourceLine {
            spans: line
                .spans
                .iter()
                .map(|(source, style)| SourceSpan {
                    source: *source,
                    offset: 0,
                    text: source_text(*source),
                    style: *style,
                })
                .collect(),
            style: line.style,
            alignment: line.alignment,
        })
        .collect();
    PreparedRows {
        rows: wrap_lines(&lines, width),
    }
}

/// Keep opaque presentation backing and physical rows under one replacement/drop owner.
#[derive(Default)]
pub(super) struct PreparedBlock<P> {
    payload: P,
    physical: PreparedRows,
}

impl<P> PreparedBlock<P> {
    pub(super) fn new(payload: P, physical: PreparedRows) -> Self {
        Self { payload, physical }
    }

    pub(super) fn payload(&self) -> &P {
        &self.payload
    }

    #[cfg(test)]
    pub(super) fn rows(&self) -> &[WrappedRow] {
        &self.physical.rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BlockLocation {
    Leading,
    Indexed(usize),
    Trailing(usize),
}

pub(super) struct LayoutMetrics {
    pub blocks: usize,
    pub rows: usize,
    pub runs: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct WorkCounts {
    pub painted_rows: usize,
    pub resolved_runs: usize,
    pub index_steps: usize,
}

#[cfg(test)]
thread_local! {
    static WORK: std::cell::Cell<WorkCounts> = const { std::cell::Cell::new(WorkCounts {
        painted_rows: 0, resolved_runs: 0, index_steps: 0,
    }) };
}

#[cfg(test)]
pub(super) fn work_counts() -> WorkCounts {
    WORK.with(std::cell::Cell::get)
}

#[cfg(test)]
fn count(update: impl FnOnce(&mut WorkCounts)) {
    WORK.with(|work| {
        let mut value = work.get();
        update(&mut value);
        work.set(value);
    });
}

/// An indexed sequence with one leading and four ordered trailing blocks.
///
/// The caller assigns presentation meaning to the edge slots. Payloads never
/// live in a parallel collection, and layout never interprets their sources.
#[derive(Default)]
pub(super) struct Layout<P> {
    leading: PreparedBlock<P>,
    indexed: VecDeque<PreparedBlock<P>>,
    heights: RowHeights,
    trailing: [PreparedBlock<P>; 4],
}

impl<P> Layout<P> {
    pub(super) fn len(&self) -> usize {
        self.indexed.len()
    }

    pub(super) fn push(&mut self, block: PreparedBlock<P>) {
        self.heights.push(block.physical.rows.len());
        self.indexed.push_back(block);
    }

    pub(super) fn replace(&mut self, index: usize, block: PreparedBlock<P>) {
        self.heights.set(index, block.physical.rows.len());
        self.indexed[index] = block;
    }

    pub(super) fn discard_prefix(&mut self, count: usize) {
        for _ in 0..count {
            self.indexed.pop_front();
        }
        self.heights.discard_prefix(count);
        if count > 0 && self.indexed.capacity() > self.indexed.len().saturating_mul(4) {
            self.indexed.shrink_to(self.indexed.len().saturating_mul(2));
        }
    }

    pub(super) fn replace_leading(&mut self, block: PreparedBlock<P>) {
        self.leading = block;
    }

    pub(super) fn replace_trailing(&mut self, index: usize, block: PreparedBlock<P>) {
        self.trailing[index] = block;
    }

    pub(super) fn total_rows(&self) -> usize {
        self.leading.physical.rows.len()
            + self.heights.total()
            + self
                .trailing
                .iter()
                .map(|block| block.physical.rows.len())
                .sum::<usize>()
    }

    /// Inspect storage only outside drawing; payload ownership accounting belongs to its caller.
    pub(super) fn metrics(&self, mut inspect: impl FnMut(&P)) -> LayoutMetrics {
        let mut runs = 0;
        for block in self
            .indexed
            .iter()
            .chain(std::iter::once(&self.leading))
            .chain(&self.trailing)
        {
            runs += block
                .physical
                .rows
                .iter()
                .map(|row| row.runs.len())
                .sum::<usize>();
            inspect(block.payload());
        }
        LayoutMetrics {
            blocks: self.indexed.len(),
            rows: self.total_rows(),
            runs,
        }
    }

    pub(super) fn paint<'a, F>(
        &'a self,
        buffer: &mut Buffer,
        area: Rect,
        start: usize,
        mut make_source: impl FnMut(BlockLocation, &'a P) -> F,
    ) where
        F: FnMut(usize) -> &'a str,
    {
        let mut skip = start;
        let mut y = 0;
        paint_block(
            buffer,
            area,
            &self.leading,
            BlockLocation::Leading,
            &mut skip,
            &mut y,
            &mut make_source,
        );
        let indexed_rows = self.heights.total();
        if y < area.height && skip < indexed_rows {
            if let Some((mut index, mut row)) = self.heights.locate(skip) {
                #[cfg(test)]
                count(|work| work.index_steps += self.heights.locate_steps());
                skip = 0;
                while y < area.height && index < self.indexed.len() {
                    let mut within = row;
                    paint_block(
                        buffer,
                        area,
                        &self.indexed[index],
                        BlockLocation::Indexed(index),
                        &mut within,
                        &mut y,
                        &mut make_source,
                    );
                    index += 1;
                    row = 0;
                }
            }
        } else {
            skip = skip.saturating_sub(indexed_rows);
        }
        for (index, block) in self.trailing.iter().enumerate() {
            paint_block(
                buffer,
                area,
                block,
                BlockLocation::Trailing(index),
                &mut skip,
                &mut y,
                &mut make_source,
            );
        }
    }

    #[cfg(test)]
    pub(super) fn block(&self, location: BlockLocation) -> &PreparedBlock<P> {
        match location {
            BlockLocation::Leading => &self.leading,
            BlockLocation::Indexed(index) => &self.indexed[index],
            BlockLocation::Trailing(index) => &self.trailing[index],
        }
    }
}

fn paint_block<'a, P, F>(
    buffer: &mut Buffer,
    area: Rect,
    block: &'a PreparedBlock<P>,
    location: BlockLocation,
    skip: &mut usize,
    y: &mut u16,
    make_source: &mut impl FnMut(BlockLocation, &'a P) -> F,
) where
    F: FnMut(usize) -> &'a str,
{
    if *skip >= block.physical.rows.len() {
        *skip -= block.physical.rows.len();
        return;
    }
    let first = *skip;
    *skip = 0;
    let remaining = usize::from(area.height - *y);
    if remaining == 0 {
        return;
    }
    let mut source_text = make_source(location, block.payload());
    for row in block.physical.rows[first..].iter().take(remaining) {
        #[cfg(test)]
        count(|work| work.painted_rows += 1);
        paint_row(buffer, area, *y, row, |source| {
            #[cfg(test)]
            count(|work| work.resolved_runs += 1);
            source_text(source)
        });
        *y += 1;
    }
}

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
            let width = grapheme.symbol.cell_width();
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

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::*;

    fn block(text: &str, width: u16) -> PreparedBlock<String> {
        let text = text.to_string();
        let lines = [LogicalLine {
            spans: vec![(0, Style::default())],
            style: Style::default().add_modifier(Modifier::BOLD),
            alignment: Alignment::Left,
        }];
        let rows = prepare(&lines, width, |_| text.as_str());
        PreparedBlock::new(text, rows)
    }

    fn window(
        layout: &Layout<String>,
        start: usize,
        expected: [&str; 3],
        locations: &[BlockLocation],
    ) {
        let canvas = Rect::new(0, 0, 8, 8);
        let area = Rect::new(3, 2, 1, 3);
        let mut buffer = Buffer::empty(canvas);
        let mut acquired = Vec::new();
        let before = work_counts();
        layout.paint(&mut buffer, area, start, |location, payload| {
            acquired.push(location);
            move |_| payload.as_str()
        });
        assert_eq!(acquired, locations);
        let painted = expected.iter().filter(|symbol| **symbol != " ").count();
        let after = work_counts();
        assert_eq!(after.painted_rows - before.painted_rows, painted);
        assert_eq!(after.resolved_runs - before.resolved_runs, painted);
        for y in 0..canvas.height {
            for x in 0..canvas.width {
                let cell = &buffer[(x, y)];
                if x == area.x && (area.y..area.bottom()).contains(&y) {
                    let symbol = expected[usize::from(y - area.y)];
                    assert_eq!(cell.symbol(), symbol, "start {start}, ({x}, {y})");
                    assert_eq!(
                        cell.modifier.contains(Modifier::BOLD),
                        symbol != " ",
                        "start {start}, ({x}, {y})"
                    );
                } else {
                    assert_eq!(cell.symbol(), " ", "paint escaped at ({x}, {y})");
                    assert!(!cell.modifier.contains(Modifier::BOLD));
                }
            }
        }
    }

    #[test]
    fn indexed_windows_cross_edges_and_acquire_only_visible_block_sources() {
        let mut layout = Layout::default();
        layout.replace_leading(block("H", 1));
        layout.push(block("ab", 1));
        layout.push(PreparedBlock::default());
        layout.push(block("cd", 1));
        layout.replace_trailing(0, block("L", 1));
        layout.replace_trailing(1, block("ef", 1));
        layout.replace_trailing(3, block("Z", 1));
        let mut payloads = ["ab", "", "cd", "H", "L", "ef", "", "Z"].into_iter();
        let metrics = layout.metrics(|payload| {
            assert_eq!(Some(payload.as_str()), payloads.next());
        });
        assert_eq!((metrics.blocks, metrics.rows, metrics.runs), (3, 9, 9));
        assert_eq!(payloads.next(), None);
        window(
            &layout,
            0,
            ["H", "a", "b"],
            &[BlockLocation::Leading, BlockLocation::Indexed(0)],
        );
        window(
            &layout,
            1,
            ["a", "b", "c"],
            &[BlockLocation::Indexed(0), BlockLocation::Indexed(2)],
        );
        window(
            &layout,
            2,
            ["b", "c", "d"],
            &[BlockLocation::Indexed(0), BlockLocation::Indexed(2)],
        );
        window(
            &layout,
            4,
            ["d", "L", "e"],
            &[
                BlockLocation::Indexed(2),
                BlockLocation::Trailing(0),
                BlockLocation::Trailing(1),
            ],
        );
        window(
            &layout,
            6,
            ["e", "f", "Z"],
            &[BlockLocation::Trailing(1), BlockLocation::Trailing(3)],
        );
        window(&layout, 8, ["Z", " ", " "], &[BlockLocation::Trailing(3)]);
        window(&layout, 9, [" ", " ", " "], &[]);
        window(&layout, usize::MAX, [" ", " ", " "], &[]);
    }

    #[test]
    fn empty_zero_width_and_zero_height_do_not_acquire_sources() {
        let mut layout = Layout::<String>::default();
        window(&layout, 0, [" ", " ", " "], &[]);
        layout.replace_leading(block("not visible", 0));
        layout.push(block("also hidden", 0));
        layout.replace_trailing(0, block("tail", 0));
        assert_eq!(layout.total_rows(), 0);
        window(&layout, 0, [" ", " ", " "], &[]);
        layout.push(block("visible", 1));
        let area = Rect::new(3, 2, 1, 0);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 8));
        let before = work_counts();
        let mut acquired = 0;
        layout.paint(&mut buffer, area, 0, |_, payload| {
            acquired += 1;
            move |_| payload.as_str()
        });
        assert_eq!(acquired, 0);
        assert_eq!(work_counts(), before);
        assert!(buffer.content.iter().all(|cell| cell.symbol() == " "));
    }

    #[test]
    fn blank_rows_consume_window_without_resolving_source_runs() {
        let lines = [LogicalLine {
            spans: Vec::new(),
            style: Style::default(),
            alignment: Alignment::Left,
        }];
        let rows = prepare(&lines, 1, |_| unreachable!("blank line has no source span"));
        let mut layout = Layout::default();
        layout.replace_leading(PreparedBlock::new(String::new(), rows));
        layout.push(block("ab", 1));
        let area = Rect::new(3, 2, 1, 2);
        let mut buffer = Buffer::empty(area);
        let mut acquired = Vec::new();
        let before = work_counts();
        layout.paint(&mut buffer, area, 0, |location, payload| {
            acquired.push(location);
            move |_| payload.as_str()
        });
        assert_eq!(
            acquired,
            [BlockLocation::Leading, BlockLocation::Indexed(0)]
        );
        let after = work_counts();
        assert_eq!(after.painted_rows - before.painted_rows, 2);
        assert_eq!(after.resolved_runs - before.resolved_runs, 1);
        assert_eq!(buffer[(3, 2)].symbol(), " ");
        assert!(!buffer[(3, 2)].modifier.contains(Modifier::BOLD));
        assert_eq!(buffer[(3, 3)].symbol(), "a");
        assert!(buffer[(3, 3)].modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn append_replace_and_discard_update_rows_and_payloads_together() {
        let mut layout = Layout::default();
        layout.push(block("ab", 1));
        layout.push(block("c", 1));
        let untouched = layout.block(BlockLocation::Indexed(1)).rows().as_ptr();
        layout.replace(0, block("A", 1));
        layout.push(block("de", 1));
        assert_eq!(layout.total_rows(), 4);
        assert_eq!(
            layout.block(BlockLocation::Indexed(1)).rows().as_ptr(),
            untouched
        );
        window(
            &layout,
            0,
            ["A", "c", "d"],
            &[
                BlockLocation::Indexed(0),
                BlockLocation::Indexed(1),
                BlockLocation::Indexed(2),
            ],
        );
        layout.discard_prefix(1);
        assert_eq!(layout.len(), 2);
        assert_eq!(layout.total_rows(), 3);
        assert_eq!(
            layout.block(BlockLocation::Indexed(0)).rows().as_ptr(),
            untouched
        );
        window(
            &layout,
            0,
            ["c", "d", "e"],
            &[BlockLocation::Indexed(0), BlockLocation::Indexed(1)],
        );
        layout.discard_prefix(2);
        assert_eq!(layout.len(), 0);
        assert_eq!(layout.total_rows(), 0);
        window(&layout, 0, [" ", " ", " "], &[]);
    }

    #[test]
    fn prefix_compaction_moves_only_metadata_not_rows_or_payload_backing() {
        let mut layout = Layout::default();
        for index in 0..100 {
            layout.push(block(&format!("{index:02}"), 1));
        }
        let retained = layout.block(BlockLocation::Indexed(80));
        let rows = retained.rows().as_ptr();
        let text = retained.payload().as_ptr();
        let wraps = wrap::work_count();
        layout.discard_prefix(80);
        assert_eq!(wrap::work_count(), wraps);
        assert_eq!(
            layout.block(BlockLocation::Indexed(0)).rows().as_ptr(),
            rows
        );
        assert_eq!(
            layout.block(BlockLocation::Indexed(0)).payload().as_ptr(),
            text
        );
        assert_eq!(layout.len(), 20);
        assert_eq!(layout.heights.len(), 20);
        assert_eq!(layout.heights.total(), 40);
        assert_eq!(layout.heights.locate(39), Some((19, 1)));
    }
}
