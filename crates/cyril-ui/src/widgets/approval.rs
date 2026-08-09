use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use crate::theme::Theme;
use crate::traits::{ApprovalPhase, ApprovalState};

/// Render the permission approval overlay.
///
/// `input_top` is the absolute row of the input box's top border; the popup
/// is placed by [`super::modal::place`] so it never covers the input
/// (cyril-a14l C7) and windows its selection when clamped (C8).
pub fn render(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    match state.phase {
        ApprovalPhase::SelectOption => {
            render_option_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::SelectTrust { .. } => {
            render_trust_phase(frame, area, input_top, state, session_attribution, theme)
        }
    }
}

/// Center-scroll window start: keeps `selected` near the middle, clamped so
/// the window never starts before 0 or runs past the end. Mirrors the
/// suggestions widget's windowing so selection behavior feels uniform.
fn window_start(selected: usize, total: usize, visible: usize) -> usize {
    if total > visible && visible > 0 {
        selected.saturating_sub(visible / 2).min(total - visible)
    } else {
        0
    }
}

/// Max body lines in the approval preview (cyril-j1b3). Five matches the
/// chat widget's tool-output cap; the diff renderer applies its own 20-line
/// cap, so a diff-heavy preview is truncated before this budget matters.
const MAX_PREVIEW_LINES: usize = 5;

/// Build the approval-preview block from the joined tool-call snapshot.
///
/// Order: path line (when the tracked call resolves one), then diff content
/// (the existing diff projection, with its own line cap), else the raw-input
/// `text`/`content` payload capped at [`MAX_PREVIEW_LINES`]. A snapshot with
/// no displayable payload yields a single `Preview unavailable` line — the
/// degraded-but-actionable contract from cyril-j1b3's spec: the operator
/// still sees the request's own title/message and every option.
fn preview_lines<'a>(state: &ApprovalState, theme: &Theme) -> Vec<Line<'a>> {
    let tc = &state.tool_call;
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut has_body = false;

    if let Some(path) = tc.primary_path() {
        lines.push(Line::styled(
            format!("  {path}"),
            Style::default().fg(theme.text_secondary),
        ));
    }

    let has_diff = tc
        .content()
        .iter()
        .any(|c| matches!(c, cyril_core::types::ToolCallContent::Diff { .. }));
    if has_diff {
        super::chat::render_diff_lines(&mut lines, tc, theme);
        return lines;
    }

    if let Some(raw) = tc.raw_input() {
        match raw
            .get("text")
            .or_else(|| raw.get("content"))
            .and_then(|v| v.as_str())
        {
            Some(text) => {
                has_body = true;
                let total = text.lines().count();
                for line in text.lines().take(MAX_PREVIEW_LINES) {
                    lines.push(Line::styled(
                        format!("    {line}"),
                        Style::default().fg(theme.subdued),
                    ));
                }
                if total > MAX_PREVIEW_LINES {
                    lines.push(Line::styled(
                        format!("    ...{} more lines", total - MAX_PREVIEW_LINES),
                        Style::default().fg(theme.subdued),
                    ));
                }
            }
            None => {
                // raw_input present but text/content absent or non-string:
                // do not coerce arbitrary values (cyril-j1b3 spec) — log the
                // malformed shape and fall through to the degraded marker.
                tracing::warn!(
                    tool_call_id = %tc.id(),
                    raw_input_shape = %shape_of(raw),
                    "approval preview raw_input has no string text/content field"
                );
            }
        }
    }

    if !has_body {
        lines.push(Line::styled(
            "  Preview unavailable",
            Style::default().fg(theme.subdued),
        ));
    }
    lines
}

/// One-word JSON shape tag for the malformed-raw_input log (never the value
/// itself, which can carry file contents).
fn shape_of(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Object(_) => "object",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::Bool(_) => "bool",
        serde_json::Value::Null => "null",
    }
}

fn render_option_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    // Preview block (cyril-j1b3): the joined tool-call snapshot is rendered
    // above the options so a KAS stub permission request still shows the
    // file path and proposed content. Budget: 1 path line + up to
    // MAX_PREVIEW_LINES of body + blanks; clamped below when space is tight.
    let preview = preview_lines(state, theme);
    // options.len() is a handful of user-facing choices; the sum stays far
    // below u16::MAX, so try_from is infallible and the saturation is
    // defensive, not an error default (same pattern as the picker).
    let desired_height = u16::try_from(
        state
            .options
            .len()
            .saturating_add(preview.len())
            .saturating_add(6),
    )
    .unwrap_or(u16::MAX);
    let Some(popup_area) = super::modal::place(area, input_top, 60, desired_height) else {
        return; // no rows above the input can hold the popup
    };

    frame.render_widget(Clear, popup_area);

    // Inner rows inside the borders decide how much chrome fits: with 2+
    // rows the message keeps its line, with 3+ the blank separator returns,
    // then the preview, and options get the rest (always at least one row —
    // the selection). Preview rows are dropped first under clamping: the
    // dialog must stay actionable even when the preview cannot fit.
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let (show_message, show_blank, preview_rows, option_rows) = match inner {
        0 => (false, false, 0, 0),
        1 => (false, false, 0, 1),
        2 => (true, false, 0, 1),
        3 => (true, true, 0, 1),
        n => {
            let remaining = n - 2; // message + blank accounted
            let preview_rows = preview.len().min(remaining.saturating_sub(1));
            (true, true, preview_rows, remaining - preview_rows)
        }
    };

    let mut lines: Vec<Line> = Vec::new();
    if show_message {
        lines.push(Line::styled(
            &state.message,
            Style::default().fg(theme.emphasis),
        ));
    }
    if show_blank {
        lines.push(Line::default());
    }
    lines.extend(preview.into_iter().take(preview_rows));
    if preview_rows > 0 {
        lines.push(Line::default());
    }

    let visible = state.options.len().min(option_rows);
    let start = window_start(state.selected, state.options.len(), visible);
    for (i, opt) in state.options.iter().enumerate().skip(start).take(visible) {
        let style = if i == state.selected {
            Style::default()
                .bg(theme.selection)
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        let prefix = if i == state.selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{}", opt.label), style));
    }

    let title = match session_attribution {
        Some(session) => format!(" Permission Required — {session} "),
        None => " Permission Required ".to_owned(),
    };
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(theme.emphasis)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.emphasis)),
    );

    frame.render_widget(popup, popup_area);
}

fn render_trust_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    // Each trust option: label line + display line + blank = 3 lines, plus
    // header. Trust tiers are a handful, so try_from is infallible; the
    // saturation is defensive, not an error default.
    let desired_height = u16::try_from(
        state
            .trust_options
            .len()
            .saturating_mul(3)
            .saturating_add(4),
    )
    .unwrap_or(u16::MAX);
    let Some(popup_area) = super::modal::place(area, input_top, 64, desired_height) else {
        return; // no rows above the input can hold the popup
    };

    frame.render_widget(Clear, popup_area);

    // With 5+ inner rows the header and separator fit above one full 3-row
    // item; tighter popups drop the header and window items directly (the
    // selected item's label renders first, so it survives any clamp).
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let (show_header, item_rows) = if inner >= 5 {
        (true, (inner - 2) / 3)
    } else {
        (false, 1)
    };

    let mut lines: Vec<Line> = Vec::new();
    if show_header {
        lines.push(Line::styled(
            "Select trust level:",
            Style::default().fg(theme.accent_quinary),
        ));
        lines.push(Line::default());
    }

    let visible = state.trust_options.len().min(item_rows.max(1));
    let start = window_start(state.selected, state.trust_options.len(), visible);
    for (i, trust) in state
        .trust_options
        .iter()
        .enumerate()
        .skip(start)
        .take(visible)
    {
        let style = if i == state.selected {
            Style::default()
                .bg(theme.selection)
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        let prefix = if i == state.selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{}", trust.label), style));
        // Show the display string (pattern preview) dimmed below the label
        let display_style = if i == state.selected {
            Style::default().bg(theme.selection).fg(theme.subdued)
        } else {
            Style::default().fg(theme.subdued)
        };
        lines.push(Line::styled(
            format!("    {}", trust.display),
            display_style,
        ));
        // Blank separator between options — matches the 3-lines-per-option
        // height reserved above (label + display + blank).
        lines.push(Line::default());
    }

    let title = match session_attribution {
        Some(session) => format!(" Always Allow — Choose Scope — {session} "),
        None => " Always Allow — Choose Scope ".to_owned(),
    };
    let popup = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(Span::styled(
                title,
                Style::default()
                    .fg(theme.accent_quinary)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );

    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn option(id: &str, label: &str) -> cyril_core::types::PermissionOption {
        cyril_core::types::PermissionOption {
            id: cyril_core::types::PermissionOptionId::new(id),
            label: label.into(),
            kind: cyril_core::types::PermissionOptionKind::AllowOnce,
            is_destructive: false,
        }
    }

    fn approval_with(
        options: Vec<cyril_core::types::PermissionOption>,
        trust_options: Vec<cyril_core::types::TrustOption>,
        selected: usize,
        phase: ApprovalPhase,
    ) -> ApprovalState {
        ApprovalState {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: crate::traits::TrackedToolCall::new(cyril_core::types::ToolCall::new(
                cyril_core::types::ToolCallId::new("tc_1"),
                "echo hello".into(),
                cyril_core::types::ToolKind::Execute,
                cyril_core::types::ToolCallStatus::Pending,
                None,
            )),
            message: "Allow execution?".into(),
            options,
            trust_options,
            selected,
            phase,
            responder: tokio::sync::oneshot::channel().0,
        }
    }

    fn theme() -> Theme {
        crate::theme::resolve(
            crate::theme::ThemeId::CyrilDark,
            crate::theme::ColorMode::TrueColor,
        )
    }

    /// Flatten a `TestBackend` buffer into one string per row, joined by `\n`.
    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        let buffer = terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| {
                (0..area.width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn render_at(
        state: &ApprovalState,
        width: u16,
        height: u16,
        input_top: u16,
    ) -> Terminal<TestBackend> {
        render_at_with_attribution(state, width, height, input_top, None)
    }

    fn render_at_with_attribution(
        state: &ApprovalState,
        width: u16,
        height: u16,
        input_top: u16,
        session_attribution: Option<&str>,
    ) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    input_top,
                    state,
                    session_attribution,
                    &theme(),
                );
            })
            .expect("draw");
        terminal
    }

    #[test]
    fn approval_renders() {
        let state = approval_with(
            vec![option("allow", "Allow Once"), option("reject", "Reject")],
            vec![],
            0,
            ApprovalPhase::SelectOption,
        );
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(text.contains("Allow Once"));
        assert!(text.contains("▸ Allow Once"));
    }

    fn trust_option(label: &str, display: &str) -> cyril_core::types::TrustOption {
        cyril_core::types::TrustOption {
            label: label.into(),
            display: display.into(),
            setting_key: "allowedCommands".into(),
            patterns: vec![display.into()],
        }
    }

    #[test]
    fn trust_phase_renders_each_tier_label_and_display() {
        let state = approval_with(
            vec![option("always", "Always Allow")],
            vec![
                trust_option("Full command", "echo hello"),
                trust_option("Base command", "echo *"),
            ],
            1,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Full command"),
            "missing tier 0 label:\n{text}"
        );
        assert!(
            text.contains("Base command"),
            "missing tier 1 label:\n{text}"
        );
        assert!(
            text.contains("echo hello"),
            "missing tier 0 display:\n{text}"
        );
        assert!(text.contains('▸'), "missing selection marker:\n{text}");
    }

    /// cyril-a14l C8: with the popup clamped to a 5-row region above the
    /// input (inner = 3 → message + blank + ONE option row), the selected
    /// LAST option is the one shown. Pre-a14l code rendered options 0..n in
    /// order and clipped the bottom — the ▸ row vanished exactly like probe
    /// S4 showed.
    #[test]
    fn approval_selection_visible_when_clamped() {
        let state = approval_with(
            vec![option("y", "Yes"), option("a", "Always"), option("n", "No")],
            vec![],
            2,
            ApprovalPhase::SelectOption,
        );
        // input_top=6 → region rows 1-5 → popup h=5, inner=3.
        let terminal = render_at(&state, 60, 16, 6);
        let text = buffer_text(&terminal);
        assert!(text.contains("▸ No"), "selected option missing:\n{text}");
        assert!(
            text.contains("Allow execution?"),
            "message dropped:\n{text}"
        );
        // Nothing may render at or below the input row.
        for row in text.lines().skip(6) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    /// cyril-a14l C8 (trust phase): 3-row items window around the selected
    /// LAST item when the region holds one item.
    #[test]
    fn trust_selection_visible_when_clamped() {
        let state = approval_with(
            vec![option("always", "Always Allow")],
            vec![
                trust_option("Full command", "echo hello"),
                trust_option("Base command", "echo *"),
                trust_option("Any command", "*"),
            ],
            2,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        // input_top=8 → region rows 1-7 → popup h=7, inner=5 → header + 1 item.
        let terminal = render_at(&state, 60, 16, 8);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("▸ Any command"),
            "selected item missing:\n{text}"
        );
        for row in text.lines().skip(8) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    #[test]
    fn renders_foreign_session_in_both_phases_without_displacing_selection() {
        let option_state = approval_with(
            vec![option("y", "Yes"), option("n", "No")],
            vec![],
            1,
            ApprovalPhase::SelectOption,
        );
        let option_terminal = render_at_with_attribution(&option_state, 80, 24, 24, Some("peer-α"));
        let option_text = buffer_text(&option_terminal);
        assert!(option_text.contains("Permission Required — peer-α"));

        let trust_state = approval_with(
            vec![option("always", "Always Allow")],
            vec![trust_option("Full command", "echo hello")],
            0,
            ApprovalPhase::SelectTrust {
                chosen_option_id: cyril_core::types::PermissionOptionId::new("always"),
            },
        );
        let trust_terminal = render_at_with_attribution(&trust_state, 80, 24, 24, Some("peer-α"));
        let trust_text = buffer_text(&trust_terminal);
        assert!(trust_text.contains("Always Allow — Choose Scope — peer-α"));

        let long_origin = "x".repeat(256);
        let option_clamped =
            render_at_with_attribution(&option_state, 24, 16, 4, Some(&long_origin));
        assert!(buffer_text(&option_clamped).contains("▸ No"));
        let trust_clamped = render_at_with_attribution(&trust_state, 24, 16, 4, Some(&long_origin));
        assert!(buffer_text(&trust_clamped).contains("▸ Full command"));
    }

    /// place() empty-rect contract: no region → nothing rendered, no panic.
    #[test]
    fn empty_region_renders_nothing() {
        let state = approval_with(
            vec![option("y", "Yes")],
            vec![],
            0,
            ApprovalPhase::SelectOption,
        );
        let terminal = render_at(&state, 60, 16, 1);
        let text = buffer_text(&terminal);
        assert_eq!(text.trim(), "", "expected empty frame:\n{text}");
    }

    /// One option in a 3-row region (inner = 1): the selection alone renders.
    #[test]
    fn single_row_popup_shows_selection_only() {
        let state = approval_with(
            vec![option("y", "Yes"), option("n", "No")],
            vec![],
            1,
            ApprovalPhase::SelectOption,
        );
        // input_top=4 → region rows 1-3 → popup h=3, inner=1.
        let terminal = render_at(&state, 60, 16, 4);
        let text = buffer_text(&terminal);
        assert!(text.contains("▸ No"), "selection missing:\n{text}");
        assert!(
            !text.contains("Allow execution?"),
            "message should be dropped"
        );
    }

    // ---------- cyril-j1b3: joined approval preview ----------

    use cyril_core::types::{ToolCall, ToolCallContent, ToolCallStatus, ToolKind};

    fn approval_for_tool_call(tc: ToolCall) -> ApprovalState {
        ApprovalState {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: crate::traits::TrackedToolCall::new(tc),
            message: "Write File".into(),
            options: vec![option("accept", "Allow"), option("reject", "Deny")],
            trust_options: vec![],
            selected: 0,
            phase: ApprovalPhase::SelectOption,
            responder: tokio::sync::oneshot::channel().0,
        }
    }

    /// C3 fence: a KAS Write File snapshot (rawInput {path, text}, no diff
    /// content yet) renders the path and the proposed text above the options.
    #[test]
    fn renders_joined_preview() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_x"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({
                "path": "/tmp/specs/bug fix.md",
                "text": "# Héllo\nsecond line\nthird line"
            })),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("/tmp/specs/bug fix.md"),
            "path missing (unicode/spaces fixture):\n{text}"
        );
        assert!(text.contains("# Héllo"), "first text line missing:\n{text}");
        assert!(
            text.contains("third line"),
            "last text line missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    /// C3 bounds fence: 6-line raw-input text shows exactly 5 lines plus the
    /// omission marker; a 21-line diff is truncated by the diff renderer's
    /// own 20-line cap. Both must leave the options visible.
    #[test]
    fn bounds_preview() {
        let six_lines = (1..=6)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_text"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "/tmp/a.md", "text": six_lines})),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 40, 40);
        let text = buffer_text(&terminal);
        assert!(text.contains("line 5"), "fifth line missing:\n{text}");
        assert!(
            !text.contains("line 6"),
            "sixth line must be capped:\n{text}"
        );
        assert!(
            text.contains("...1 more lines"),
            "omission marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");

        let big_diff = (1..=21)
            .map(|i| format!("new line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_diff"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Completed,
            None,
        )
        .with_content(vec![ToolCallContent::Diff {
            path: "/tmp/b.md".into(),
            old_text: None,
            new_text: big_diff,
        }]);
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 60, 60);
        let text = buffer_text(&terminal);
        assert!(text.contains("/tmp/b.md"), "diff path missing:\n{text}");
        assert!(
            text.contains("..."),
            "diff truncation marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    /// C4 fence: a stub with no path, no raw input, and no content renders
    /// the degraded marker while every option stays actionable.
    #[test]
    fn degraded_preview_keeps_options() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_stub"),
            "Write File".into(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            None,
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Preview unavailable"),
            "degraded marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
        assert!(text.contains("Deny"), "second option must stay:\n{text}");
    }

    /// C4 malformed-payload fence: raw_input present but `text` non-string —
    /// the path still renders (it is valid) but the degraded marker must
    /// appear instead of coercing the value.
    #[test]
    fn malformed_raw_input_shows_marker_beside_path() {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_bad"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": "/tmp/ok.md", "text": 42})),
        );
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 80, 24, 24);
        let text = buffer_text(&terminal);
        assert!(text.contains("/tmp/ok.md"), "path missing:\n{text}");
        assert!(
            text.contains("Preview unavailable"),
            "malformed payload must show the marker:\n{text}"
        );
        assert!(
            !text.contains("42"),
            "non-string text must not be coerced:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }
}
