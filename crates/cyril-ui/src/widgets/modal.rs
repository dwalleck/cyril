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
