use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::traits::HooksPanelState;

const TRIGGER_COL: usize = 18;
const MATCHER_COL: usize = 18;
const MIN_COMMAND_COL: usize = 20;
// Two inner columns of padding (2 + 2) + one trailing gap (2)
const PADDING: usize = 6;

/// Render the hooks panel overlay (centered popup).
///
/// Response shape: `/hooks` command returns `{data: {hooks: HookInfo[]}}`.
/// Each `HookInfo` has `trigger`, `command`, and optional `matcher`.
/// The panel displays them as a three-column table sorted by trigger.
pub fn render(frame: &mut Frame, area: Rect, state: &HooksPanelState) {
    let width = 96.min(area.width.saturating_sub(4));
    // +4 = top border + bottom border + header row + 1 row of margin for
    // the title span (the title sits on the top border row in ratatui, so
    // the "margin" is what keeps the header from sitting directly under it).
    // Cap at 15 data rows before the content starts scrolling.
    let data_rows = state.hooks.len().max(1).min(15) as u16;
    let height = (data_rows + 4).min(area.height.saturating_sub(4));
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let popup_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, popup_area);

    let title = format!(
        " /hooks · {} hook{} ",
        state.hooks.len(),
        if state.hooks.len() == 1 { "" } else { "s" }
    );
    let block = Block::default()
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    if state.hooks.is_empty() {
        let empty = Paragraph::new(Line::styled(
            "  No hooks configured",
            Style::default().fg(Color::DarkGray),
        ))
        .block(block);
        frame.render_widget(empty, popup_area);
        return;
    }

    // Distribute column widths. Fall back to the minimum command column if
    // the terminal is too narrow — we prefer readable commands over padding.
    let inner_width = (width as usize).saturating_sub(2); // minus border
    let command_col = inner_width
        .saturating_sub(TRIGGER_COL + MATCHER_COL + PADDING)
        .max(MIN_COMMAND_COL);

    let header_style = Style::default()
        .fg(Color::DarkGray)
        .add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![
        Span::styled(
            format!("  {}  ", pad_right("Trigger", TRIGGER_COL)),
            header_style,
        ),
        Span::styled(
            format!("{}  ", pad_right("Command", command_col)),
            header_style,
        ),
        Span::styled(pad_right("Matcher", MATCHER_COL), header_style),
    ]));

    // `state.hooks` is stored pre-sorted by UiState::show_hooks_panel, so the
    // widget never re-sorts on render.
    let visible_rows = (height as usize).saturating_sub(4);
    let end = (state.scroll_offset + visible_rows).min(state.hooks.len());
    for hook in state.hooks.iter().take(end).skip(state.scroll_offset) {
        let trigger_cell = truncate_and_pad(&hook.trigger, TRIGGER_COL);
        let command_cell = truncate_and_pad(&hook.command, command_col);
        let matcher_cell = match hook.matcher.as_deref() {
            Some(m) => truncate_and_pad(m, MATCHER_COL),
            None => pad_right("—", MATCHER_COL),
        };
        let matcher_style = if hook.matcher.is_some() {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {trigger_cell}  "),
                Style::default().fg(Color::Rgb(176, 141, 255)),
            ),
            Span::styled(
                format!("{command_cell}  "),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(matcher_cell, matcher_style),
        ]));
    }

    let popup = Paragraph::new(lines).block(block);
    frame.render_widget(popup, popup_area);
}

/// Truncate `s` so its **terminal display width** is at most `max_width`
/// cells, appending `…` when truncation happens.
///
/// Uses `unicode-width` to count display cells rather than Unicode scalar
/// values or bytes — so wide characters (CJK, most emoji) count as 2 cells
/// and narrow characters count as 1. This matters for column alignment:
/// a 3-char CJK trigger like "日本語" occupies 6 cells, not 3.
///
/// Complexity: `O(n)` worst case — the fast path (`s.width() <= max_width`)
/// walks the whole string via `UnicodeWidthStr::width`, and the slow path
/// (truncation needed) walks input chars until the cell budget is exhausted.
/// Both paths are bounded at column-width scale (`max_width` is ~18–52 in
/// practice), so the absolute cost is negligible, but the function is not
/// sub-linear for very long inputs that happen to fit in the budget.
fn truncate(s: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    if s.width() <= max_width {
        return s.to_string();
    }
    // Reserve 1 cell for the ellipsis (`…` is 1 cell wide).
    let budget = max_width.saturating_sub(1);
    let mut out = String::new();
    let mut used: usize = 0;
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

/// Pad `s` with trailing spaces so its display width equals `width` cells.
/// Returns `s` unchanged if it already meets or exceeds `width`.
///
/// Rust's `format!("{:<N}", ...)` spec pads by character count, not cell
/// count, so it miscounts CJK content. This helper uses `UnicodeWidthStr`
/// so columns stay aligned regardless of character script.
fn pad_right(s: &str, width: usize) -> String {
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

/// Truncate to at most `width` cells and then pad to exactly `width` cells.
/// Used by the table renderer to keep every column aligned regardless of
/// content length or character width.
fn truncate_and_pad(s: &str, width: usize) -> String {
    let trunc = truncate(s, width);
    pad_right(&trunc, width)
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use super::*;
    use cyril_core::types::HookInfo;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(state: &HooksPanelState, width: u16, height: u16) -> Terminal<TestBackend> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| render(frame, frame.area(), state))
            .unwrap();
        terminal
    }

    fn rendered_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect()
    }

    /// Find the cell x coordinate where `needle` starts in the rendered
    /// buffer. Only supports ASCII needles — each char must occupy exactly
    /// one cell. Used by the CJK-alignment test to compare the cell
    /// positions of two commands in different rows.
    fn find_ascii_cell_x(terminal: &Terminal<TestBackend>, needle: &str) -> Option<u16> {
        assert!(
            needle.is_ascii(),
            "find_ascii_cell_x only supports ASCII needles"
        );
        let buf = terminal.backend().buffer();
        let area = buf.area();
        let needle_bytes = needle.as_bytes();
        if needle_bytes.is_empty() || area.width < needle_bytes.len() as u16 {
            return None;
        }
        let max_start = area.width - needle_bytes.len() as u16;
        for y in 0..area.height {
            for start_x in 0..=max_start {
                let mut matched = true;
                for (i, &expected) in needle_bytes.iter().enumerate() {
                    let sym = buf[(start_x + i as u16, y)].symbol();
                    if sym.len() != 1 || sym.as_bytes()[0] != expected {
                        matched = false;
                        break;
                    }
                }
                if matched {
                    return Some(start_x);
                }
            }
        }
        None
    }

    #[test]
    fn empty_hooks_renders_placeholder() {
        let state = HooksPanelState {
            hooks: Vec::new(),
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("No hooks configured"));
        assert!(text.contains("0 hooks"));
    }

    #[test]
    fn single_hook_is_singular_in_title() {
        let state = HooksPanelState {
            hooks: vec![HookInfo {
                trigger: "PreToolUse".into(),
                command: "echo pre".into(),
                matcher: Some("read".into()),
            }],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("PreToolUse"));
        assert!(text.contains("echo pre"));
        assert!(text.contains("read"));
        assert!(text.contains("1 hook "));
        assert!(!text.contains("1 hooks"));
    }

    #[test]
    fn multiple_hooks_render_pluralized_in_state_order() {
        // The widget renders `state.hooks` in whatever order it's given —
        // sorting is a `UiState::show_hooks_panel` invariant tested
        // separately in `state.rs`. The input here is already in the order
        // `UiState` would produce (Post < Pre alphabetically), so this test
        // verifies the widget faithfully preserves that order on screen.
        let state = HooksPanelState {
            hooks: vec![
                HookInfo {
                    trigger: "PostToolUse".into(),
                    command: "post".into(),
                    matcher: Some("write".into()),
                },
                HookInfo {
                    trigger: "PreToolUse".into(),
                    command: "pre".into(),
                    matcher: None,
                },
            ],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 24);
        let text = rendered_text(&terminal);
        assert!(text.contains("2 hooks"));
        assert!(text.contains("PostToolUse"));
        assert!(text.contains("PreToolUse"));
        // Em-dash for missing matcher
        assert!(text.contains("—"));
        // `rendered_text` walks the TestBackend cells in row-major order, so
        // a substring that appears earlier in the returned string was rendered
        // on an earlier row.
        let post_pos = text.find("PostToolUse").expect("PostToolUse should render");
        let pre_pos = text.find("PreToolUse").expect("PreToolUse should render");
        assert!(
            post_pos < pre_pos,
            "widget should preserve state.hooks order"
        );
    }

    #[test]
    fn long_command_is_truncated_without_panic() {
        let long = "echo ".to_string() + &"x".repeat(500);
        let state = HooksPanelState {
            hooks: vec![HookInfo {
                trigger: "Stop".into(),
                command: long,
                matcher: None,
            }],
            scroll_offset: 0,
        };
        // Small terminal forces aggressive truncation
        let terminal = draw(&state, 60, 20);
        let text = rendered_text(&terminal);
        // Ellipsis char indicates truncation occurred
        assert!(text.contains("…"));
    }

    #[test]
    fn unicode_trigger_renders_with_truncation_and_command_marker() {
        // CJK trigger overflows the 18-cell Trigger column (10 chars × 2 cells
        // = 20 cells). The widget should truncate the trigger and still render
        // a distinguishable marker in the Command column, so users can see
        // there's a value there even if the trigger is mangled.
        let state = HooksPanelState {
            hooks: vec![HookInfo {
                trigger: "日本語トリガーテスト".into(),
                command: "MARKER".into(),
                matcher: None,
            }],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 20);
        let text = rendered_text(&terminal);
        assert!(text.contains("MARKER"), "command marker should render");
        // Truncation happened — ellipsis indicates the trigger was cut off.
        assert!(text.contains("…"), "long CJK trigger should be truncated");
    }

    #[test]
    fn cjk_trigger_aligns_command_column_with_ascii() {
        // Two hooks, one ASCII trigger and one CJK trigger. Because
        // `pad_right` measures cells (not chars), both should place their
        // Command column at the same cell x coordinate regardless of
        // trigger script. A regression in `truncate_and_pad` or `pad_right`
        // that fell back to char counts would shift the CJK row's command
        // right by several cells — this test catches that.
        let state = HooksPanelState {
            hooks: vec![
                HookInfo {
                    trigger: "Short".into(),
                    command: "FIRST".into(),
                    matcher: None,
                },
                HookInfo {
                    trigger: "日本語テスト".into(),
                    command: "SECOND".into(),
                    matcher: None,
                },
            ],
            scroll_offset: 0,
        };
        let terminal = draw(&state, 100, 20);
        let first_col = find_ascii_cell_x(&terminal, "FIRST").expect("FIRST should render");
        let second_col = find_ascii_cell_x(&terminal, "SECOND").expect("SECOND should render");
        assert_eq!(
            first_col, second_col,
            "ASCII and CJK triggers must place their commands at the same cell \
             column (CJK alignment regression — check pad_right / truncate_and_pad)"
        );
    }

    #[test]
    fn truncate_helper_preserves_short_strings() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_helper_shortens_ascii_with_ellipsis() {
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn truncate_helper_uses_display_width_for_cjk() {
        // Width budget 3 with 1 cell reserved for ellipsis leaves room for
        // exactly one 2-cell CJK char. The old char-count implementation
        // would have returned "日本…" (3 chars, 5 cells wide); the new
        // display-width implementation returns "日…" (2 chars, 3 cells wide).
        let result = truncate("日本語テスト", 3);
        assert_eq!(result, "日…");
        assert_eq!(result.width(), 3, "result should fit the cell budget");
    }

    #[test]
    fn truncate_helper_handles_exact_display_width() {
        // 3 ASCII chars == 3 cells, fits exactly, no truncation.
        assert_eq!(truncate("abc", 3), "abc");
        // 1 CJK char == 2 cells, fits in a 2-cell budget, no truncation.
        assert_eq!(truncate("日", 2), "日");
    }

    #[test]
    fn truncate_helper_max_zero_returns_empty() {
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn pad_right_adds_spaces_to_ascii() {
        assert_eq!(pad_right("abc", 6), "abc   ");
    }

    #[test]
    fn pad_right_pads_cjk_by_cell_width() {
        // "日本" is 2 chars but 4 cells. Padding to 6 cells adds 2 spaces.
        let padded = pad_right("日本", 6);
        assert_eq!(padded.width(), 6);
        assert_eq!(padded, "日本  ");
    }

    #[test]
    fn pad_right_noop_when_already_at_width() {
        assert_eq!(pad_right("abc", 3), "abc");
        assert_eq!(pad_right("日", 2), "日");
    }

    #[test]
    fn pad_right_noop_when_wider_than_width() {
        // Overflow is unchanged — truncation is truncate()'s job.
        assert_eq!(pad_right("abcdef", 3), "abcdef");
    }

    #[test]
    fn truncate_and_pad_fits_exactly_in_cells() {
        // ASCII
        assert_eq!(truncate_and_pad("abc", 10).width(), 10);
        // CJK that overflows: truncate, then pad
        let result = truncate_and_pad("日本語テスト", 8);
        assert_eq!(
            result.width(),
            8,
            "truncate_and_pad output must always equal the requested width"
        );
        assert!(result.contains('…'), "should have been truncated");
    }

    #[test]
    fn truncate_and_pad_short_input_is_padded() {
        let result = truncate_and_pad("hi", 10);
        assert_eq!(result, "hi        ");
        assert_eq!(result.width(), 10);
    }
}
