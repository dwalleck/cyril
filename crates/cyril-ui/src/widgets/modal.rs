use ratatui::prelude::*;

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
    let width = desired_width.min(area.width.saturating_sub(4));
    let height = desired_height.min(area.height.saturating_sub(4));
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    Rect::new(x, y, width, height)
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
/// Returns an empty rect when the region has no rows or the area has no
/// clampable width; callers must skip rendering (including `Clear`) when
/// `rect.area() == 0` — rendering a popup frame into a zero rect would
/// paint nothing meaningful but still wipe cells.
pub fn place(area: Rect, input_top: u16, desired_width: u16, desired_height: u16) -> Rect {
    let legacy = centered(area, desired_width, desired_height);
    if legacy.area() == 0 {
        return Rect::default();
    }
    if legacy.bottom() <= input_top {
        return legacy;
    }
    let region_top = area.y.saturating_add(1);
    let region_height = input_top.saturating_sub(region_top);
    if region_height == 0 {
        return Rect::default();
    }
    let height = legacy.height.min(region_height);
    Rect::new(
        legacy.x,
        input_top.saturating_sub(height),
        legacy.width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

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
                                    placed, legacy,
                                    "parity broke: area={area:?} desired={dw}x{dh} input_top={input_top}"
                                );
                            } else if placed.area() > 0 {
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

    /// cyril-a14l C9 stress: regions that cannot hold content yield the
    /// empty rect (callers skip rendering), never a panic or a popup that
    /// bleeds into the toolbar or input rows.
    #[test]
    fn place_degenerate_regions_yield_empty_rects() {
        // input_top at/above the region start: no rows available.
        for input_top in [0u16, 1] {
            let placed = place(Rect::new(0, 0, 60, 16), input_top, 56, 9);
            assert_eq!(placed.area(), 0, "input_top={input_top}");
        }
        // Unclampable width.
        assert_eq!(place(Rect::new(0, 0, 3, 16), 10, 56, 9).area(), 0);
        // Region of exactly one row: popup shrinks to it, anchored above input.
        let one_row = place(Rect::new(0, 0, 60, 16), 2, 56, 9);
        assert_eq!(one_row, Rect::new(2, 1, 56, 1));
        // Max-draft corner from the probe: input_top=4 leaves rows 1-3.
        let tight = place(Rect::new(0, 0, 60, 16), 4, 56, 9);
        assert_eq!(tight, Rect::new(2, 1, 56, 3));
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
