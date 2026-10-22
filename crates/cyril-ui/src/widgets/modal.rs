use ratatui::buffer::CellWidth;
use ratatui::prelude::*;

/// Clear an overlay's clipped rectangle and every painted grapheme span that
/// intersects it, including leads left of the rectangle and tails right of it.
///
/// Cells/styles belonging to disjoint spans and rows remain untouched. Walk
/// each affected row from its buffer origin: continuation cells have no owner
/// backpointer, and a painted grapheme can occupy more than two columns.
pub(crate) fn clear(frame: &mut Frame<'_>, area: Rect) {
    let buffer = frame.buffer_mut();
    let bounds = *buffer.area();
    let area = area.intersection(bounds);
    if area.area() == 0 {
        return;
    }

    for y in area.top()..area.bottom() {
        let mut x = bounds.left();
        while x < area.right() {
            // Compute the complete, row-bounded span before resetting its lead.
            let end = x
                .saturating_add(buffer[(x, y)].cell_width().max(1))
                .min(bounds.right());
            if end > area.left() {
                for column in x..end {
                    buffer[(column, y)].reset();
                }
            }
            x = end;
        }
    }
}

/// Compute the rect for a centered modal popup.
///
/// Clamps the desired size to `area` minus a 4-cell margin pair on each
/// axis, then centers with floor division (a one-cell remainder lands on
/// the right/bottom margin). Total function: degenerate areas produce
/// empty rects, never a panic.
///
/// Deliberately NOT `ratatui::layout::Rect::centered`: that helper routes
/// through the `Flex::Center` layout solver, which neither applies the
/// 4-cell margin clamp nor guarantees the floor-division rounding this
/// crate's popups have always used (cyril-cc5e claim C8 pins parity).
pub fn centered(area: Rect, desired_width: u16, desired_height: u16) -> Rect {
    let width = popup_width(area, desired_width);
    let height = desired_height.min(area.height.saturating_sub(4));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
}

/// The width [`centered`] and [`place`] give a popup in `area`, independent of
/// its height — lets a popup lay out width-dependent content (a wrapped path,
/// cyril-rcub) before it knows how tall it will be.
pub fn popup_width(area: Rect, desired_width: u16) -> u16 {
    desired_width.min(area.width.saturating_sub(4))
}

/// Compute the rect for a popup that must never cover the input area.
///
/// `input_top` is the absolute row of the input box's top border. The
/// placement region is rows `[area.y + 1, input_top)` — the toolbar row and
/// everything from the input downward are protected (cyril-a14l claim C7).
/// Width and the initial height clamp match [`centered`] exactly, and the
/// result IS the [`centered`] rect whenever that rect already sits above
/// `input_top` (claim C9 parity). Otherwise the popup shrinks to the region
/// height and anchors directly above the input.
///
/// Returns `None` when the region has no rows or the area has no clampable
/// width — there is nowhere to draw, and callers must render nothing
/// (including clearing). `Option` instead of an empty-rect sentinel per the
/// workspace rule "use `Option` for absent values, not sentinels".
pub fn place(area: Rect, input_top: u16, desired_width: u16, desired_height: u16) -> Option<Rect> {
    let legacy = centered(area, desired_width, desired_height);
    if legacy.area() == 0 {
        return None;
    }
    if legacy.bottom() <= input_top {
        return Some(legacy);
    }
    let region_top = area.y.saturating_add(1);
    let region_height = input_top.saturating_sub(region_top);
    if region_height == 0 {
        return None;
    }
    let height = legacy.height.min(region_height);
    Some(Rect::new(
        legacy.x,
        input_top.saturating_sub(height),
        legacy.width,
        height,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::{Buffer, Cell};
    use ratatui::widgets::{Block, Borders, Paragraph};
    use ratatui::{Terminal, TerminalOptions, Viewport};

    fn fixed_terminal(area: Rect) -> anyhow::Result<Terminal<TestBackend>> {
        Terminal::with_options(
            TestBackend::new(24, 8),
            TerminalOptions {
                viewport: Viewport::Fixed(area),
            },
        )
        .map_err(Into::into)
    }

    fn styled_mixed_frame() -> anyhow::Result<(Terminal<TestBackend>, Buffer)> {
        let mut terminal = fixed_terminal(Rect::new(3, 2, 20, 5))?;
        let before = {
            let mut frame = terminal.get_frame();
            let area = frame.area();
            frame
                .buffer_mut()
                .set_style(area, Style::default().bg(Color::Blue));
            frame.render_widget(
                Paragraph::new("A界ｶﾞあﾞ☀\u{fe0f}Z").style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Rect::new(3, 4, 20, 1),
            );
            frame.buffer_mut().clone()
        };
        Ok((terminal, before))
    }

    #[test]
    fn clear_evicts_both_crossing_spans_and_preserves_disjoint_styles() -> anyhow::Result<()> {
        let (mut terminal, before) = styled_mixed_frame()?;
        assert_eq!(before[(4, 4)].symbol(), "界");
        assert_eq!(before[(6, 4)].symbol(), "ｶﾞ");
        assert_eq!(before[(8, 4)].symbol(), "あﾞ");
        assert_eq!(before[(8, 4)].cell_width(), 3);
        assert_eq!(before[(11, 4)].symbol(), "☀\u{fe0f}");
        assert_eq!(before[(11, 4)].cell_width(), 2);

        let mut frame = terminal.get_frame();
        // Left x10 is two cells after the width-three lead at x8. Right x12
        // cuts the VS16 span at x11. Neither a one-cell halo nor only clearing
        // outside leads can satisfy this literal five-cell eviction.
        clear(&mut frame, Rect::new(10, 4, 2, 1));
        let after = frame.buffer_mut();
        for y in 2..7 {
            for x in 3..23 {
                if y == 4 && (8..13).contains(&x) {
                    assert_eq!(after[(x, y)], Cell::default(), "evicted ({x},{y})");
                } else {
                    assert_eq!(
                        after[(x, y)],
                        before[(x, y)],
                        "disjoint cell/style ({x},{y})"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn clear_keeps_exact_touching_combining_and_wide_spans() -> anyhow::Result<()> {
        let mut terminal = fixed_terminal(Rect::new(0, 0, 12, 3))?;
        let mut frame = terminal.get_frame();
        let area = frame.area();
        frame
            .buffer_mut()
            .set_style(area, Style::default().bg(Color::Magenta));
        frame.render_widget(
            Paragraph::new("ｶ\u{3099}ｶ\u{309a}界").style(Style::default().fg(Color::Green)),
            Rect::new(0, 1, 12, 1),
        );
        let before = frame.buffer_mut().clone();
        assert_eq!(before[(0, 1)].symbol(), "ｶ\u{3099}");
        assert_eq!(before[(0, 1)].cell_width(), 1);
        assert_eq!(before[(1, 1)].symbol(), "ｶ\u{309a}");
        assert_eq!(before[(1, 1)].cell_width(), 1);
        assert_eq!(before[(2, 1)].symbol(), "界");
        assert_eq!(before[(2, 1)].cell_width(), 2);
        clear(&mut frame, Rect::new(1, 1, 1, 1));
        let after = frame.buffer_mut();
        for y in 0..3 {
            for x in 0..12 {
                if (x, y) == (1, 1) {
                    assert_eq!(after[(x, y)], Cell::default());
                } else {
                    assert_eq!(
                        after[(x, y)],
                        before[(x, y)],
                        "exact-touch/disjoint ({x},{y})"
                    );
                }
            }
        }
        Ok(())
    }

    #[test]
    fn clear_handles_long_spans_from_either_edge_without_a_fixed_halo() -> anyhow::Result<()> {
        for (label, popup) in [
            ("lead six cells left", Rect::new(10, 4, 1, 1)),
            ("tail seven cells right", Rect::new(4, 4, 1, 1)),
        ] {
            let mut terminal = fixed_terminal(Rect::new(3, 2, 20, 5))?;
            let mut frame = terminal.get_frame();
            let area = frame.area();
            frame
                .buffer_mut()
                .set_style(area, Style::default().bg(Color::Blue));
            frame.render_widget(
                Paragraph::new("あﾞﾞﾞﾞﾞﾞ").style(Style::default().fg(Color::Cyan)),
                Rect::new(4, 4, 18, 1),
            );
            let before = frame.buffer_mut().clone();
            assert_eq!(before[(4, 4)].symbol(), "あﾞﾞﾞﾞﾞﾞ", "{label}");
            assert_eq!(before[(4, 4)].cell_width(), 8, "{label}");
            clear(&mut frame, popup);
            let after = frame.buffer_mut();
            for y in 2..7 {
                for x in 3..23 {
                    if y == 4 && (4..12).contains(&x) {
                        assert_eq!(after[(x, y)], Cell::default(), "{label}: evicted ({x},{y})");
                    } else {
                        assert_eq!(after[(x, y)], before[(x, y)], "{label}: outside ({x},{y})");
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    fn clear_clips_offset_rectangles_and_leaves_empty_intersections_unchanged() -> anyhow::Result<()>
    {
        let (mut terminal, before) = styled_mixed_frame()?;
        let mut frame = terminal.get_frame();
        for empty in [
            Rect::new(3, 2, 0, 5),
            Rect::new(3, 2, 20, 0),
            Rect::new(0, 0, 3, 2),
            Rect::new(23, 2, 10, 5),
            Rect::new(3, 7, 20, 4),
        ] {
            clear(&mut frame, empty);
            assert_eq!(
                &*frame.buffer_mut(),
                &before,
                "empty intersection {empty:?}"
            );
        }
        clear(&mut frame, Rect::new(0, 0, 8, 5));
        let after = frame.buffer_mut();
        for y in 2..7 {
            for x in 3..23 {
                if y < 5 && x < 8 {
                    assert_eq!(after[(x, y)], Cell::default(), "clipped ({x},{y})");
                } else {
                    assert_eq!(after[(x, y)], before[(x, y)], "outside clip ({x},{y})");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn clear_bounds_overwide_and_zero_width_cells_to_their_row() -> anyhow::Result<()> {
        let mut terminal = fixed_terminal(Rect::new(65_526, 65_533, 9, 2))?;
        let mut frame = terminal.get_frame();
        let area = frame.area();
        frame
            .buffer_mut()
            .set_style(area, Style::default().bg(Color::Blue));
        // Adversarial low-level cells exercise arithmetic/row bounds, not a
        // claim that Paragraph paints a partially fitting grapheme.
        frame.buffer_mut()[(65_530, 65_533)].set_symbol("あﾞﾞﾞﾞﾞﾞ");
        frame.buffer_mut()[(65_526, 65_534)].set_symbol("\u{301}");
        let before = frame.buffer_mut().clone();
        assert_eq!(before[(65_530, 65_533)].cell_width(), 8);
        assert_eq!(before[(65_526, 65_534)].cell_width(), 0);
        clear(&mut frame, Rect::new(65_533, 65_533, 1, 1));
        clear(&mut frame, Rect::new(65_526, 65_534, 1, 1));
        let after = frame.buffer_mut();
        for y in 65_533..65_535 {
            for x in 65_526..65_535 {
                if (y == 65_533 && x >= 65_530) || (x, y) == (65_526, 65_534) {
                    assert_eq!(
                        after[(x, y)],
                        Cell::default(),
                        "row-bounded reset ({x},{y})"
                    );
                } else {
                    assert_eq!(after[(x, y)], before[(x, y)], "other row/cell ({x},{y})");
                }
            }
        }
        Ok(())
    }

    fn paint_transition(frame: &mut Frame<'_>, popups: &[Rect]) {
        frame.render_widget(
            Paragraph::new(
                "1234567あﾞXYZ\n1234567あﾞXYZ\n1234567あﾞXYZ\n1234567あﾞXYZ\n\
                 1234567あﾞXYZ\n1234567あﾞXYZ\n1234567あﾞXYZ\n1234567あﾞXYZ",
            ),
            frame.area(),
        );
        for &popup in popups {
            clear(frame, popup);
            frame.render_widget(Block::default().borders(Borders::ALL), popup);
        }
    }

    #[test]
    fn clear_empty_buffers_are_unchanged() -> anyhow::Result<()> {
        for bounds in [Rect::new(3, 2, 0, 5), Rect::new(3, 2, 20, 0)] {
            let mut terminal = fixed_terminal(bounds)?;
            let mut frame = terminal.get_frame();
            let before = frame.buffer_mut().clone();
            clear(&mut frame, Rect::new(0, 0, 100, 100));
            assert_eq!(&*frame.buffer_mut(), &before, "empty buffer {bounds:?}");
        }
        Ok(())
    }

    #[test]
    fn clear_move_shrink_stack_and_close_reach_the_persistent_backend() -> anyhow::Result<()> {
        let mut terminal = Terminal::new(TestBackend::new(24, 8))?;
        let mut previous = terminal
            .draw(|frame| paint_transition(frame, &[]))?
            .buffer
            .clone();
        assert_eq!(previous[(7, 2)].symbol(), "あﾞ");
        assert_eq!(previous[(7, 2)].cell_width(), 3);
        let open = [Rect::new(8, 1, 8, 5)];
        let moved = [Rect::new(9, 1, 6, 5)];
        let shrunk = [Rect::new(11, 1, 3, 3)];
        let stacked = [open[0], Rect::new(6, 2, 3, 3)];
        for (label, popups, (x, y, symbol)) in [
            ("open", open.as_slice(), (8, 2, "│")),
            ("move", moved.as_slice(), (9, 2, "│")),
            ("shrink", shrunk.as_slice(), (7, 2, "あﾞ")),
            ("stack", stacked.as_slice(), (6, 3, "│")),
            ("close", &[], (7, 2, "あﾞ")),
        ] {
            let current = terminal
                .draw(|frame| paint_transition(frame, popups))?
                .buffer
                .clone();
            assert!(
                previous.diff_iter(&current).any(|(column, row, cell)| {
                    (column, row) == (x, y) && cell.symbol() == symbol
                }),
                "{label}: required visible transition ({x},{y},{symbol:?}) omitted"
            );
            assert_eq!(
                terminal.backend().buffer()[(x, y)].symbol(),
                symbol,
                "{label}"
            );
            if label == "stack" {
                assert_eq!(
                    terminal.backend().buffer()[(15, 3)].symbol(),
                    "│",
                    "lower overlay"
                );
            }
            previous = current;
        }
        Ok(())
    }

    /// The popup geometry inlined in picker.rs before cyril-cc5e,
    /// transcribed verbatim as the parity oracle (claim C8).
    fn legacy_picker_geometry(area: Rect, desired_width: u16, desired_height: u16) -> Rect {
        let width = desired_width.min(area.width.saturating_sub(4));
        let height = desired_height.min(area.height.saturating_sub(4));
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;
        Rect::new(x, y, width, height)
    }

    #[test]
    fn centered_parity_sweep() {
        // C8: byte-identical to the legacy inline arithmetic across the
        // full sweep, including offset (non-zero x/y) parent areas.
        let dims = [0u16, 1, 5, 8, 16, 20, 24, 50, 59, 60, 61, 80, 200];
        let desired = [0u16, 5, 12, 15, 21, 56, 80, 200];
        for &aw in &dims {
            for &ah in &dims {
                for &dw in &desired {
                    for &dh in &desired {
                        for area in [Rect::new(0, 0, aw, ah), Rect::new(3, 2, aw, ah)] {
                            assert_eq!(
                                centered(area, dw, dh),
                                legacy_picker_geometry(area, dw, dh),
                                "parity broke at area={area:?} desired={dw}x{dh}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn degenerate_areas_yield_empty_rects() {
        // C10 (arithmetic part): saturation, never a panic.
        assert_eq!(
            centered(Rect::new(0, 0, 0, 0), 80, 21),
            Rect::new(0, 0, 0, 0)
        );
        let tiny = centered(Rect::new(0, 0, 5, 5), 80, 21);
        assert_eq!((tiny.width, tiny.height), (1, 1));
        assert_eq!((tiny.x, tiny.y), (2, 2));
    }

    #[test]
    fn clamps_desired_size_to_area_margin() {
        // 60x16 floor: an 80-wide request clamps to 56, 21-tall to 12.
        let popup = centered(Rect::new(0, 0, 60, 16), 80, 21);
        assert_eq!(popup, Rect::new(2, 2, 56, 12));
    }

    /// cyril-a14l C9: wherever the legacy centered rect would not overlap
    /// the input, `place` returns it byte-identically; wherever it would,
    /// `place` stays inside rows [area.y+1, input_top). A buggy
    /// always-anchor-above implementation fails the parity half; a buggy
    /// still-centered implementation fails the containment half.
    #[test]
    fn place_parity_and_containment_sweep() {
        let dims = [3u16, 5, 8, 16, 20, 24, 50, 60, 61, 80, 100, 200];
        let desired = [0u16, 5, 9, 12, 13, 21, 56, 80];
        for &aw in &dims {
            for &ah in &dims {
                let area = Rect::new(0, 0, aw, ah);
                for &dw in &desired {
                    for &dh in &desired {
                        for input_top in [0u16, 1, 2, 3, ah / 2, ah.saturating_sub(2), ah] {
                            let legacy = centered(area, dw, dh);
                            let placed = place(area, input_top, dw, dh);
                            if legacy.area() > 0 && legacy.bottom() <= input_top {
                                assert_eq!(
                                    placed,
                                    Some(legacy),
                                    "parity broke: area={area:?} desired={dw}x{dh} input_top={input_top}"
                                );
                            } else if let Some(placed) = placed {
                                assert!(
                                    placed.y >= 1 && placed.bottom() <= input_top,
                                    "containment broke: {placed:?} input_top={input_top}"
                                );
                                assert_eq!(placed.width, legacy.width, "width clamp drifted");
                            }
                        }
                    }
                }
            }
        }
    }

    /// cyril-a14l C9 stress: regions that cannot hold content yield `None`
    /// (callers render nothing), never a panic or a popup that bleeds into
    /// the toolbar or input rows.
    #[test]
    fn place_degenerate_regions_yield_none() {
        // input_top at/above the region start: no rows available.
        for input_top in [0u16, 1] {
            let placed = place(Rect::new(0, 0, 60, 16), input_top, 56, 9);
            assert_eq!(placed, None, "input_top={input_top}");
        }
        // Unclampable width.
        assert_eq!(place(Rect::new(0, 0, 3, 16), 10, 56, 9), None);
        // Region of exactly one row: popup shrinks to it, anchored above input.
        let one_row = place(Rect::new(0, 0, 60, 16), 2, 56, 9);
        assert_eq!(one_row, Some(Rect::new(2, 1, 56, 1)));
        // Max-draft corner from the probe: input_top=4 leaves rows 1-3.
        let tight = place(Rect::new(0, 0, 60, 16), 4, 56, 9);
        assert_eq!(tight, Some(Rect::new(2, 1, 56, 3)));
    }

    #[test]
    fn odd_remainder_lands_on_trailing_margin() {
        // area width 61, popup 56: margins are 2 (left) and 3 (right).
        let popup = centered(Rect::new(0, 0, 61, 20), 56, 10);
        assert_eq!(popup.x, 2);
        assert_eq!(61 - (popup.x + popup.width), 3);
        // vertical: 20 - 10 = 10, even split 5/5
        assert_eq!(popup.y, 5);
    }
}
