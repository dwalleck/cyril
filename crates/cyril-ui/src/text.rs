//! Shared text utilities for display-width-aware truncation and padding.
//!
//! These operate on terminal display columns (not byte count or char count),
//! correctly handling CJK wide characters and other multi-column glyphs.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Truncate a string to at most `max_width` display columns, appending `…`
/// if truncation occurred.
pub fn truncate(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if s.width() <= max_width {
        return s.to_string();
    }
    let budget = max_width.saturating_sub(1); // reserve 1 cell for `…`
    let mut used: usize = 0;
    let mut out = String::new();
    for ch in s.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + ch_width > budget {
            break;
        }
        out.push(ch);
        used += ch_width;
    }
    out.push('…');
    out
}

/// Pad a string with trailing spaces to exactly `width` display columns.
/// If the string is already at or beyond `width`, it is returned unchanged.
pub fn pad_right(s: &str, width: usize) -> String {
    let current = s.width();
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
}
