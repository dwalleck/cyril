use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

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
    // Reserve 4 rows of chrome (borders, title, header, blank line) plus
    // one row per visible hook. Cap at 15 data rows.
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
        Span::styled(format!("  {:<TRIGGER_COL$}  ", "Trigger"), header_style),
        Span::styled(format!("{:<command_col$}  ", "Command"), header_style),
        Span::styled(format!("{:<MATCHER_COL$}", "Matcher"), header_style),
    ]));

    // Sort by trigger then command — matches Kiro's HooksPanel behavior.
    let mut sorted: Vec<&cyril_core::types::HookInfo> = state.hooks.iter().collect();
    sorted.sort_by(|a, b| {
        a.trigger
            .cmp(&b.trigger)
            .then_with(|| a.command.cmp(&b.command))
    });

    let visible_rows = (height as usize).saturating_sub(4);
    let end = (state.scroll_offset + visible_rows).min(sorted.len());
    for hook in sorted.iter().take(end).skip(state.scroll_offset) {
        let trigger_text = truncate(&hook.trigger, TRIGGER_COL);
        let command_text = truncate(&hook.command, command_col);
        let matcher_text = match hook.matcher.as_deref() {
            Some(m) => truncate(m, MATCHER_COL),
            None => "—".into(),
        };
        let matcher_style = if hook.matcher.is_some() {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::styled(
                format!("  {:<TRIGGER_COL$}  ", trigger_text),
                Style::default().fg(Color::Rgb(176, 141, 255)),
            ),
            Span::styled(
                format!("{:<command_col$}  ", command_text),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(format!("{:<MATCHER_COL$}", matcher_text), matcher_style),
        ]));
    }

    let popup = Paragraph::new(lines).block(block);
    frame.render_widget(popup, popup_area);
}

/// Truncate `s` to at most `max_chars` characters, appending `…` when
/// truncation happens. Uses character boundaries to avoid splitting UTF-8.
fn truncate(s: &str, max_chars: usize) -> String {
    let count = s.chars().count();
    if count <= max_chars {
        return s.to_string();
    }
    if max_chars == 0 {
        return String::new();
    }
    let take = max_chars.saturating_sub(1);
    let mut out: String = s.chars().take(take).collect();
    out.push('…');
    out
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
    fn multiple_hooks_render_pluralized_and_sorted() {
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
        // on an earlier row. "Post" < "Pre" alphabetically (P-o vs P-r), so
        // PostToolUse should appear before PreToolUse on screen even though
        // it was inserted second in the input vector.
        let post_pos = text.find("PostToolUse").expect("PostToolUse should render");
        let pre_pos = text.find("PreToolUse").expect("PreToolUse should render");
        assert!(
            post_pos < pre_pos,
            "PostToolUse should sort before PreToolUse"
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
    fn unicode_trigger_is_not_split_on_truncation() {
        let state = HooksPanelState {
            hooks: vec![HookInfo {
                trigger: "日本語トリガーテスト".into(),
                command: "echo ok".into(),
                matcher: None,
            }],
            scroll_offset: 0,
        };
        // Small terminal — trigger will be truncated mid-string
        let terminal = draw(&state, 80, 20);
        // The test is that this doesn't panic and produces some output
        let _ = rendered_text(&terminal);
    }

    #[test]
    fn truncate_helper_preserves_short_strings() {
        assert_eq!(truncate("abc", 10), "abc");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn truncate_helper_shortens_with_ellipsis() {
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }

    #[test]
    fn truncate_helper_handles_unicode_boundary() {
        // 4 chars, each multi-byte. Truncating to 3 should not split bytes.
        let result = truncate("日本語テスト", 3);
        assert_eq!(result.chars().count(), 3);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn truncate_helper_max_zero_returns_empty() {
        assert_eq!(truncate("abc", 0), "");
    }
}
