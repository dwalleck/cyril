//! Shared text utilities for display-width-aware truncation and padding.
//!
//! These operate on Ratatui's painted cell columns (not byte or char counts),
//! including the physical columns occupied by halfwidth sound marks.

use ratatui::buffer::CellWidth;
use ratatui::style::Style;
use ratatui::text::{Line, Span};

/// Painted columns of a string, accumulated without narrowing a long line to `u16`.
pub(crate) fn width(text: &str) -> usize {
    Span::raw(text)
        .styled_graphemes(Style::default())
        .map(|grapheme| usize::from(grapheme.symbol.cell_width()))
        .sum()
}

/// Painted columns of a line, retaining its separately painted span boundaries.
pub(crate) fn line_width(line: &Line<'_>) -> usize {
    line.spans.iter().map(|span| width(&span.content)).sum()
}

/// Painted columns of one scalar, for the existing char-wrapping editors.
pub(crate) fn char_width(character: char) -> usize {
    if character.is_control() {
        return 0;
    }
    let mut encoded = [0; 4];
    usize::from(character.encode_utf8(&mut encoded).cell_width())
}

/// Truncate a string to at most `max_width` display columns, appending `…`
/// if truncation occurred.
pub fn truncate(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if width(s) <= max_width {
        return s.to_string();
    }
    let budget = max_width.saturating_sub(1); // reserve 1 cell for `…`
    let mut used: usize = 0;
    let span = Span::raw(s);
    let mut end = s.len();
    for grapheme in span.styled_graphemes(Style::default()) {
        let columns = usize::from(grapheme.symbol.cell_width());
        if used + columns > budget {
            // styled_graphemes borrows subslices of this span's original text.
            end = grapheme.symbol.as_ptr() as usize - s.as_ptr() as usize;
            break;
        }
        used += columns;
    }
    let mut out = String::with_capacity(end + '…'.len_utf8());
    out.push_str(&s[..end]);
    out.push('…');
    out
}

/// Pad a string with trailing spaces to exactly `width` display columns.
/// If the string is already at or beyond `width`, it is returned unchanged.
pub fn pad_right(s: &str, width: usize) -> String {
    let current = self::width(s);
    if current >= width {
        return s.to_string();
    }
    let padding = width - current;
    let mut out = String::with_capacity(s.len() + padding);
    out.push_str(s);
    for _ in 0..padding {
        out.push(' ');
    }
    out
}

/// Truncate to at most `width` display columns, then pad to exactly `width`.
/// The result is always exactly `width` display columns wide.
pub fn truncate_and_pad(s: &str, width: usize) -> String {
    let trunc = truncate(s, width);
    pad_right(&trunc, width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn truncate_preserves_short_strings() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_shortens_with_ellipsis() {
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn truncate_uses_display_width_for_cjk() {
        let result = truncate("日本語テスト", 3);
        assert_eq!(result, "日…");
        assert_eq!(result.width(), 3);
    }

    #[test]
    fn truncate_handles_exact_display_width() {
        assert_eq!(truncate("abc", 3), "abc");
        assert_eq!(truncate("日", 2), "日");
    }

    #[test]
    fn truncate_max_zero_returns_empty() {
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn truncate_max_one_returns_ellipsis() {
        assert_eq!(truncate("hello", 1), "…");
    }

    #[test]
    fn truncate_cjk_boundary() {
        // "AB中" where 中 is 2 cols wide, max_width=3 → "AB…" (not "AB中" which is 4 cols)
        assert_eq!(truncate("AB中", 3), "AB…");
    }

    #[test]
    fn pad_right_adds_spaces() {
        assert_eq!(pad_right("abc", 6), "abc   ");
    }

    #[test]
    fn pad_right_handles_cjk() {
        let padded = pad_right("日本", 6);
        assert_eq!(padded.width(), 6);
        assert_eq!(padded, "日本  ");
    }

    #[test]
    fn pad_right_noop_at_width() {
        assert_eq!(pad_right("abc", 3), "abc");
    }

    #[test]
    fn pad_right_noop_beyond_width() {
        assert_eq!(pad_right("abcdef", 3), "abcdef");
    }

    #[test]
    fn truncate_and_pad_exact_width() {
        assert_eq!(truncate_and_pad("abc", 10).width(), 10);
        let result = truncate_and_pad("日本語テスト", 8);
        assert_eq!(result.width(), 8);
        assert!(result.contains('…'));
    }

    #[test]
    fn truncate_and_pad_short_input() {
        let result = truncate_and_pad("hi", 10);
        assert_eq!(result, "hi        ");
        assert_eq!(result.width(), 10);
    }

    #[test]
    fn halfwidth_truncation_and_padding_keep_whole_graphemes() {
        for (input, columns, expected) in [
            ("ｶﾞZ", 3, "ｶﾞZ"),
            ("ﾊﾟZ", 3, "ﾊﾟZ"),
            ("ｶﾞZ", 2, "… "),
            ("ﾊﾟZ", 2, "… "),
            ("ﾞZ", 2, "ﾞZ"),
            ("ﾟZ", 2, "ﾟZ"),
            ("ｶ\u{3099}Z", 2, "ｶ\u{3099}Z"),
            ("ﾊ\u{309A}Z", 2, "ﾊ\u{309A}Z"),
            ("ｶ\u{3099}ZZ", 2, "ｶ\u{3099}…"),
        ] {
            assert_eq!(truncate_and_pad(input, columns), expected, "{input:?}");
        }
    }

    #[test]
    fn padded_halfwidth_text_keeps_the_following_column_visible_in_diff() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;

        for (input, columns) in [("ｶﾞ", 2), ("ﾊﾟ", 2), ("ｶ\u{3099}", 2)] {
            let padded = truncate_and_pad(input, columns);
            let area = Rect::new(0, 0, 3, 1);
            let previous = Buffer::empty(area);
            let mut painted = previous.clone();
            painted.set_string(0, 0, format!("{padded}Z"), Style::default());
            assert_eq!(painted[(2, 0)].symbol(), "Z", "{input:?}");
            assert!(
                previous
                    .diff(&painted)
                    .iter()
                    .any(|(x, y, cell)| (*x, *y) == (2, 0) && cell.symbol() == "Z"),
                "following glyph omitted from diff for {input:?}"
            );
        }
    }

    #[test]
    fn long_halfwidth_line_does_not_become_a_short_fitting_line() {
        let input = format!("{}Z", "ｶﾞ".repeat(40_000));
        assert_eq!(truncate(&input, 4), "ｶﾞ…");
        assert_eq!(pad_right(&input, 3), input);
    }
}
