use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::theme::Theme;
use crate::traits::{ApprovalPhase, ApprovalState, PreviewBody};

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
        ApprovalPhase::SelectScope { .. } => {
            render_scope_phase(frame, area, input_top, state, session_attribution, theme)
        }
        ApprovalPhase::EnterFolder { .. } => {
            render_folder_phase(frame, area, input_top, state, session_attribution, theme)
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
            .saturating_sub(2 + PATH_INDENT.width())
            .max(1);
        let path_rows = path.map_or(0, |path| wrap_columns(path, path_width).len());
        let text_width = usize::from(super::modal::popup_width(area, desired_width))
            .saturating_sub(2 + TEXT_INDENT.width())
            .max(1);
        Self {
            path,
            path_width,
            path_rows,
            body: preview_lines(state, theme, text_width),
        }
    }

    /// Rows the whole preview wants.
    fn len(&self) -> usize {
        self.path_rows.saturating_add(self.body.len())
    }

    fn has_path(&self) -> bool {
        self.path.is_some()
    }

    /// The path fitted into one run of `width` columns, for a row it shares
    /// with an option label. `None` without a path or without a column to
    /// show it in.
    fn inline_path(&self, width: usize) -> Option<String> {
        let path = self.path?;
        if width == 0 {
            return None;
        }
        Some(fit_path(path, width, 1).concat())
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

/// Gap between an option label and a path sharing its row.
const INLINE_PATH_GAP: &str = "  ";

/// The selection marker column of an option row.
fn option_prefix(selected: bool) -> &'static str {
    if selected { "▸ " } else { "  " }
}

/// Which rows the option page shows inside `inner` rows, by priority
/// (cyril-rcub). The selection always gets a row. When the call has a path,
/// any shown option is accompanied by the path, so nothing is approvable
/// with its target hidden: from inner height 2 the path has its own row
/// (ahead of the message at 2, and ahead of the cosmetic blanks at 3), and
/// at height 1 it shares the lone option row. Calls without a path keep the
/// message-first order. Above height 3 the message and blank return, the
/// preview takes all but one row, and its blank separator appears only when
/// a row is spare: the separator wins over a second option row, as the
/// selection stays visible through `window_start`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OptionLayout {
    message: bool,
    blank: bool,
    preview_rows: usize,
    separator: bool,
    option_rows: usize,
    /// The path is appended to the lone option row (height 1).
    inline_path: bool,
}

impl OptionLayout {
    fn new(inner: usize, preview_len: usize, has_path: bool) -> Self {
        let none = Self {
            message: false,
            blank: false,
            preview_rows: 0,
            separator: false,
            option_rows: 0,
            inline_path: false,
        };
        match inner {
            0 => none,
            1 => Self {
                option_rows: 1,
                inline_path: has_path,
                ..none
            },
            2 if has_path => Self {
                preview_rows: 1,
                option_rows: 1,
                ..none
            },
            2 => Self {
                message: true,
                option_rows: 1,
                ..none
            },
            3 if has_path => Self {
                message: true,
                preview_rows: 1,
                option_rows: 1,
                ..none
            },
            3 => Self {
                message: true,
                blank: true,
                option_rows: 1,
                ..none
            },
            n => {
                let remaining = n - 2; // message + blank
                let preview_rows = preview_len.min(remaining - 1);
                let separator = preview_rows > 0 && remaining - preview_rows >= 2;
                Self {
                    message: true,
                    blank: true,
                    preview_rows,
                    separator,
                    option_rows: remaining - preview_rows - usize::from(separator),
                    inline_path: false,
                }
            }
        }
    }
}

/// Split `text` into rows of at most `width` display columns. A character
/// never splits: a wide glyph that would straddle the edge starts the next
/// row, matching how ratatui draws it (the glyph plus a blank continuation
/// cell).
///
/// A character with no display width (a control character) counts as zero
/// columns, as ratatui draws nothing for it; that is its width, not a
/// fallback for an error.
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
        Some(head_room) => format!("{}{ELLIPSIS}{tail}", leading_components(path, head_room)),
        None => format!("{ELLIPSIS}{}", trailing_columns(tail, room)),
    }
}

/// The longest prefix of `path` that ends just after a separator and is at
/// most `width` display columns wide, so every directory it shows is whole:
/// `/home/dwa…` could be `dwalleck` or `dwalleck-x`, `/home/…` cannot
/// mislead (cyril-rcub review round 2). Empty when not even the first
/// component fits.
fn leading_components(path: &str, width: usize) -> &str {
    let mut used = 0usize;
    let mut cut = 0usize;
    for (offset, ch) in path.char_indices() {
        used = used.saturating_add(UnicodeWidthChar::width(ch).unwrap_or(0));
        if used > width {
            break;
        }
        if PATH_SEPARATORS.contains(&ch) {
            cut = offset + ch.len_utf8();
        }
    }
    &path[..cut]
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

/// Indent before each text row of the preview body.
const TEXT_INDENT: &str = "    ";

/// Push at most [`MAX_PREVIEW_LINES`] rows of `text_lines`, each wrapped to
/// `width` columns, indented and dimmed, then an omission marker naming how
/// many rows were left out. Wrapping keeps a long line's tail on screen —
/// the end of a KAS plan line carries the step's effort (cyril-8wh0) —
/// instead of clipping it at the border.
fn push_bounded_text<'a, 's>(
    lines: &mut Vec<Line<'a>>,
    text_lines: impl Iterator<Item = &'s str>,
    width: usize,
    theme: &Theme,
) {
    let mut total = 0usize;
    for row in text_lines.flat_map(|line| wrap_columns(line, width)) {
        total += 1;
        if total <= MAX_PREVIEW_LINES {
            lines.push(Line::styled(
                format!("{TEXT_INDENT}{row}"),
                Style::default().fg(theme.subdued),
            ));
        }
    }
    if total > MAX_PREVIEW_LINES {
        lines.push(Line::styled(
            format!("{TEXT_INDENT}...{} more lines", total - MAX_PREVIEW_LINES),
            Style::default().fg(theme.subdued),
        ));
    }
}

/// Build the approval-preview body from the joined tool-call snapshot. The
/// path rows above it are laid out by [`Preview`] (cyril-rcub).
///
/// Order: diff content
/// (the existing diff projection, with its own line cap), else the tool
/// call's Text content blocks (cyril-8wh0), else the raw-input
/// `text`/`content` payload; text of either kind is wrapped to `text_width`
/// and capped at [`MAX_PREVIEW_LINES`] rows. A snapshot with no displayable
/// payload yields a single `Preview unavailable` line — the
/// degraded-but-actionable contract from cyril-j1b3's spec: the operator
/// still sees the request's own title/message and every option.
///
/// Runs every frame, so it never logs: a malformed payload's warning is
/// emitted once when the approval is installed (cyril-q0kg).
fn preview_lines<'a>(state: &ApprovalState, theme: &Theme, text_width: usize) -> Vec<Line<'a>> {
    let tc = &state.tool_call;
    let mut lines: Vec<Line<'a>> = Vec::new();

    match tc.preview_body() {
        PreviewBody::Diff => super::chat::render_diff_lines(&mut lines, tc, theme),
        PreviewBody::Content(blocks) => push_bounded_text(
            &mut lines,
            blocks.iter().flat_map(|block| match block {
                cyril_core::types::ToolCallContent::Text(text) => text.lines(),
                // Classification guarantees Text only; a diff is never
                // flattened into text.
                cyril_core::types::ToolCallContent::Diff { .. } => "".lines(),
            }),
            text_width,
            theme,
        ),
        PreviewBody::Text(text) => push_bounded_text(&mut lines, text.lines(), text_width, theme),
        // Never coerce a non-string payload (cyril-j1b3 spec).
        PreviewBody::Absent | PreviewBody::Malformed { .. } => lines.push(Line::styled(
            "  Preview unavailable",
            Style::default().fg(theme.subdued),
        )),
    }
    lines
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

    // Inner rows inside the borders decide what fits, by the priority in
    // [`OptionLayout`]: the selection always, the path beside any option,
    // then message, blanks, the rest of the preview and the other options.
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let layout = OptionLayout::new(inner, preview.len(), preview.has_path());
    let mut option_rows = layout.option_rows;
    let show_feedback_hint = show_feedback_hint && option_rows > 1;
    if show_feedback_hint {
        option_rows -= 1;
    }

    let visible = state.options.len().min(option_rows);
    let start = window_start(state.selected, state.options.len(), visible);
    // Height 1 shows exactly one option row (the selection, via
    // `window_start`); the path shares it after the label.
    let inline_path = if layout.inline_path && visible == 1 {
        state.options.get(start).and_then(|opt| {
            let label_width = option_prefix(start == state.selected)
                .width()
                .saturating_add(opt.label.width())
                .saturating_add(INLINE_PATH_GAP.width());
            preview.inline_path(
                usize::from(popup_area.width.saturating_sub(2)).saturating_sub(label_width),
            )
        })
    } else {
        None
    };

    let mut lines: Vec<Line> = Vec::new();
    if layout.message {
        lines.push(Line::styled(
            &state.message,
            Style::default().fg(theme.emphasis),
        ));
    }
    if layout.blank {
        lines.push(Line::default());
    }
    lines.extend(preview.into_lines(layout.preview_rows, theme));
    if layout.separator {
        lines.push(Line::default());
    }

    for (i, opt) in state.options.iter().enumerate().skip(start).take(visible) {
        let style = if i == state.selected {
            Style::default()
                .bg(theme.selection)
                .fg(theme.text)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(theme.text_secondary)
        };
        let label = format!("{}{}", option_prefix(i == state.selected), opt.label);
        match &inline_path {
            Some(path) => lines.push(
                Line::from(vec![
                    Span::raw(label),
                    Span::styled(
                        format!("{INLINE_PATH_GAP}{path}"),
                        Style::default()
                            .fg(theme.text_secondary)
                            .remove_modifier(Modifier::BOLD),
                    ),
                ])
                .style(style),
            ),
            None => lines.push(Line::styled(label, style)),
        }
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

/// Kiro's TUI label truncation (`UM`): more than 50 characters keep the first
/// 47 and an ellipsis.
fn truncate_label(text: &str) -> String {
    if text.chars().count() > 50 {
        format!("{}…", text.chars().take(47).collect::<String>())
    } else {
        text.to_owned()
    }
}

/// The scope page's row text, as Kiro's TUI words it (`isr`, `m2t`).
fn scope_row(
    choice: &cyril_core::always_allow::ScopeChoice,
    scope: cyril_core::always_allow::GrantScope,
    tool: Option<&str>,
) -> (String, String) {
    use cyril_core::always_allow::ScopeChoice;
    let scope_label = scope.label();
    match choice {
        ScopeChoice::Exact(resource) => (
            format!("Trust \"{}\"", truncate_label(resource)),
            format!("exact match · {scope_label}"),
        ),
        ScopeChoice::Pattern(pattern) => (
            format!("Trust \"{pattern}\""),
            format!("pattern · {scope_label}"),
        ),
        ScopeChoice::Folder(offer) => (
            format!("Allow a folder {}…", scope.folder_phrase()),
            format!("{} · {scope_label}", truncate_label(offer.prefill())),
        ),
        ScopeChoice::WholeTool => (
            match tool {
                Some(tool) => format!("Trust entire tool ({tool})"),
                None => "Trust entire tool".to_owned(),
            },
            scope_label.to_owned(),
        ),
    }
}

/// KAS "Always allow" scope page (cyril-auxt): `capability → resource`, the
/// scope line (`s` cycles), and one label + description pair per row.
fn render_scope_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::SelectScope { scope, .. } = &state.phase else {
        return;
    };
    let choices = state.scope_choices();
    let consent = state.consent.as_ref();
    let capability = consent.and_then(|c| c.wire_capability());
    let resource = consent
        .and_then(|c| c.resource())
        .filter(|resource| !resource.is_empty());
    let target = match (capability, resource) {
        (Some(capability), Some(resource)) => Some(format!("{capability} → {resource}")),
        (Some(only), None) | (None, Some(only)) => Some(only.to_owned()),
        (None, None) => None,
    };
    let tool = consent.and_then(|c| c.tool_id());

    // Chrome: target + blank (when present), scope line + blank, hint.
    let chrome = usize::from(target.is_some()) * 2 + 3;
    let desired_height = u16::try_from(choices.len().saturating_mul(2).saturating_add(chrome + 2))
        .unwrap_or(u16::MAX);
    let Some(popup_area) = super::modal::place(area, input_top, 72, desired_height) else {
        return;
    };
    frame.render_widget(Clear, popup_area);

    // Rows win over chrome when clamped: the selected row's label is what
    // keeps the page actionable.
    let inner = usize::from(popup_area.height.saturating_sub(2));
    let show_chrome = inner >= chrome + 2;
    let row_lines = if show_chrome { inner - chrome } else { inner };
    let visible = choices.len().min((row_lines / 2).max(1));
    let start = window_start(state.selected, choices.len(), visible);

    let mut lines: Vec<Line> = Vec::new();
    if show_chrome {
        if let Some(target) = target {
            lines.push(Line::styled(
                target,
                Style::default().fg(theme.text_secondary),
            ));
            lines.push(Line::default());
        }
        let mut spans = vec![Span::styled(
            "Scope:  ",
            Style::default().fg(theme.text_secondary),
        )];
        for (index, option) in cyril_core::always_allow::GrantScope::ALL.iter().enumerate() {
            if index > 0 {
                spans.push(Span::raw("  "));
            }
            if option == scope {
                spans.push(Span::styled(
                    format!("[{}]", option.label()),
                    Style::default()
                        .fg(theme.accent_quinary)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    format!(" {} ", option.label()),
                    Style::default().fg(theme.text_secondary),
                ));
            }
        }
        lines.push(Line::from(spans));
        lines.push(Line::default());
    }
    for (index, choice) in choices.iter().enumerate().skip(start).take(visible) {
        let (label, description) = scope_row(choice, *scope, tool);
        let selected = index == state.selected;
        let (label_style, description_style) = if selected {
            (
                Style::default()
                    .bg(theme.selection)
                    .fg(theme.text)
                    .add_modifier(Modifier::BOLD),
                Style::default().bg(theme.selection).fg(theme.subdued),
            )
        } else {
            (
                Style::default().fg(theme.text_secondary),
                Style::default().fg(theme.subdued),
            )
        };
        let prefix = if selected { "▸ " } else { "  " };
        lines.push(Line::styled(format!("{prefix}{label}"), label_style));
        lines.push(Line::styled(
            format!("    {description}"),
            description_style,
        ));
    }
    if show_chrome {
        lines.push(Line::styled(
            "↑↓ navigate · Enter select · s cycle scope · Esc back",
            Style::default().fg(theme.subdued),
        ));
    }

    let title = attribution_title(
        " Always Allow — Choose Scope ",
        session_attribution,
        theme.accent_quinary,
    );
    let popup = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.accent_quinary)),
    );
    frame.render_widget(popup, popup_area);
}

/// KAS "Always allow" folder editor (cyril-auxt): Kiro's live validation line
/// (`h2t` / `vde`) above a one-line editor.
fn render_folder_phase(
    frame: &mut Frame,
    area: Rect,
    input_top: u16,
    state: &ApprovalState,
    session_attribution: Option<&str>,
    theme: &Theme,
) {
    let ApprovalPhase::EnterFolder {
        scope,
        offer,
        editor,
        attempted,
        ..
    } = &state.phase
    else {
        return;
    };
    let status = match offer.validate(editor.text()) {
        Ok(folder) => {
            let reach = match scope {
                cyril_core::always_allow::GrantScope::Session => "for this session only",
                other => other.folder_phrase(),
            };
            Line::styled(
                format!(
                    "Allows writes in {} and every folder under it, {reach}.",
                    folder.path()
                ),
                Style::default().fg(theme.text_secondary),
            )
        }
        Err(reason) => Line::styled(
            reason.to_string(),
            Style::default().fg(if *attempted {
                theme.danger
            } else {
                theme.text_secondary
            }),
        ),
    };

    let Some(popup_area) = super::modal::place(area, input_top, 84, 7) else {
        return;
    };
    frame.render_widget(Clear, popup_area);
    let inner = usize::from(popup_area.height.saturating_sub(2));
    if inner == 0 {
        return;
    }
    // The editor row survives any clamp; the status and hint join when the
    // popup has room for them.
    let show_status = inner >= 2;
    let show_hint = inner >= 4;
    let content_width = usize::from(popup_area.width.saturating_sub(2)).max(1);
    let mut lines: Vec<Line> = Vec::with_capacity(inner);
    if show_status {
        lines.push(status);
    }
    if show_hint {
        lines.push(Line::default());
    }
    lines.extend(super::input::text_cursor_lines(
        editor.text(),
        editor.cursor(),
        content_width,
        1,
        theme,
    ));
    if show_hint {
        lines.push(Line::default());
        lines.push(Line::styled(
            "Enter to allow this folder · Esc back",
            Style::default().fg(theme.subdued),
        ));
    }

    let heading = format!(" Allow a folder {} ", scope.folder_phrase());
    let title = attribution_title(&heading, session_attribution, theme.accent_quinary);
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
            consent: None,
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
        // input_top=7 → region rows 1-6 → popup h=6, inner=4: message,
        // blank, ONE preview row, ONE option row (no spare row for the
        // separator).
        let terminal = render_at(&write_approval(path), width, 16, 7);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        let path_row = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .find(|body| body.contains(name))
            .unwrap_or_else(|| panic!("file name {name} not on screen:\n{screen}"));
        assert_whole_component_head(path, path_row, &screen);
        assert!(
            path_row.ends_with(name),
            "file name must end the row: {path_row:?}\n{screen}"
        );
        assert!(
            path_row.contains('…'),
            "elision must be marked: {path_row:?}\n{screen}"
        );
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        for row in rows.iter().skip(7) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// The text before an elision is a whole-component prefix of `path`:
    /// empty, or ending at a separator (review round 2).
    fn assert_whole_component_head(path: &str, path_row: &str, screen: &str) {
        let Some((head, _)) = path_row.split_once('…') else {
            return; // nothing elided
        };
        assert!(
            path.starts_with(head) && (head.is_empty() || head.ends_with(['/', '\\'])),
            "head {head:?} is not a whole-component prefix of {path:?}\n{screen}"
        );
    }

    /// Popup body rows that end with the last `columns` display columns of
    /// `path` — evidence the path's end is on screen.
    fn rows_ending_with_path_tail(rows: &[String], path: &str, columns: usize) -> usize {
        let tail = trailing_columns(path, columns);
        rows.iter()
            .filter_map(|row| popup_body(row))
            .filter(|body| body.ends_with(tail))
            .count()
    }

    /// cyril-rcub review round 2 fence: at the cramped heights (inner 1-3)
    /// an option is actionable, so the path must be on screen beside it.
    /// Inner 3: message, path, option. Inner 2: path, option (the message is
    /// the first chrome dropped). Inner 1: the lone option row carries the
    /// elided path after its label.
    #[rstest::rstest]
    #[case::inner_1(4, false)]
    #[case::inner_2(5, false)]
    #[case::inner_3(6, true)]
    fn cramped_heights_show_path_beside_the_option(
        #[case] input_top: u16,
        #[case] message_shown: bool,
    ) {
        let terminal = render_at(&write_approval(LONG_WRITE_PATH), 80, 16, input_top);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        assert!(screen.contains("▸ Allow"), "selection missing:\n{screen}");
        assert_eq!(
            rows_ending_with_path_tail(&rows, LONG_WRITE_PATH, LONG_WRITE_NAME.width()),
            1,
            "file name must end exactly one row:\n{screen}"
        );
        assert_eq!(
            screen.contains("Write File"),
            message_shown,
            "message presence at this height:\n{screen}"
        );
        for row in rows.iter().skip(usize::from(input_top)) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
    }

    /// cyril-rcub review round 2: the layout table at every height and
    /// preview size. Rows never exceed `inner`; any non-empty popup shows
    /// the selection; with a path, any shown option has the path beside it
    /// (its own row, or inline at height 1).
    #[test]
    fn option_layout_never_shows_an_option_without_the_path() {
        for inner in 0usize..=12 {
            for preview_len in 0usize..=8 {
                for has_path in [false, true] {
                    if has_path && preview_len == 0 {
                        continue; // a path always yields at least one row
                    }
                    let layout = OptionLayout::new(inner, preview_len, has_path);
                    let context = format!("inner={inner} preview={preview_len} path={has_path}");
                    let used = usize::from(layout.message)
                        + usize::from(layout.blank)
                        + layout.preview_rows
                        + usize::from(layout.separator)
                        + layout.option_rows;
                    assert!(used <= inner, "over budget {layout:?}: {context}");
                    assert_eq!(
                        layout.option_rows >= 1,
                        inner >= 1,
                        "selection row: {context}"
                    );
                    assert!(
                        layout.preview_rows <= preview_len,
                        "invented rows: {context}"
                    );
                    if has_path && layout.option_rows >= 1 {
                        assert!(
                            layout.preview_rows >= 1 || layout.inline_path,
                            "option without path {layout:?}: {context}"
                        );
                    }
                    if !has_path {
                        assert!(!layout.inline_path, "inline without a path: {context}");
                    }
                }
            }
        }
    }

    /// cyril-rcub review round 2 sweep: across terminal widths and EVERY
    /// popup height, whenever an option row is visible the path's end (at
    /// least `…` plus the file name's last columns) is visible too; at 80
    /// columns the whole file name is. Nothing renders at or below the input.
    #[test]
    fn option_visible_implies_path_visible_at_every_height() {
        for (path, name) in [
            (LONG_WRITE_PATH, LONG_WRITE_NAME),
            (WIDE_WRITE_PATH, WIDE_WRITE_NAME),
        ] {
            for width in [24u16, 30, 36, 44, 60, 80] {
                for input_top in 2u16..=20 {
                    let terminal = render_at(&write_approval(path), width, 24, input_top);
                    let rows = screen_rows(&terminal);
                    let screen = rows.join("\n");
                    let context = format!("path={path} width={width} input_top={input_top}");
                    for row in rows.iter().skip(usize::from(input_top)) {
                        assert_eq!(row.trim(), "", "bled below input: {context}\n{screen}");
                    }
                    if !screen.contains('▸') {
                        continue; // no actionable row at this height
                    }
                    // Joined rows: a roomy popup wraps the name across rows.
                    let tail_columns = if width >= 80 { name.width() } else { 4 };
                    assert!(
                        joined_body(&terminal).contains(trailing_columns(path, tail_columns)),
                        "option visible without the path's end: {context}\n{screen}"
                    );
                }
            }
        }
    }

    /// cyril-rcub review round 2: the clamped reject-with-reason page. At
    /// inner 4 it holds the editor row, its label, the limit line and ONE
    /// preview row, which carries the elided path ending in the file name.
    #[test]
    fn clamped_reason_phase_keeps_file_name_and_cursor() {
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
        ui.approval_feedback_paste("not that file");
        // input_top=7 → region rows 1-6 → popup h=6, inner=4.
        let terminal = render_ui(&ui, 80, 16, 7);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        let path_row = rows
            .iter()
            .filter_map(|row| popup_body(row))
            .find(|body| body.ends_with(LONG_WRITE_NAME))
            .unwrap_or_else(|| panic!("file name not on the reason page:\n{screen}"));
        assert!(path_row.contains('…'), "elision must be marked:\n{screen}");
        assert_whole_component_head(LONG_WRITE_PATH, path_row, &screen);
        assert!(
            screen.contains("not that file█"),
            "cursor clipped:\n{screen}"
        );
        for row in rows.iter().skip(7) {
            assert_eq!(row.trim(), "", "popup bled into input rows:\n{screen}");
        }
        insta::assert_snapshot!("rcub_reason_phase_clamped_80x16", screen);
    }

    /// cyril-rcub (found by the clamped fence): the blank row after a shown
    /// preview is counted in the row budget, so the selected LAST option
    /// stays inside a clamped popup at every clamp height — and the path is
    /// not given up for the cosmetic separator (inner 4 has room for both).
    #[rstest::rstest]
    #[case::inner_4(7)]
    #[case::inner_5(8)]
    #[case::inner_6(9)]
    #[case::inner_7(10)]
    fn clamped_preview_keeps_selected_option_visible(#[case] input_top: u16) {
        let mut state = write_approval("/tmp/a.md");
        state.selected = 1;
        let terminal = render_at(&state, 80, 16, input_top);
        let rows = screen_rows(&terminal);
        let screen = rows.join("\n");
        assert!(screen.contains("/tmp/a.md"), "preview dropped:\n{screen}");
        assert!(
            screen.contains("▸ Deny"),
            "selected option clipped:\n{screen}"
        );
        for row in rows.iter().skip(usize::from(input_top)) {
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
    #[case::clamped_80x16("rcub_write_path_clamped_80x16", 80, 16, 7)]
    #[case::clamped_narrow_44x16("rcub_write_path_clamped_44x16", 44, 16, 7)]
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
    #[case::keeps_parent_and_name("/home/user/project/src/main.rs", 20, "/home/…/src/main.rs")]
    #[case::name_only_when_parent_too_wide(
        "/a/very-long-parent-directory/file.txt",
        16,
        "/a/…/file.txt"
    )]
    // Review round 2: `/home/dwa…` could be `dwalleck` or `dwalleck-x`; the
    // head stops at a separator so every directory shown is whole.
    #[case::head_never_shows_a_partial_directory(
        "/home/dwalleck-x/repos/cyril/notes.txt",
        26,
        "/home/…/cyril/notes.txt"
    )]
    #[case::relative_path_with_no_whole_head_component(
        "notes/long-directory-name/file.txt",
        14,
        "…/file.txt"
    )]
    #[case::name_wider_than_budget_keeps_its_end(
        "/x/an-extremely-long-file-name.txt",
        10,
        "…-name.txt"
    )]
    #[case::wide_glyphs("/用户/项目/设计.md", 12, "/…/设计.md")]
    #[case::wide_glyph_head_component("/用户/项目/文档/设计.md", 21, "/用户/…/文档/设计.md")]
    #[case::windows_separators(r"C:\Users\dev\projects\cyril\notes.md", 20, r"C:\…\cyril\notes.md")]
    #[case::trailing_separator("/home/user/projects/cyril/", 16, "/home/…/cyril/")]
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
    #[case::elides_when_rows_short("/abc/defgh/ij.md", 5, 2, &["/…/ij", ".md"])]
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
            consent: None,
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

    // ---------- cyril-8wh0: Text content in the approval preview ----------

    /// The plan text KAS 0.66.43 sent as the `toolCall.content` Text block of
    /// a workflow plan approval (`kas-lead-wfplan-2.32.0.jsonl` frame 93,
    /// workspace path scrubbed to `<LIVE_TMP>` as in
    /// `experiments/conductor-spike/kas-workflow-plan-2.32.0.jsonl`).
    const LIVE_PLAN_TEXT: &str = "Steps to add after 'create-workflow':\n\nwrite-hello: step, agent wf-coder, model auto, effort low\n  Task: Create a file named hello.txt at the absolute path <LIVE_TMP>/ws-wfplan-nnzrhi04/hello.txt con...";

    /// The update_workflow approval as cyril joins it: the tracked tool call
    /// (title "Update Workflow", rawInput without a text field) merged with
    /// the request's Text content.
    fn plan_text_tool_call(text: &str) -> ToolCall {
        ToolCall::new(
            cyril_core::types::ToolCallId::new("toolu_bdrk_01QLZqv14pnXT2nFjjkTakjL"),
            "Update Workflow".into(),
            ToolKind::Other,
            ToolCallStatus::Pending,
            Some(serde_json::json!({
                "workflowId": "wf_6b22f0c99f407f0e",
                "action": "replace_remaining",
                "workflowRef": "generated://gen_09d8668976ecd154",
                "remainingSteps": []
            })),
        )
        .with_content(vec![ToolCallContent::Text(text.into())])
    }

    /// REGRESSION FENCE (cyril-8wh0): a permission request whose tool call
    /// carries a Text content block shows that text in the overlay instead of
    /// "Preview unavailable" — the live KAS workflow plan approval.
    #[test]
    fn text_content_renders_in_the_preview() {
        let mut state = approval_for_tool_call(plan_text_tool_call(LIVE_PLAN_TEXT));
        state.message = "Add 1 workflow step to wf_6b22f0c99f407f0e".into();
        let terminal = render_at(&state, 80, 24, 24);
        let screen = screen_rows(&terminal).join("\n");
        assert!(
            !screen.contains("Preview unavailable"),
            "the Text block must be shown:\n{screen}"
        );
        // Long plan lines wrap at the popup's text width rather than being
        // clipped at the border, so the step's effort stays on screen.
        for expected in [
            "Steps to add after 'create-workflow':",
            "write-hello: step, agent wf-coder, model auto,",
            "low",
            "Task: Create a file named hello.txt",
        ] {
            assert!(screen.contains(expected), "missing {expected:?}:\n{screen}");
        }
        assert!(screen.contains("▸ Allow"), "options must stay:\n{screen}");
        insta::assert_snapshot!("8wh0_text_content_preview_80x24", screen);
    }

    /// cyril-8wh0: Text content is bounded like the raw-input text — five
    /// lines across all blocks, then the omission marker.
    #[test]
    fn text_content_preview_is_bounded() {
        let seven = (1..=7)
            .map(|i| format!("plan line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = plan_text_tool_call(&seven).with_content(vec![
            ToolCallContent::Text("plan line 1\nplan line 2".into()),
            ToolCallContent::Text(
                (3..=7)
                    .map(|i| format!("plan line {i}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ]);
        let state = approval_for_tool_call(tc);
        let terminal = render_at(&state, 90, 40, 40);
        let text = buffer_text(&terminal);
        assert!(text.contains("plan line 5"), "fifth line missing:\n{text}");
        assert!(
            !text.contains("plan line 6"),
            "sixth line must be capped:\n{text}"
        );
        assert!(
            text.contains("...2 more lines"),
            "omission marker missing:\n{text}"
        );
        assert!(text.contains("▸ Allow"), "options must stay:\n{text}");
    }

    // ---------- cyril-q0kg: malformed-preview warning is not per-frame ----------

    const MALFORMED_PREVIEW_WARNING: &str =
        "approval preview raw_input has no string text/content field";

    /// A permission-request tool call with the given raw input, optionally
    /// carrying diff content (which takes precedence over raw input).
    fn preview_tool_call(raw_input: Option<serde_json::Value>, with_diff: bool) -> ToolCall {
        let tc = ToolCall::new(
            cyril_core::types::ToolCallId::new("tooluse_q0kg"),
            "Write File".into(),
            ToolKind::Write,
            ToolCallStatus::Pending,
            raw_input,
        );
        if with_diff {
            tc.with_content(vec![ToolCallContent::Diff {
                path: "/tmp/q0kg.md".into(),
                old_text: None,
                new_text: "new body".into(),
            }])
        } else {
            tc
        }
    }

    /// Install `tc` as an approval through the production `show_approval`
    /// path, with a reject option so the feedback phase can be entered too.
    fn approval_ui_for(tc: ToolCall) -> UiState {
        let mut ui = UiState::new(500);
        ui.show_approval(cyril_core::types::PermissionRequest {
            session_id: cyril_core::types::SessionId::new("main"),
            tool_call: tc,
            message: "Write File".into(),
            options: vec![option("reject", "Reject")],
            trust_options: vec![],
            can_reject_with_reason: true,
            responder: tokio::sync::oneshot::channel().0,
            consent: None,
        });
        ui
    }

    fn malformed_preview_warnings(events: &[serde_json::Value]) -> Vec<&serde_json::Value> {
        events
            .iter()
            .filter(|event| event["fields"]["message"] == MALFORMED_PREVIEW_WARNING)
            .collect()
    }

    /// cyril-q0kg fence: installing an approval and rendering it for many
    /// frames — in both the option and the reject-with-reason phase — logs
    /// the malformed-shape warning at most once for the tool call (the
    /// cyril-j1b3 intent), never once per frame. Payloads that render a body
    /// (string text, diff) or carry no raw input never warn.
    #[rstest::rstest]
    #[case::non_string_text(Some(serde_json::json!({"path": "/tmp/ok.md", "text": 42})), false, 1)]
    #[case::shell_command_shape(Some(serde_json::json!({"command": "ls -la"})), false, 1)]
    #[case::string_text(Some(serde_json::json!({"text": "hello"})), false, 0)]
    #[case::string_content(Some(serde_json::json!({"content": "hello"})), false, 0)]
    #[case::no_raw_input(None, false, 0)]
    #[case::diff_takes_precedence(Some(serde_json::json!({"command": "ls"})), true, 0)]
    fn malformed_preview_warning_is_not_repeated_per_frame(
        #[case] raw_input: Option<serde_json::Value>,
        #[case] with_diff: bool,
        #[case] expected_warnings: usize,
    ) {
        let (_guard, capture, dispatch) = cyril_core::test_support::capture_json_subscriber();
        tracing::dispatcher::with_default(&dispatch, || {
            let mut ui = approval_ui_for(preview_tool_call(raw_input, with_diff));
            for _ in 0..3 {
                render_ui(&ui, 80, 24, 24);
            }
            assert!(ui.approval_begin_feedback(), "feedback phase must open");
            for _ in 0..3 {
                render_ui(&ui, 80, 24, 24);
            }
        });

        let events = capture.captured();
        let warnings = malformed_preview_warnings(&events);
        assert_eq!(
            warnings.len(),
            expected_warnings,
            "malformed-preview warnings across 6 frames: {events:?}"
        );
        for warning in warnings {
            assert_eq!(warning["level"], "WARN", "at warn level: {warning:?}");
            assert_eq!(
                warning["fields"]["tool_call_id"], "tooluse_q0kg",
                "names the tool call: {warning:?}"
            );
            assert_eq!(
                warning["fields"]["raw_input_shape"], "object",
                "names the shape, never the value: {warning:?}"
            );
        }
    }

    /// cyril-q0kg: the degraded marker still renders on every frame for a
    /// payload that warned once at install time.
    #[test]
    fn malformed_preview_marker_survives_repeated_renders() {
        let ui = approval_ui_for(preview_tool_call(
            Some(serde_json::json!({"command": "ls -la"})),
            false,
        ));
        for _ in 0..2 {
            let text = buffer_text(&render_ui(&ui, 80, 24, 24));
            assert!(
                text.contains("Preview unavailable"),
                "degraded marker missing:\n{text}"
            );
        }
    }
}
