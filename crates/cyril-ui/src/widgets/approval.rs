use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    match &state.phase {
        ApprovalPhase::SelectOption => {
            render_option_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::SelectTrust { .. } => {
            render_trust_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::EnterReason { .. } => {
            render_reason_phase(frame, area, input_top, state, session_attribution, theme)
        }
    }
}

fn attribution_title<'a>(
    base: &'a str,
    session_attribution: Option<&str>,
    color: Color,
) -> Span<'a> {
    let style = Style::default().fg(color).add_modifier(Modifier::BOLD);
    match session_attribution {
        Some(session) => Span::styled(format!("{base}— {session} "), style),
        None => Span::styled(base, style),
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

/// Desired widths of the option and reject-with-reason popups.
const OPTION_POPUP_WIDTH: u16 = 60;
const REASON_POPUP_WIDTH: u16 = 64;

/// Indent before each path row of the preview.
const PATH_INDENT: &str = "  ";
const ELLIPSIS: &str = "…";
const PATH_SEPARATORS: [char; 2] = ['/', '\\'];

/// The approval preview: the target path above the body built by
/// [`preview_lines`].
///
/// The path is the most important fact on a write approval, so it is laid
/// out first and never clipped at the border (cyril-rcub). When the popup has
/// rows to spare, the path wraps across as many rows as it needs and is
/// readable in full. When the popup is clamped and gives the path fewer rows
/// than that, the path is middle-elided into those rows, always keeping the
/// file name and as much leading context as fits. Wrapping alone would not
/// be enough, because clamping drops preview rows from the bottom, which is
/// where a wrapped path's file name sits.
struct Preview<'a> {
    path: Option<&'a str>,
    /// Columns available to a path row (popup inner width minus the indent).
    path_width: usize,
    /// Rows the whole path needs when wrapped.
    path_rows: usize,
    body: Vec<Line<'a>>,
}

impl<'a> Preview<'a> {
    fn new(state: &'a ApprovalState, theme: &Theme, area: Rect, desired_width: u16) -> Self {
        let path = state.tool_call.primary_path();
        // Borders take two columns. A popup too narrow to show any path
        // column still lays the path out one column per row; ratatui clips it.
        let path_width = usize::from(super::modal::popup_width(area, desired_width))
            .saturating_sub(2 + PATH_INDENT.len())
            .max(1);
        let path_rows = path.map_or(0, |path| wrap_columns(path, path_width).len());
        Self {
            path,
            path_width,
            path_rows,
            body: preview_lines(state, theme),
        }
    }

    /// Rows the whole preview wants.
    fn len(&self) -> usize {
        self.path_rows.saturating_add(self.body.len())
    }

    /// The preview laid out in at most `rows` rows. The path takes rows
    /// first and is fitted to its share; the body fills whatever is left.
    fn into_lines(self, rows: usize, theme: &Theme) -> Vec<Line<'a>> {
        let mut lines: Vec<Line<'a>> = Vec::with_capacity(rows);
        if let Some(path) = self.path {
            for row in fit_path(path, self.path_width, self.path_rows.min(rows)) {
                lines.push(Line::styled(
                    format!("{PATH_INDENT}{row}"),
                    Style::default().fg(theme.text_secondary),
                ));
            }
        }
        let body_rows = rows.saturating_sub(lines.len());
        lines.extend(self.body.into_iter().take(body_rows));
        lines
    }
}

/// Split `text` into rows of at most `width` display columns. A character
/// never splits: a wide glyph that would straddle the edge starts the next
/// row, matching how ratatui draws it (the glyph plus a blank continuation
/// cell).
fn wrap_columns(text: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    let mut row = String::new();
    let mut used = 0usize;
    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used.saturating_add(ch_width) > width && !row.is_empty() {
            rows.push(std::mem::take(&mut row));
            used = 0;
        }
        row.push(ch);
        used = used.saturating_add(ch_width);
    }
    if !row.is_empty() || rows.is_empty() {
        rows.push(row);
    }
    rows
}

/// Lay `path` out in at most `rows` rows of `width` columns. The path is
/// wrapped whole when it fits; otherwise it is middle-elided
/// ([`elide_middle`]) into the space available.
fn fit_path(path: &str, width: usize, rows: usize) -> Vec<String> {
    if rows == 0 {
        return Vec::new();
    }
    let wrapped = wrap_columns(path, width);
    if wrapped.len() <= rows {
        return wrapped;
    }
    // A wide glyph that straddles a row edge wastes that row's last column,
    // so shrink the column budget until the elided path wraps into `rows`.
    // The loop ends: at a budget of zero the path elides to "", one row.
    let mut budget = width.saturating_mul(rows);
    loop {
        let fitted = wrap_columns(&elide_middle(path, budget), width);
        if fitted.len() <= rows || budget == 0 {
            return fitted;
        }
        budget -= 1;
    }
}

/// Shorten `path` to at most `budget` display columns by replacing its
/// middle with `…`. The kept tail is the file name, plus its parent
/// directory when that fits too. Every remaining column goes to the leading
/// context. If the file name alone is wider than the budget, its end (the
/// extension) is what stays.
fn elide_middle(path: &str, budget: usize) -> String {
    if path.width() <= budget {
        return path.to_owned();
    }
    let Some(room) = budget.checked_sub(ELLIPSIS.width()) else {
        return String::new();
    };
    // Separator offsets, ignoring a trailing separator so a directory path
    // still keeps its last component. Separators are ASCII, so every offset
    // is a char boundary.
    let separators: Vec<usize> = path
        .trim_end_matches(&PATH_SEPARATORS[..])
        .match_indices(&PATH_SEPARATORS[..])
        .map(|(offset, _)| offset)
        .collect();
    let name = separators.last().map_or(path, |&offset| &path[offset..]);
    let parent = separators
        .iter()
        .rev()
        .nth(1)
        .map_or(name, |&offset| &path[offset..]);
    let tail = if parent.width() <= room { parent } else { name };
    match room.checked_sub(tail.width()) {
        Some(head_room) => format!("{}{ELLIPSIS}{tail}", leading_columns(path, head_room)),
        None => format!("{ELLIPSIS}{}", trailing_columns(tail, room)),
    }
}

/// The longest prefix of `text` at most `width` display columns wide.
fn leading_columns(text: &str, width: usize) -> &str {
    let mut used = 0usize;
    for (offset, ch) in text.char_indices() {
        used = used.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        if used > width {
            return &text[..offset];
        }
    }
    text
}

/// The longest suffix of `text` at most `width` display columns wide.
fn trailing_columns(text: &str, width: usize) -> &str {
    let mut used = 0usize;
    for (offset, ch) in text.char_indices().rev() {
        used = used.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        if used > width {
            return &text[offset + ch.len_utf8()..];
        }
    }
    text
}

/// Max body lines in the approval preview (cyril-j1b3). Five matches the
/// chat widget's tool-output cap; the diff renderer applies its own 20-line
/// cap, so a diff-heavy preview is truncated before this budget matters.
const MAX_PREVIEW_LINES: usize = 5;

/// Build the approval-preview body from the joined tool-call snapshot. The
/// path rows above it are laid out by [`Preview`] (cyril-rcub).
///
/// Order: diff content
/// (the existing diff projection, with its own line cap), else the raw-input
/// `text`/`content` payload capped at [`MAX_PREVIEW_LINES`]. A snapshot with
/// no displayable payload yields a single `Preview unavailable` line — the
/// degraded-but-actionable contract from cyril-j1b3's spec: the operator
/// still sees the request's own title/message and every option.
fn preview_lines<'a>(state: &ApprovalState, theme: &Theme) -> Vec<Line<'a>> {
    let tc = &state.tool_call;
    let mut lines: Vec<Line<'a>> = Vec::new();
    let mut has_body = false;

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
    // file path and proposed content. Budget: the path's wrapped rows + up
    // to MAX_PREVIEW_LINES of body + blanks; clamped below when space is
    // tight (the path is then middle-elided, cyril-rcub).
    let preview = Preview::new(state, theme, area, OPTION_POPUP_WIDTH);
    let show_feedback_hint = state.can_reject_with_reason();
    // options.len() is a handful of user-facing choices; the sum stays far
    // below u16::MAX, so try_from is infallible and the saturation is
    // defensive, not an error default (same pattern as the picker).
    let desired_height = u16::try_from(
        state
            .options
            .len()
            .saturating_add(preview.len())
            .saturating_add(6)
            .saturating_add(usize::from(show_feedback_hint)),
    )
    .unwrap_or(u16::MAX);
    let Some(popup_area) = super::modal::place(area, input_top, OPTION_POPUP_WIDTH, desired_height)
    else {
        return; // no rows above the input can hold the popup
    };

    frame.render_widget(Clear, popup_area);

    // Inner rows inside the borders decide how much chrome fits: with 2+
    // rows the message keeps its line, with 3+ the blank separator returns,
    // then the preview, and options get the rest (always at least one row —
    // the selection). Preview rows are dropped first under clamping: the
    // dialog must stay actionable even when the preview cannot fit.
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let (show_message, show_blank, preview_rows, mut option_rows) = match inner {
        0 => (false, false, 0, 0),
        1 => (false, false, 0, 1),
        2 => (true, false, 0, 1),
        3 => (true, true, 0, 1),
        n => {
            let remaining = n - 2; // message + blank accounted
            // A shown preview is followed by a blank separator; both must
            // leave the option row (cyril-rcub: the unbudgeted separator
            // used to push the selection out of a clamped popup).
            let preview_rows = preview.len().min(remaining.saturating_sub(2));
            let separator = usize::from(preview_rows > 0);
            (
                true,
                true,
                preview_rows,
                remaining - preview_rows - separator,
            )
        }
    };
    let show_feedback_hint = show_feedback_hint && option_rows > 1;
    if show_feedback_hint {
        option_rows -= 1;
    }

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
    lines.extend(preview.into_lines(preview_rows, theme));
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
    if show_feedback_hint {
        lines.push(Line::styled(
            "r  Reject with reason",
            Style::default().fg(theme.accent_quinary),
        ));
    }

    let title = attribution_title(" Permission Required ", session_attribution, theme.emphasis);
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.emphasis)),
    );
    frame.render_widget(popup, popup_area);
}
fn render_reason_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::EnterReason {
        chosen_option_id,
        editor,
    } = &state.phase
    else {
        return;
    };
    let preview = Preview::new(state, theme, area, REASON_POPUP_WIDTH);
    let selected_label = state
        .options
        .iter()
        .find(|option| option.id == *chosen_option_id)
        .map_or("Reject", |option| option.label.as_str());
    let desired_height = u16::try_from(
        preview
            .len()
            .saturating_add(14)
            .saturating_add(usize::from(editor.notice().is_some())),
    )
    .unwrap_or(u16::MAX);
    let Some(popup_area) = super::modal::place(area, input_top, REASON_POPUP_WIDTH, desired_height)
    else {
        return;
    };

    frame.render_widget(Clear, popup_area);
    let inner = usize::from(popup_area.height.saturating_sub(2));
    if inner == 0 {
        return;
    }

    // Reserve one row for the editor so even a cramped viewport keeps the
    // cursor visible. Context and chrome are admitted only when they leave
    // that row intact; preview rows are the first content to be dropped.
    let show_limit = inner >= 2;
    let show_notice = editor.notice().is_some() && inner >= 3;
    let show_editor_label = inner >= 4;
    let show_tool_title = inner >= 6;
    let show_selected = inner >= 7;
    let show_message = inner >= 8;
    let show_blank = inner >= 9;
    let fixed = 1usize
        .saturating_add(usize::from(show_editor_label))
        .saturating_add(usize::from(show_limit))
        .saturating_add(usize::from(show_notice))
        .saturating_add(usize::from(show_tool_title))
        .saturating_add(usize::from(show_selected))
        .saturating_add(usize::from(show_message))
        .saturating_add(usize::from(show_blank));
    let preview_rows = preview.len().min(inner.saturating_sub(fixed));
    let editor_rows = inner
        .saturating_sub(fixed.saturating_add(preview_rows))
        .max(1);
    let content_width = usize::from(popup_area.width.saturating_sub(2)).max(1);
    let editor_lines = super::input::text_cursor_lines(
        editor.text(),
        editor.cursor(),
        content_width,
        editor_rows,
        theme,
    );

    let mut lines: Vec<Line> = Vec::with_capacity(inner);
    if show_message {
        lines.push(Line::styled(
            &state.message,
            Style::default().fg(theme.emphasis),
        ));
    }
    if show_blank {
        lines.push(Line::default());
    }
    lines.extend(preview.into_lines(preview_rows, theme));
    if show_selected {
        lines.push(Line::styled(
            format!("Rejecting: {selected_label}"),
            Style::default().fg(theme.text_secondary),
        ));
    }
    if show_editor_label {
        lines.push(Line::styled(
            "Reason (Enter submits, Ctrl+J adds a line, Esc backs out):",
            Style::default().fg(theme.accent_quinary),
        ));
    }
    lines.extend(editor_lines);
    if show_tool_title {
        lines.push(Line::styled(
            format!("Request: {}", state.tool_call.title()),
            Style::default().fg(theme.text_secondary),
        ));
    }
    if show_limit {
        lines.push(Line::styled(
            format!(
                "Limit: {} Unicode characters",
                crate::feedback_editor::RejectionFeedback::max_scalars()
            ),
            Style::default().fg(theme.subdued),
        ));
    }
    if show_notice && let Some(notice) = editor.notice() {
        lines.push(Line::styled(notice, Style::default().fg(theme.warning)));
    }

    let title = attribution_title(" Reject with reason ", session_attribution, theme.emphasis);
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
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

    let title = attribution_title(
        " Always Allow — Choose Scope ",
        session_attribution,
        theme.accent_quinary,
    );
    let popup = Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );

    frame.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::UiState;
    use crate::traits::TuiState;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn option(id: &str, label: &str) -> cyril_core::types::PermissionOption {
        cyril_core::types::PermissionOption {
            id: cyril_core::types::PermissionOptionId::new(id),
            label: label.into(),
            kind: if id == "reject" {
                cyril_core::types::PermissionOptionKind::RejectOnce
            } else {
                cyril_core::types::PermissionOptionKind::AllowOnce
            },
            is_destructive: false,
        }
    }

    fn approval_tool_call() -> cyril_core::types::ToolCall {
        cyril_core::types::ToolCall::new(
            cyril_core::types::ToolCallId::new("tc_1"),
            "echo hello".into(),
            cyril_core::types::ToolKind::Execute,
            cyril_core::types::ToolCallStatus::Pending,
            None,
        )
    }

    fn approval_with(
        options: Vec<cyril_core::types::PermissionOption>,
        trust_options: Vec<cyril_core::types::TrustOption>,
        selected: usize,
        phase: ApprovalPhase,
    ) -> ApprovalState {
        ApprovalState {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: crate::traits::TrackedToolCall::new(approval_tool_call()),
            message: "Allow execution?".into(),
            options,
            trust_options,
            selected,
            can_reject_with_reason: false,
            phase,
            responder: tokio::sync::oneshot::channel().0,
        }
    }

    fn approval_ui(options: Vec<cyril_core::types::PermissionOption>) -> UiState {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: approval_tool_call(),
            message: "Allow execution?".into(),
            options,
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
        });
        ui
    }

    fn feedback_ui() -> UiState {
        let mut ui = approval_ui(vec![option("reject", "Reject")]);
        assert!(ui.approval_begin_feedback());
        ui
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

    fn render_ui(ui: &UiState, width: u16, height: u16, input_top: u16) -> Terminal<TestBackend> {
        let Some(approval) = ui.approval() else {
            panic!("expected front approval");
        };
        render_at(approval, width, height, input_top)
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
    fn rejection_feedback_hint_only_for_eligible_option() {
        let mut ui = approval_ui(vec![
            option("allow", "Allow Once"),
            option("reject", "Reject"),
        ]);
        let before = render_ui(&ui, 80, 24, 24);
        assert!(!buffer_text(&before).contains("r  Reject with reason"));
        ui.approval_select_next();
        let terminal = render_ui(&ui, 80, 24, 24);
        assert!(buffer_text(&terminal).contains("r  Reject with reason"));
    }

    #[test]
    fn rejection_feedback_renders_limit_and_request_title() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste(&"界".repeat(4096));
        ui.approval_feedback_paste("x");
        let terminal = render_ui(&ui, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(
            text.contains("Request: echo hello"),
            "title missing:\n{text}"
        );
        assert!(
            text.contains("Limit: 4096 Unicode characters"),
            "limit missing:\n{text}"
        );
        assert!(
            text.contains("Reason limit reached"),
            "limit notice missing:\n{text}"
        );
    }
    #[test]
    fn rejection_feedback_cursor_visible_in_ordinary_popup() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste("ordinary cursor");
        let terminal = render_ui(&ui, 90, 30, 30);
        let text = buffer_text(&terminal);
        assert!(text.contains("ordinary cursor█"), "cursor missing:\n{text}");
    }

    #[test]
    fn rejection_feedback_cursor_visible_when_popup_is_cramped() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste("cramped cursor");
        let terminal = render_ui(&ui, 30, 16, 6);
        let text = buffer_text(&terminal);
        assert!(text.contains("cramped cursor█"), "cursor missing:\n{text}");
        for row in text.lines().skip(6) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{text}");
        }
    }

    #[test]
    fn rejection_feedback_overflow_keeps_limit_cursor_and_warning_visible() {
        let mut ui = feedback_ui();
        ui.approval_feedback_paste(&"界".repeat(4096));
        let before = render_ui(&ui, 30, 16, 6);
        let warning = theme().warning;
        let warning_cells = |terminal: &Terminal<TestBackend>| {
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .filter(|cell| cell.fg == warning && !cell.symbol().trim().is_empty())
                .count()
        };
        assert_eq!(
            warning_cells(&before),
            0,
            "exact-bound draft has no overflow"
        );
        ui.approval_feedback_paste("x");
        let after = render_ui(&ui, 30, 16, 6);
        let text = buffer_text(&after);
        assert!(text.contains("4096"), "limit clipped:\n{text}");
        assert!(text.contains('█'), "cursor clipped:\n{text}");
        assert!(
            warning_cells(&after) > 0,
            "overflow warning clipped:\n{text}"
        );
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

    // ---------- cyril-rcub: long write paths stay identifiable ----------

    /// The live 2.32.0 path the overlay clipped to `…/conductor-spike/a`.
    const LONG_WRITE_PATH: &str = "/home/dwalleck/repos/cyril/experiments/conductor-spike/audit-2.32.0/live/cyril-kas/S4b-write-outside-approval.txt";
    const LONG_WRITE_NAME: &str = "S4b-write-outside-approval.txt";
    /// Wide glyphs (2 columns each) in directories and the file name.
    const WIDE_WRITE_PATH: &str =
        "/home/用户/项目/文档/非常长的目录名称/另一个目录/设计说明书-最终版本.md";
    const WIDE_WRITE_NAME: &str = "设计说明书-最终版本.md";

    fn write_approval(path: &str) -> ApprovalState {
        approval_for_tool_call(ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_rcub"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"path": path, "text": "body"})),
        ))
    }

    /// Screen rows as a reader sees them: a wide glyph's blank continuation
    /// cell is skipped rather than read as a space.
    fn screen_rows(terminal: &Terminal<TestBackend>) -> Vec<String> {
        let buffer = terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| {
                let mut row = String::new();
                let mut x = 0;
                while x < area.width {
                    let symbol = buffer[(x, y)].symbol();
                    row.push_str(symbol);
                    let advance = unicode_width::UnicodeWidthStr::width(symbol).max(1);
                    x = x.saturating_add(u16::try_from(advance).unwrap_or(1));
                }
                row
            })
            .collect()
    }

    /// The text between a popup row's side borders with the preview's
    /// two-column indent removed; `None` for rows outside the popup body.
    fn popup_body(row: &str) -> Option<&str> {
        let start = row.find('│')? + '│'.len_utf8();
        let end = row.rfind('│')?;
        let inner = row.get(start..end)?;
        Some(inner.strip_prefix("  ").unwrap_or(inner).trim_end())
    }

    /// Popup body rows concatenated, so a path wrapped across rows reads back
    /// as one string.
    fn joined_body(terminal: &Terminal<TestBackend>) -> String {
        screen_rows(terminal)
            .iter()
            .filter_map(|row| popup_body(row))
            .collect()
    }

    /// cyril-rcub fence (roomy): when the popup has rows to spare, the whole
    /// path is on screen — wrapped across rows, never clipped at the border.
    #[rstest::rstest]
    #[case::ascii_80x24(LONG_WRITE_PATH, 80, 24)]
    #[case::ascii_narrow_40x24(LONG_WRITE_PATH, 40, 24)]
    #[case::wide_chars_80x24(WIDE_WRITE_PATH, 80, 24)]
    #[case::wide_chars_narrow_30x24(WIDE_WRITE_PATH, 30, 24)]
    fn long_write_path_is_fully_visible_when_rows_allow(
        #[case] path: &str,
        #[case] width: u16,
        #[case] height: u16,
    ) {
        let terminal = render_at(&write_approval(path), width, height, height);
        let joined = joined_body(&terminal);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            joined.contains(path),
            "full path not readable on screen:\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
    }

    /// cyril-rcub fence (clamped): with a single preview row the path is
    /// middle-elided, keeping the file name whole plus leading context, and
    /// the options stay actionable.
    #[rstest::rstest]
    #[case::ascii_80(LONG_WRITE_PATH, LONG_WRITE_NAME, 80)]
    #[case::ascii_narrow_44(LONG_WRITE_PATH, LONG_WRITE_NAME, 44)]
    #[case::wide_chars_80(WIDE_WRITE_PATH, WIDE_WRITE_NAME, 80)]
    #[case::wide_chars_narrow_36(WIDE_WRITE_PATH, WIDE_WRITE_NAME, 36)]
    fn clamped_write_path_keeps_file_name(
        #[case] path: &str,
        #[case] name: &str,
        #[case] width: u16,
    ) {
        // input_top=8 → region rows 1-7 → popup h=7, inner=5: message,
        // blank, ONE preview row, separator, ONE option row.
        let terminal = render_at(&write_approval(path), width, 16, 8);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        let path_row = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .find(|body| body.contains(name))
            .unwrap_or_else(|| panic!("file name {name} not on screen:\n{screen}"));
        assert!(
            path_row.starts_with("/hom"),
            "leading context dropped: {path_row:?}\n{screen}"
        );
        assert!(
            path_row.ends_with(name),
            "file name must end the row: {path_row:?}\n{screen}"
        );
        assert!(
            path_row.contains('…'),
            "elision must be marked: {path_row:?}\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        for row in rows.iter().skip(8) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// cyril-rcub: the reject-with-reason page shares the preview layout, so
    /// the path it is rejecting is readable there too.
    #[test]
    fn reason_phase_shows_full_write_path() {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: ToolCall::new(
                cyril_core::types::ToolCallId::new("tooluse_rcub"),
                "Write File".into(),
                ToolKind::Write,
                ToolCallStatus::Pending,
                Some(serde_json::json!({"path": LONG_WRITE_PATH, "text": "body"})),
            ),
            message: "Write File".into(),
            options: vec![option("reject", "Reject")],
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
        });
        assert!(ui.approval_begin_feedback(), "feedback phase must open");
        let terminal = render_ui(&ui, 80, 30, 30);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            joined_body(&terminal).contains(LONG_WRITE_PATH),
            "full path not readable on the reason page:\n{screen}"
        );
        assert!(
            screen.contains("Reason ("),
            "editor label missing:\n{screen}"
        );
    }

    /// cyril-rcub snapshots: the roomy wrap at 80x24 and at a narrow width,
    /// and the clamped middle-elision.
    #[rstest::rstest]
    #[case::roomy_80x24("rcub_write_path_roomy_80x24", 80, 24, 24)]
    #[case::roomy_narrow_40x24("rcub_write_path_roomy_40x24", 40, 24, 24)]
    #[case::clamped_80x16("rcub_write_path_clamped_80x16", 80, 16, 8)]
    #[case::clamped_narrow_44x16("rcub_write_path_clamped_44x16", 44, 16, 8)]
    fn long_write_path_snapshots(
        #[case] name: &str,
        #[case] width: u16,
        #[case] height: u16,
        #[case] input_top: u16,
    ) {
        let terminal = render_at(&write_approval(LONG_WRITE_PATH), width, height, input_top);
        insta::assert_snapshot!(name, screen_rows(&terminal).join("\n"));
    }

    #[rstest::rstest]
    #[case::fits("/tmp/a.md", 20, "/tmp/a.md")]
    #[case::keeps_parent_and_name("/home/user/project/src/main.rs", 20, "/home/u…/src/main.rs")]
    #[case::name_only_when_parent_too_wide(
        "/a/very-long-parent-directory/file.txt",
        16,
        "/a/ver…/file.txt"
    )]
    #[case::name_wider_than_budget_keeps_its_end(
        "/x/an-extremely-long-file-name.txt",
        10,
        "…-name.txt"
    )]
    #[case::wide_glyphs("/用户/项目/设计.md", 12, "/用…/设计.md")]
    #[case::windows_separators(
        r"C:\Users\dev\projects\cyril\notes.md",
        20,
        r"C:\U…\cyril\notes.md"
    )]
    #[case::trailing_separator("/home/user/projects/cyril/", 16, "/home/us…/cyril/")]
    #[case::zero_budget("/a/b.md", 0, "")]
    fn elide_middle_keeps_file_name_and_leading_context(
        #[case] path: &str,
        #[case] budget: usize,
        #[case] expected: &str,
    ) {
        let elided = elide_middle(path, budget);
        assert_eq!(elided, expected);
        assert!(elided.width() <= budget, "over budget: {elided:?}");
    }

    #[rstest::rstest]
    #[case::wraps_whole_when_rows_allow("/abc/defgh/ij.md", 5, 4, &["/abc/", "defgh", "/ij.m", "d"])]
    #[case::elides_when_rows_short("/abc/defgh/ij.md", 5, 2, &["/ab…/", "ij.md"])]
    #[case::wide_glyph_straddle_shrinks_budget("/x/a文件", 3, 2, &["…文", "件"])]
    #[case::no_rows("/abc/defgh/ij.md", 5, 0, &[])]
    fn fit_path_wraps_or_elides_into_rows(
        #[case] path: &str,
        #[case] width: usize,
        #[case] rows: usize,
        #[case] expected: &[&str],
    ) {
        assert_eq!(fit_path(path, width, rows), expected);
    }

    /// Invariant sweep over widths and row budgets: never more rows than
    /// granted, no row wider than the width (a lone wide glyph in a
    /// one-column row excepted), the whole path whenever it fits, and the
    /// file name intact whenever the budget can hold it.
    #[test]
    fn fit_path_invariants_hold_across_widths_and_rows() {
        let paths = [
            (LONG_WRITE_PATH, LONG_WRITE_NAME),
            (WIDE_WRITE_PATH, WIDE_WRITE_NAME),
            (r"C:\Users\dev\projects\cyril\notes.md", "notes.md"),
            ("/tmp/a.md", "a.md"),
        ];
        for (path, name) in paths {
            for width in 1..=40usize {
                for rows in 1..=5usize {
                    let fitted = fit_path(path, width, rows);
                    let joined = fitted.concat();
                    let context = format!("path={path} width={width} rows={rows} -> {fitted:?}");
                    assert!(fitted.len() <= rows, "too many rows: {context}");
                    for row in &fitted {
                        assert!(row.width() <= width.max(2), "row too wide: {context}");
                    }
                    if wrap_columns(path, width).len() <= rows {
                        assert_eq!(joined, path, "fitting path altered: {context}");
                    } else {
                        assert!(joined.contains(ELLIPSIS), "unmarked elision: {context}");
                    }
                    if width >= 2 && name.width() + 1 + rows <= width * rows {
                        assert!(joined.ends_with(name), "file name lost: {context}");
                    }
                }
            }
        }
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
            can_reject_with_reason: false,
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
