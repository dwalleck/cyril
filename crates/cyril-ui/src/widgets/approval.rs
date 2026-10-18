use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

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
///
/// Runs every frame, so it never logs: a malformed payload's warning is
/// emitted once when the approval is installed (cyril-q0kg).
fn preview_lines<'a>(state: &ApprovalState, theme: &Theme) -> Vec<Line<'a>> {
    let tc = &state.tool_call;
    let mut lines: Vec<Line<'a>> = Vec::new();

    if let Some(path) = tc.primary_path() {
        lines.push(Line::styled(
            format!("  {path}"),
            Style::default().fg(theme.text_secondary),
        ));
    }

    match tc.preview_body() {
        PreviewBody::Diff => super::chat::render_diff_lines(&mut lines, tc, theme),
        PreviewBody::Text(text) => {
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
    // file path and proposed content. Budget: 1 path line + up to
    // MAX_PREVIEW_LINES of body + blanks; clamped below when space is tight.
    let preview = preview_lines(state, theme);
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
    let (show_message, show_blank, preview_rows, mut option_rows) = match inner {
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
    let preview = preview_lines(state, theme);
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
    let Some(popup_area) = super::modal::place(area, input_top, 64, desired_height) else {
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
    lines.extend(preview.into_iter().take(preview_rows));
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
