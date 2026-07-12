use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap};

use crate::theme::Theme;
use crate::traits::{ChatMessage, ChatMessageKind, SteerEchoStatus, TrackedToolCall, TuiState};
use crate::widgets::markdown;

const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const SPINNER_FRAME_MS: u128 = 80;

/// Render the chat area. If a subagent is focused, renders the focused
/// subagent's stream instead of the main chat.
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState, theme: &Theme) {
    // Drill-in: if a subagent is focused, render its stream instead.
    if let Some(focused) = state.subagent_ui().focused_stream() {
        render_subagent_drill_in(frame, area, state, focused, theme);
        return;
    }

    let mut lines: Vec<Line> = Vec::new();

    // Render committed messages (includes tool calls in chronological position)
    for msg in state.messages() {
        render_message(&mut lines, msg, area.width as usize, theme);
        lines.push(Line::default()); // spacing between messages
    }

    // Render streaming text
    let streaming = state.streaming_text();
    if !streaming.is_empty() {
        lines.push(Line::styled(
            "Kiro:",
            Style::default()
                .fg(theme.agent)
                .add_modifier(Modifier::BOLD),
        ));
        let md_lines = markdown::render_with_theme(streaming, area.width as usize, theme);
        lines.extend(md_lines);
    }

    // Render streaming thought
    if let Some(thought) = state.streaming_thought() {
        push_thought_lines(&mut lines, thought, theme);
    }

    // Activity indicator — visible in the chat area when the agent is busy
    // but not actively streaming text.
    render_activity_indicator(&mut lines, state, theme);

    let visible_height = area.height as usize;

    let chat = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default());

    // Use line_count to get the wrapped height (accounts for long lines wrapping)
    let total_lines = chat.line_count(area.width);

    let max_scroll = total_lines.saturating_sub(visible_height);
    let scroll_offset = match state.chat_scroll_back() {
        None => max_scroll,
        Some(back) => max_scroll.saturating_sub(back),
    };

    if scroll_offset > u16::MAX as usize {
        tracing::warn!(scroll_offset, "scroll offset exceeds u16::MAX, clamping");
    }
    let scroll_clamped = scroll_offset.min(u16::MAX as usize) as u16;
    let chat = chat.scroll((scroll_clamped, 0));

    frame.render_widget(chat, area);

    // Scrollbar
    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Render a focused subagent's message stream in place of the main chat.
/// Shows a header with the subagent name and "[Esc] Back" hint.
fn render_subagent_drill_in(
    frame: &mut Frame,
    area: Rect,
    state: &dyn TuiState,
    stream: &crate::subagent_ui::SubagentStream,
    theme: &Theme,
) {
    let mut lines: Vec<Line> = Vec::new();

    // Header bar with subagent name
    let focused_id = state.subagent_ui().focused_session_id();
    let name = focused_id
        .and_then(|id| state.subagent_tracker().get(id))
        .map(|info| info.session_name().to_string())
        .or_else(|| focused_id.map(|id| id.as_str().to_string()))
        .unwrap_or_else(|| "subagent".to_string());

    lines.push(Line::from(vec![
        Span::styled(
            format!("─── {name} "),
            Style::default()
                .fg(theme.soft_accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("[Esc] Back", Style::default().fg(theme.muted)),
    ]));
    lines.push(Line::default());

    // Render committed messages
    for msg in stream.messages() {
        render_message(&mut lines, msg, area.width as usize, theme);
        lines.push(Line::default());
    }

    // Render streaming text
    let streaming = stream.streaming_text();
    if !streaming.is_empty() {
        lines.push(Line::styled(
            format!("{name}:"),
            Style::default()
                .fg(theme.agent)
                .add_modifier(Modifier::BOLD),
        ));
        let md_lines = markdown::render_with_theme(streaming, area.width as usize, theme);
        lines.extend(md_lines);
    }

    let visible_height = area.height as usize;

    let chat = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .block(Block::default());

    let total_lines = chat.line_count(area.width);
    let scroll_offset = if total_lines > visible_height {
        total_lines.saturating_sub(visible_height)
    } else {
        0
    };

    let scroll_clamped = scroll_offset.min(u16::MAX as usize) as u16;
    let chat = chat.scroll((scroll_clamped, 0));
    frame.render_widget(chat, area);

    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll_offset);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Render a (possibly multi-line) agent thought block as muted italic lines.
/// The 💭 marker prefixes the first line; continuation lines are indented to
/// align under it, because accumulated thoughts span multiple physical lines
/// and a single `Line` would not break on embedded newlines.
fn push_thought_lines(lines: &mut Vec<Line>, text: &str, theme: &Theme) {
    let style = Style::default()
        .fg(theme.muted)
        .add_modifier(Modifier::ITALIC);
    if text.is_empty() {
        // Live preview before the first thought token: keep the 💭 placeholder
        // visible instead of rendering nothing (`"".lines()` yields no rows).
        lines.push(Line::styled("💭 ", style));
        return;
    }
    for (i, segment) in text.lines().enumerate() {
        let rendered = if i == 0 {
            format!("💭 {segment}")
        } else {
            format!("   {segment}")
        };
        lines.push(Line::styled(rendered, style));
    }
}

fn render_message(lines: &mut Vec<Line>, msg: &ChatMessage, width: usize, theme: &Theme) {
    match msg.kind() {
        ChatMessageKind::UserText(text) => {
            lines.push(Line::styled(
                "You:",
                Style::default().fg(theme.user).add_modifier(Modifier::BOLD),
            ));
            for line in text.lines() {
                lines.push(Line::raw(format!("  {line}")));
            }
        }
        ChatMessageKind::AgentText(text) => {
            lines.push(Line::styled(
                "Kiro:",
                Style::default()
                    .fg(theme.agent)
                    .add_modifier(Modifier::BOLD),
            ));
            let md_lines = markdown::render_with_theme(text, width, theme);
            lines.extend(md_lines);
        }
        ChatMessageKind::Thought(text) => {
            push_thought_lines(lines, text, theme);
        }
        ChatMessageKind::ToolCall(tc) => {
            render_tool_call(lines, tc, theme);
        }
        ChatMessageKind::Plan(plan) => {
            lines.push(Line::styled(
                "Plan:",
                Style::default()
                    .fg(theme.emphasis)
                    .add_modifier(Modifier::BOLD),
            ));
            for entry in plan.entries() {
                let icon = match entry.status() {
                    cyril_core::types::PlanEntryStatus::Pending => "○",
                    cyril_core::types::PlanEntryStatus::InProgress => "◐",
                    cyril_core::types::PlanEntryStatus::Completed => "●",
                    cyril_core::types::PlanEntryStatus::Failed => "✗",
                };
                lines.push(Line::raw(format!("  {icon} {}", entry.title())));
            }
        }
        ChatMessageKind::System(text) => {
            let style = Style::default()
                .fg(theme.system)
                .add_modifier(Modifier::ITALIC);
            for line in text.lines() {
                lines.push(Line::styled(line.to_string(), style));
            }
        }
        ChatMessageKind::CommandOutput { command, text } => {
            lines.push(Line::styled(
                format!("/{command}:"),
                Style::default()
                    .fg(theme.soft_accent)
                    .add_modifier(Modifier::BOLD),
            ));
            for line in text.lines() {
                lines.push(Line::raw(format!("  {line}")));
            }
        }
        ChatMessageKind::SteerEcho { text, status } => {
            let (suffix, color) = match status {
                SteerEchoStatus::Queued => ("queued", theme.emphasis),
                SteerEchoStatus::Applied => ("applied", theme.positive_accent),
                SteerEchoStatus::Cleared => ("cleared", theme.subdued),
                SteerEchoStatus::Unsupported => ("not supported", theme.subdued_negative),
            };
            let style = Style::default().fg(color).add_modifier(Modifier::ITALIC);
            // Steers are short; render the first line with the status suffix and
            // indent any continuation lines under it.
            let mut lines_iter = text.lines();
            let first = lines_iter.next().unwrap_or("");
            lines.push(Line::styled(
                format!("  ↳ steer: {first} — {suffix}"),
                style,
            ));
            for line in lines_iter {
                lines.push(Line::styled(format!("    {line}"), style));
            }
        }
    }
}

/// Render a live activity indicator at the bottom of chat content.
/// Shows a spinner + label + elapsed time when the agent is busy but not
/// actively streaming text (which is already visible).
fn render_activity_indicator(lines: &mut Vec<Line>, state: &dyn TuiState, theme: &Theme) {
    use crate::traits::Activity;

    let (label, color) = match state.activity() {
        Activity::Sending | Activity::Waiting => ("Thinking...", theme.muted),
        Activity::ToolRunning => ("Running...", theme.accent_quinary),
        // Streaming text is already visible — no indicator needed.
        Activity::Streaming | Activity::Idle | Activity::Ready => return,
    };

    let elapsed_dur = state.activity_elapsed();
    let elapsed_secs = elapsed_dur.map(|d| d.as_secs()).unwrap_or(0);
    let spinner_idx = elapsed_dur
        .map(|d| (d.as_millis() / SPINNER_FRAME_MS) as usize % SPINNER_CHARS.len())
        .unwrap_or(0);

    lines.push(Line::from(vec![
        Span::styled(
            format!("{} ", SPINNER_CHARS[spinner_idx]),
            Style::default().fg(color),
        ),
        Span::styled(
            format!("{label} {elapsed_secs}s"),
            Style::default().fg(color),
        ),
    ]));
}

fn render_tool_call(lines: &mut Vec<Line>, tc: &TrackedToolCall, theme: &Theme) {
    use cyril_core::types::{ToolCallStatus, ToolKind};

    let status_icon = match tc.status() {
        ToolCallStatus::InProgress => "⟳",
        ToolCallStatus::Pending => "⏳",
        ToolCallStatus::Completed => "✓",
        ToolCallStatus::Failed => "✗",
    };

    let label = match tc.kind() {
        ToolKind::Read => {
            if let Some(path) = tc.primary_path() {
                format!("Read({path})")
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Write => {
            if let Some(path) = tc.primary_path() {
                format!("Edit({path})")
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Execute => {
            if let Some(cmd) = tc.command_text() {
                let mut chars = cmd.chars();
                let display: String = chars.by_ref().take(50).collect();
                if chars.next().is_some() {
                    format!("Run({display}...)")
                } else {
                    format!("Run({display})")
                }
            } else {
                tc.title().to_string()
            }
        }
        ToolKind::Search => tc.title().to_string(),
        ToolKind::Think => "Thinking...".to_string(),
        ToolKind::Fetch => tc.title().to_string(),
        ToolKind::SwitchMode => tc.title().to_string(),
        ToolKind::Other => tc.title().to_string(),
    };

    let color = match tc.status() {
        ToolCallStatus::Completed => theme.subdued_positive,
        ToolCallStatus::Failed => theme.subdued_negative,
        ToolCallStatus::InProgress | ToolCallStatus::Pending => theme.emphasis,
    };

    let kind_color = match tc.kind() {
        ToolKind::Read => theme.accent_tertiary,
        ToolKind::Write => theme.accent_quaternary,
        ToolKind::Execute => theme.emphasis,
        ToolKind::Search | ToolKind::Fetch => theme.accent_quinary,
        ToolKind::Think => theme.subdued,
        ToolKind::SwitchMode => theme.accent_quaternary,
        ToolKind::Other => theme.text,
    };

    let mut header_spans = vec![
        Span::styled(format!("{status_icon} "), Style::default().fg(color)),
        Span::styled(label, Style::default().fg(kind_color)),
    ];

    if let Some((added, removed)) = compute_diff_summary(tc) {
        header_spans.push(Span::styled(
            format!("  +{added} -{removed}"),
            Style::default().fg(theme.subdued),
        ));
    }

    lines.push(Line::from(header_spans));

    if tc.status() == ToolCallStatus::Completed && tc.kind() == ToolKind::Write {
        render_diff_lines(lines, tc, theme);
    }

    render_tool_output(lines, tc, theme);
}

/// Compute (added, removed) line counts from diff content using `similar`.
fn compute_diff_summary(tc: &TrackedToolCall) -> Option<(usize, usize)> {
    use similar::{ChangeTag, TextDiff};

    for content in tc.content() {
        if let cyril_core::types::ToolCallContent::Diff {
            old_text, new_text, ..
        } = content
        {
            let old = old_text.as_deref().unwrap_or("");
            let diff = TextDiff::from_lines(old, new_text);
            let mut added = 0usize;
            let mut removed = 0usize;
            for change in diff.iter_all_changes() {
                match change.tag() {
                    ChangeTag::Insert => added += 1,
                    ChangeTag::Delete => removed += 1,
                    ChangeTag::Equal => {}
                }
            }
            if added > 0 || removed > 0 {
                return Some((added, removed));
            }
        }
    }
    None
}

/// Render actual diff lines with line numbers for edit operations.
/// Uses the `similar` crate for proper diff computation with context lines.
fn render_diff_lines(lines: &mut Vec<Line>, tc: &TrackedToolCall, theme: &Theme) {
    use similar::{ChangeTag, TextDiff};

    const MAX_DIFF_LINES: usize = 20;

    for content in tc.content() {
        if let cyril_core::types::ToolCallContent::Diff {
            old_text, new_text, ..
        } = content
        {
            let old = old_text.as_deref().unwrap_or("");
            let diff = TextDiff::from_lines(old, new_text);
            let mut count = 0;

            for group in diff.grouped_ops(1) {
                for op in &group {
                    for change in diff.iter_changes(op) {
                        if count >= MAX_DIFF_LINES {
                            lines.push(Line::styled(
                                "      ...".to_string(),
                                Style::default().fg(theme.subdued),
                            ));
                            return;
                        }

                        let line_text = change.value().trim_end_matches('\n');

                        let (prefix, color) = match change.tag() {
                            ChangeTag::Delete => {
                                let line_no = change.old_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │- "), theme.subdued_negative)
                            }
                            ChangeTag::Insert => {
                                let line_no = change.new_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │+ "), theme.subdued_positive)
                            }
                            ChangeTag::Equal => {
                                let line_no = change.new_index().map(|i| i + 1).unwrap_or(0);
                                (format!("    {line_no:>4} │  "), theme.subdued)
                            }
                        };

                        lines.push(Line::from(vec![
                            Span::styled(prefix, Style::default().fg(color)),
                            Span::styled(
                                line_text.to_string(),
                                if change.tag() == ChangeTag::Equal {
                                    Style::default().fg(theme.subdued)
                                } else {
                                    Style::default().fg(color)
                                },
                            ),
                        ]));

                        count += 1;
                    }
                }
            }

            // Only render first diff block
            return;
        }
    }
}

/// Render tool output (shell stdout, errors, file read summary).
///
/// Called after the header and diff rendering in `render_tool_call`. Skips
/// Write-kind tools since they already display diff content.
fn render_tool_output(lines: &mut Vec<Line>, tc: &TrackedToolCall, theme: &Theme) {
    use cyril_core::types::{ToolCallStatus, ToolKind};

    const MAX_OUTPUT_LINES: usize = 5;
    const INDENT: &str = "    ";

    // Failed tools: show error message
    if tc.status() == ToolCallStatus::Failed {
        if let Some(err) = tc.error_message() {
            lines.push(Line::styled(
                format!("{INDENT}Error: {err}"),
                Style::default().fg(theme.subdued_negative),
            ));
        }
        return;
    }

    // Only show output for completed tools
    if tc.status() != ToolCallStatus::Completed {
        return;
    }

    // Write tools already show diff content — skip output rendering
    if tc.kind() == ToolKind::Write {
        return;
    }

    // Execute: show exit code if non-zero
    if let Some(code) = tc.exit_code()
        && code != 0
    {
        lines.push(Line::styled(
            format!("{INDENT}Exit: {code}"),
            Style::default().fg(theme.emphasis),
        ));
    }

    // Read: show char count summary instead of full output
    if tc.kind() == ToolKind::Read {
        if let Some(text) = tc.output_text() {
            let chars = text.chars().count();
            let summary = if chars < 1000 {
                format!("{chars} chars")
            } else {
                format!("{:.1}k chars", chars as f64 / 1000.0)
            };
            lines.push(Line::styled(
                format!("{INDENT}{summary}"),
                Style::default().fg(theme.subdued),
            ));
        }
        return;
    }

    // Other tools: show output preview
    if let Some(text) = tc.output_text() {
        let output_lines: Vec<&str> = text.lines().collect();
        let total = output_lines.len();
        if total == 0 {
            return;
        }

        let show = total.min(MAX_OUTPUT_LINES);
        for line_text in &output_lines[..show] {
            lines.push(Line::styled(
                format!("{INDENT}| {line_text}"),
                Style::default().fg(theme.subdued),
            ));
        }
        if total > show {
            lines.push(Line::styled(
                format!("{INDENT}...{} more lines", total - show),
                Style::default().fg(theme.subdued),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use std::time::Duration;

    use crate::traits::test_support::MockTuiState;
    use crate::traits::{Activity, ChatMessage};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    const EXPECTED_SHAPE_LABELS: [&str; 44] = [
        "message/user",
        "message/agent",
        "message/thought",
        "message/tool",
        "message/plan",
        "message/system",
        "message/command",
        "message/steer",
        "steer/queued",
        "steer/applied",
        "steer/cleared",
        "steer/unsupported",
        "activity/sending",
        "activity/waiting",
        "activity/tool-running",
        "activity/streaming",
        "activity/idle",
        "activity/ready",
        "tool-kind/read",
        "tool-kind/write",
        "tool-kind/execute",
        "tool-kind/search",
        "tool-kind/think",
        "tool-kind/fetch",
        "tool-kind/switch-mode",
        "tool-kind/other",
        "tool-status/in-progress",
        "tool-status/pending",
        "tool-status/completed",
        "tool-status/failed",
        "optional/location-present",
        "optional/location-absent",
        "optional/raw-input-present",
        "optional/raw-input-absent",
        "optional/content-present",
        "optional/content-absent",
        "optional/raw-output-present",
        "optional/raw-output-absent",
        "optional/old-text-present",
        "optional/old-text-absent",
        "optional/error-present",
        "optional/error-absent",
        "truncation/diff-20",
        "truncation/output-5",
    ];

    fn matrix_tool(
        id: &str,
        title: &str,
        kind: cyril_core::types::ToolKind,
        status: cyril_core::types::ToolCallStatus,
    ) -> TrackedToolCall {
        use cyril_core::types::{ToolCall, ToolCallId};

        TrackedToolCall::new(ToolCall::new(
            ToolCallId::new(id),
            title.into(),
            kind,
            status,
            None,
        ))
    }

    fn rendered_message_text(message: &ChatMessage, theme: &Theme) -> String {
        let mut lines = Vec::new();
        render_message(&mut lines, message, 80, theme);
        lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn rendered_tool_lines(tool: &TrackedToolCall, theme: &Theme) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        render_tool_call(&mut lines, tool, theme);
        lines
    }

    fn chat_shape_matrix() -> anyhow::Result<Vec<&'static str>> {
        use cyril_core::types::{
            Plan, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
        };

        macro_rules! record {
            ($passes:ident, $label:literal, $condition:expr) => {{
                anyhow::ensure!($condition, "shape {} failed", $label);
                $passes.push($label);
            }};
        }

        let theme = crate::traits::test_support::marker_theme();
        let mut passes = Vec::with_capacity(EXPECTED_SHAPE_LABELS.len());
        let basic_tool = matrix_tool(
            "message-tool",
            "tool",
            ToolKind::Other,
            ToolCallStatus::Pending,
        );
        let steer = |status| ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: "steer".into(),
                status,
            },
            timestamp: std::time::Instant::now(),
        };
        let messages = [
            (ChatMessage::user_text("user".into()), "You:"),
            (ChatMessage::agent_text("agent".into()), "Kiro:"),
            (ChatMessage::thought("thought".into()), "thought"),
            (ChatMessage::tool_call(basic_tool), "tool"),
            (ChatMessage::plan(Plan::new(Vec::new())), "Plan:"),
            (ChatMessage::system("system".into()), "system"),
            (
                ChatMessage::command_output("command".into(), "output".into()),
                "/command:",
            ),
            (steer(SteerEchoStatus::Queued), "steer"),
        ];
        for ((message, expected), label) in messages.into_iter().zip(&EXPECTED_SHAPE_LABELS[..8]) {
            let text = rendered_message_text(&message, &theme);
            anyhow::ensure!(text.contains(expected), "shape {label} failed: {text}");
            passes.push(*label);
        }

        for (status, suffix, label) in [
            (SteerEchoStatus::Queued, "queued", "steer/queued"),
            (SteerEchoStatus::Applied, "applied", "steer/applied"),
            (SteerEchoStatus::Cleared, "cleared", "steer/cleared"),
            (
                SteerEchoStatus::Unsupported,
                "not supported",
                "steer/unsupported",
            ),
        ] {
            let text = rendered_message_text(&steer(status), &theme);
            anyhow::ensure!(text.contains(suffix), "shape {label} failed: {text}");
            passes.push(label);
        }

        for (activity, visible, label) in [
            (Activity::Sending, true, "activity/sending"),
            (Activity::Waiting, true, "activity/waiting"),
            (Activity::ToolRunning, true, "activity/tool-running"),
            (Activity::Streaming, false, "activity/streaming"),
            (Activity::Idle, false, "activity/idle"),
            (Activity::Ready, false, "activity/ready"),
        ] {
            let state = MockTuiState {
                activity,
                activity_elapsed: Some(Duration::from_secs(1)),
                ..Default::default()
            };
            let mut lines = Vec::new();
            render_activity_indicator(&mut lines, &state, &theme);
            anyhow::ensure!(
                lines.is_empty() != visible,
                "shape {label} visibility failed"
            );
            passes.push(label);
        }

        for (kind, expected, label) in [
            (ToolKind::Read, "shape", "tool-kind/read"),
            (ToolKind::Write, "shape", "tool-kind/write"),
            (ToolKind::Execute, "shape", "tool-kind/execute"),
            (ToolKind::Search, "shape", "tool-kind/search"),
            (ToolKind::Think, "Thinking...", "tool-kind/think"),
            (ToolKind::Fetch, "shape", "tool-kind/fetch"),
            (ToolKind::SwitchMode, "shape", "tool-kind/switch-mode"),
            (ToolKind::Other, "shape", "tool-kind/other"),
        ] {
            let lines = rendered_tool_lines(
                &matrix_tool("kind", "shape", kind, ToolCallStatus::Pending),
                &theme,
            );
            let label_text = lines[0].spans[1].content.as_ref();
            anyhow::ensure!(
                label_text == expected,
                "shape {label} expected {expected:?}, got {label_text:?}"
            );
            passes.push(label);
        }

        for (status, icon, label) in [
            (ToolCallStatus::InProgress, "⟳ ", "tool-status/in-progress"),
            (ToolCallStatus::Pending, "⏳ ", "tool-status/pending"),
            (ToolCallStatus::Completed, "✓ ", "tool-status/completed"),
            (ToolCallStatus::Failed, "✗ ", "tool-status/failed"),
        ] {
            let lines = rendered_tool_lines(
                &matrix_tool("status", "status", ToolKind::Other, status),
                &theme,
            );
            let actual = lines[0].spans[0].content.as_ref();
            anyhow::ensure!(
                actual == icon,
                "shape {label} expected {icon:?}, got {actual:?}"
            );
            passes.push(label);
        }

        let location_present = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("location-present"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Pending,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "file.rs".into(),
                line: Some(7),
            }]),
        );
        let location_absent = matrix_tool(
            "location-absent",
            "read",
            ToolKind::Read,
            ToolCallStatus::Pending,
        );
        record!(
            passes,
            "optional/location-present",
            rendered_tool_lines(&location_present, &theme)[0].spans[1].content == "Read(file.rs)"
        );
        record!(
            passes,
            "optional/location-absent",
            rendered_tool_lines(&location_absent, &theme)[0].spans[1].content == "read"
        );

        let raw_input_present = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("input-present"),
            "execute".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            Some(serde_json::json!({"command": "cargo test"})),
        ));
        let raw_input_absent = matrix_tool(
            "input-absent",
            "execute",
            ToolKind::Execute,
            ToolCallStatus::Pending,
        );
        record!(
            passes,
            "optional/raw-input-present",
            rendered_tool_lines(&raw_input_present, &theme)[0].spans[1].content
                == "Run(cargo test)"
        );
        record!(
            passes,
            "optional/raw-input-absent",
            rendered_tool_lines(&raw_input_absent, &theme)[0].spans[1].content == "execute"
        );

        let content_present = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("content-present"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "file.rs".into(),
                old_text: Some("old\n".into()),
                new_text: "new\n".into(),
            }]),
        );
        let content_absent = matrix_tool(
            "content-absent",
            "write",
            ToolKind::Write,
            ToolCallStatus::Completed,
        );
        record!(
            passes,
            "optional/content-present",
            rendered_tool_lines(&content_present, &theme).len() > 1
        );
        record!(
            passes,
            "optional/content-absent",
            rendered_tool_lines(&content_absent, &theme).len() == 1
        );

        let raw_output_present = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("output-present"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({"stdout": "output"}))),
        );
        let raw_output_absent = matrix_tool(
            "output-absent",
            "execute",
            ToolKind::Execute,
            ToolCallStatus::Completed,
        );
        record!(
            passes,
            "optional/raw-output-present",
            rendered_tool_lines(&raw_output_present, &theme)
                .iter()
                .any(|line| line.to_string().contains("output"))
        );
        record!(
            passes,
            "optional/raw-output-absent",
            rendered_tool_lines(&raw_output_absent, &theme).len() == 1
        );

        let diff_with_old = content_present;
        let diff_without_old = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("old-absent"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "file.rs".into(),
                old_text: None,
                new_text: "new\n".into(),
            }]),
        );
        let with_old_text = rendered_tool_lines(&diff_with_old, &theme)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let without_old_text = rendered_tool_lines(&diff_without_old, &theme)
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        record!(
            passes,
            "optional/old-text-present",
            with_old_text.contains("│- old")
        );
        record!(
            passes,
            "optional/old-text-absent",
            !without_old_text.contains("│-") && without_old_text.contains("│+ new")
        );

        let error_present = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("error-present"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Failed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({"message": "boom"}))),
        );
        let error_absent = matrix_tool(
            "error-absent",
            "execute",
            ToolKind::Execute,
            ToolCallStatus::Failed,
        );
        record!(
            passes,
            "optional/error-present",
            rendered_tool_lines(&error_present, &theme)
                .iter()
                .any(|line| line.to_string().contains("boom"))
        );
        record!(
            passes,
            "optional/error-absent",
            rendered_tool_lines(&error_absent, &theme).len() == 1
        );

        let old_text = (0..21)
            .map(|index| format!("old-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let new_text = (0..21)
            .map(|index| format!("new-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let large_diff = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("diff-limit"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "large.rs".into(),
                old_text: Some(old_text),
                new_text,
            }]),
        );
        let diff_lines = rendered_tool_lines(&large_diff, &theme);
        record!(
            passes,
            "truncation/diff-20",
            diff_lines.len() == 22
                && diff_lines
                    .last()
                    .is_some_and(|line| line.to_string().contains("..."))
        );

        let output_six = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("output-limit"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "line-1\nline-2\nline-3\nline-4\nline-5\nline-6",
                "exit_status": 0
            }))),
        );
        let output_lines = rendered_tool_lines(&output_six, &theme);
        let output_text = output_lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        record!(
            passes,
            "truncation/output-5",
            output_lines.len() == 7
                && output_text.contains("line-5")
                && !output_text.contains("line-6")
                && output_text.contains("...1 more lines")
        );

        Ok(passes)
    }

    #[test]
    fn every_chat_and_tool_input_shape_is_fenced() -> anyhow::Result<()> {
        let passes = chat_shape_matrix()?;
        assert_eq!(passes, EXPECTED_SHAPE_LABELS);
        Ok(())
    }

    #[test]
    fn chat_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
            })
            .expect("draw");
    }

    #[test]
    fn chat_renders_messages() {
        let state = MockTuiState {
            messages: vec![
                ChatMessage::user_text("Hello".into()),
                ChatMessage::agent_text("Hi there!".into()),
                ChatMessage::system("Session started".into()),
            ],
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
            })
            .expect("draw");
    }

    fn render_markdown_case(committed: bool, theme: &Theme) -> (String, Color) {
        let state = if committed {
            MockTuiState {
                theme: *theme,
                messages: vec![ChatMessage::agent_text("# THEMED".into())],
                ..Default::default()
            }
        } else {
            MockTuiState {
                theme: *theme,
                streaming_text: "# THEMED".into(),
                ..Default::default()
            }
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, theme))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let symbols = buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        let foreground = buffer
            .content()
            .iter()
            .find(|cell| cell.symbol() == "T")
            .map(|cell| cell.fg)
            .expect("Markdown heading cell");
        (symbols, foreground)
    }

    #[test]
    fn committed_and_streaming_markdown_use_frame_theme_without_cache_leaks() {
        let marker = crate::traits::test_support::marker_theme();
        let no_color = crate::theme::resolve(
            crate::theme::ThemeId::CyrilDark,
            crate::theme::ColorMode::None,
        );

        for (committed, label) in [(true, "committed"), (false, "streaming")] {
            let (marker_symbols, marker_fg) = render_markdown_case(committed, &marker);
            let (plain_symbols, plain_fg) = render_markdown_case(committed, &no_color);
            let (warm_symbols, warm_fg) = render_markdown_case(committed, &marker);
            assert_eq!(marker_symbols, plain_symbols, "{label} symbols changed");
            assert_eq!(marker_symbols, warm_symbols, "{label} warm-cache symbols");
            assert_eq!(marker_fg, marker.accent_quinary, "{label} marker role");
            assert_eq!(plain_fg, Color::Reset, "{label} no-color role");
            assert_eq!(warm_fg, marker.accent_quinary, "{label} warm-cache role");
        }
    }

    #[test]
    fn user_identity_uses_frame_theme() {
        let state = MockTuiState {
            messages: vec![ChatMessage::user_text("hello".into())],
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");

        let cell = terminal
            .backend()
            .buffer()
            .cell((0, 0))
            .expect("user label cell");
        assert_eq!(cell.symbol(), "Y");
        assert_eq!(cell.fg, state.theme.user);
    }

    #[test]
    fn every_non_tool_message_uses_its_marker_role() {
        use cyril_core::types::Plan;

        let theme = crate::traits::test_support::marker_theme();
        let steer = |status| ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: "steer".into(),
                status,
            },
            timestamp: std::time::Instant::now(),
        };
        let cases = [
            (ChatMessage::user_text("user".into()), theme.user),
            (ChatMessage::agent_text(String::new()), theme.agent),
            (ChatMessage::thought("thought".into()), theme.muted),
            (ChatMessage::plan(Plan::new(Vec::new())), theme.emphasis),
            (ChatMessage::system("system".into()), theme.system),
            (
                ChatMessage::command_output("command".into(), String::new()),
                theme.soft_accent,
            ),
            (steer(SteerEchoStatus::Queued), theme.emphasis),
            (steer(SteerEchoStatus::Applied), theme.positive_accent),
            (steer(SteerEchoStatus::Cleared), theme.subdued),
            (steer(SteerEchoStatus::Unsupported), theme.subdued_negative),
        ];

        for (message, expected) in cases {
            let mut lines = Vec::new();
            render_message(&mut lines, &message, 80, &theme);
            assert_eq!(
                lines.first().and_then(|line| line.style.fg),
                Some(expected),
                "wrong role for {:?}",
                message.kind()
            );
        }
    }

    #[test]
    fn every_activity_uses_its_marker_role_or_stays_hidden() {
        let theme = crate::traits::test_support::marker_theme();
        for (activity, expected) in [
            (Activity::Sending, Some(theme.muted)),
            (Activity::Waiting, Some(theme.muted)),
            (Activity::ToolRunning, Some(theme.accent_quinary)),
            (Activity::Streaming, None),
            (Activity::Idle, None),
            (Activity::Ready, None),
        ] {
            let state = MockTuiState {
                activity,
                ..Default::default()
            };
            let mut lines = Vec::new();
            render_activity_indicator(&mut lines, &state, &theme);
            let actual = lines
                .first()
                .and_then(|line| line.spans.first())
                .and_then(|span| span.style.fg);
            assert_eq!(actual, expected, "wrong activity role for {activity:?}");
        }
    }

    #[test]
    fn tool_header_uses_marker_status_and_kind_roles() {
        use cyril_core::types::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

        let theme = crate::traits::test_support::marker_theme();
        let tool = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("marker-tool"),
            "Read fixture".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        ));
        let mut lines = Vec::new();
        render_tool_call(&mut lines, &tool, &theme);

        assert_eq!(lines[0].spans[0].style.fg, Some(theme.subdued_positive));
        assert_eq!(lines[0].spans[1].style.fg, Some(theme.accent_tertiary));
    }

    #[test]
    fn every_tool_status_and_kind_uses_its_marker_role() {
        use cyril_core::types::{ToolCall, ToolCallId, ToolCallStatus, ToolKind};

        let theme = crate::traits::test_support::marker_theme();
        for (status, expected) in [
            (ToolCallStatus::InProgress, theme.emphasis),
            (ToolCallStatus::Pending, theme.emphasis),
            (ToolCallStatus::Completed, theme.subdued_positive),
            (ToolCallStatus::Failed, theme.subdued_negative),
        ] {
            let tool = TrackedToolCall::new(ToolCall::new(
                ToolCallId::new("status"),
                "status".into(),
                ToolKind::Other,
                status,
                None,
            ));
            let mut lines = Vec::new();
            render_tool_call(&mut lines, &tool, &theme);
            assert_eq!(lines[0].spans[0].style.fg, Some(expected));
        }

        for (kind, expected) in [
            (ToolKind::Read, theme.accent_tertiary),
            (ToolKind::Write, theme.accent_quaternary),
            (ToolKind::Execute, theme.emphasis),
            (ToolKind::Search, theme.accent_quinary),
            (ToolKind::Think, theme.subdued),
            (ToolKind::Fetch, theme.accent_quinary),
            (ToolKind::SwitchMode, theme.accent_quaternary),
            (ToolKind::Other, theme.text),
        ] {
            let tool = TrackedToolCall::new(ToolCall::new(
                ToolCallId::new("kind"),
                "kind".into(),
                kind,
                ToolCallStatus::Completed,
                None,
            ));
            let mut lines = Vec::new();
            render_tool_call(&mut lines, &tool, &theme);
            assert_eq!(lines[0].spans[1].style.fg, Some(expected));
        }
    }

    #[test]
    fn tool_scene_shape_matches_pinned_baseline() -> anyhow::Result<()> {
        use cyril_core::types::{
            ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
        };

        let make_tool = |id, title: &str, kind, status| {
            TrackedToolCall::new(ToolCall::new(
                ToolCallId::new(id),
                title.to_string(),
                kind,
                status,
                None,
            ))
        };
        let old_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("old-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let new_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("new-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let write = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("write"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "diff.rs".into(),
                old_text: Some(old_text),
                new_text,
            }])
            .with_locations(vec![ToolCallLocation {
                path: "diff.rs".into(),
                line: Some(1),
            }]),
        );
        let read = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("read"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Pending,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "read.rs".into(),
                line: None,
            }]),
        );
        let execute = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("execute"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "line-1\nline-2\nline-3\nline-4\nline-5\nline-6",
                "exit_status": 1
            }))),
        );
        let left_state = MockTuiState {
            messages: vec![ChatMessage::tool_call(write)],
            ..Default::default()
        };
        let right_state = MockTuiState {
            messages: vec![
                ChatMessage::tool_call(read),
                ChatMessage::tool_call(execute),
                ChatMessage::tool_call(make_tool(
                    "search",
                    "Search(marker)",
                    ToolKind::Search,
                    ToolCallStatus::InProgress,
                )),
                ChatMessage::tool_call(make_tool(
                    "think",
                    "think",
                    ToolKind::Think,
                    ToolCallStatus::Failed,
                )),
                ChatMessage::tool_call(make_tool(
                    "fetch",
                    "Fetch(url)",
                    ToolKind::Fetch,
                    ToolCallStatus::Pending,
                )),
                ChatMessage::tool_call(make_tool(
                    "switch",
                    "Switch(mode)",
                    ToolKind::SwitchMode,
                    ToolCallStatus::Completed,
                )),
                ChatMessage::tool_call(make_tool(
                    "other",
                    "Other(custom)",
                    ToolKind::Other,
                    ToolCallStatus::Failed,
                )),
            ],
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| {
            let [left, right] =
                Layout::horizontal([Constraint::Length(40), Constraint::Length(40)])
                    .areas(frame.area());
            render(frame, left, &left_state, &left_state.theme);
            render(frame, right, &right_state, &right_state.theme);
        })?;

        let expected = include_str!("../../tests/fixtures/conversation-theme-baseline.tsv")
            .lines()
            .skip(2)
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                (fields.first() == Some(&"tools")).then_some(fields)
            })
            .map(|fields| {
                Ok((
                    fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing tool symbol"))?
                        .to_string(),
                    fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing tool modifier"))?
                        .parse::<u16>()?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let actual = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| {
                let mut symbol = String::with_capacity(cell.symbol().len() * 2);
                for byte in cell.symbol().as_bytes() {
                    symbol.push(HEX[(byte >> 4) as usize] as char);
                    symbol.push(HEX[(byte & 0x0f) as usize] as char);
                }
                (symbol, cell.modifier.bits())
            })
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), 1_920);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn push_thought_lines_renders_multiline_with_marker_and_indent() {
        // A multi-line thought block: 💭 on the first row, continuation rows
        // indented under it (a single Line would not break on the \n).
        let mut lines: Vec<Line> = Vec::new();
        push_thought_lines(
            &mut lines,
            "first line\nsecond line",
            &crate::traits::test_support::marker_theme(),
        );
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].to_string(), "💭 first line");
        assert_eq!(lines[1].to_string(), "   second line");
    }

    #[test]
    fn push_thought_lines_keeps_marker_for_empty_preview() {
        // Empty live preview (thinking started, no token yet) must still show the
        // 💭 placeholder rather than nothing.
        let mut lines: Vec<Line> = Vec::new();
        push_thought_lines(&mut lines, "", &crate::traits::test_support::marker_theme());
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].to_string(), "💭 ");
    }

    #[test]
    fn render_tool_call_with_diff_shows_line_numbers() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Editing main.rs".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "src/main.rs".into(),
                old_text: Some("fn main() {\n    println!(\"old\");\n}\n".into()),
                new_text: "fn main() {\n    println!(\"new\");\n    println!(\"extra\");\n}\n"
                    .into(),
            }])
            .with_locations(vec![ToolCallLocation {
                path: "src/main.rs".into(),
                line: Some(1),
            }]),
        );

        let theme = crate::traits::test_support::marker_theme();
        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc, &theme);

        // Header should have label and diff summary
        let header = lines[0].to_string();
        assert!(
            header.contains("Edit"),
            "header should contain Edit label: {header}"
        );
        assert!(
            header.contains("+"),
            "header should contain diff summary: {header}"
        );

        // Should have diff lines with line numbers and │ separator
        let diff_lines: Vec<String> = lines[1..].iter().map(|l| l.to_string()).collect();
        let has_gutter = diff_lines.iter().any(|l| l.contains('│'));
        assert!(
            has_gutter,
            "diff lines should have │ gutter separator: {diff_lines:?}"
        );

        let has_add = diff_lines.iter().any(|l| l.contains("│+"));
        let has_del = diff_lines.iter().any(|l| l.contains("│-"));
        assert!(has_add, "should have added lines: {diff_lines:?}");
        assert!(has_del, "should have removed lines: {diff_lines:?}");
        assert_eq!(lines[0].spans[0].style.fg, Some(theme.subdued_positive));
        assert_eq!(lines[0].spans[1].style.fg, Some(theme.accent_quaternary));
        assert_eq!(lines[0].spans[2].style.fg, Some(theme.subdued));
        assert!(lines[1..].iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.contains("│-") && span.style.fg == Some(theme.subdued_negative)
            })
        }));
        assert!(lines[1..].iter().any(|line| {
            line.spans.iter().any(|span| {
                span.content.contains("│+") && span.style.fg == Some(theme.subdued_positive)
            })
        }));
        assert!(lines[1..].iter().any(|line| {
            line.spans
                .iter()
                .any(|span| span.content.contains("│  ") && span.style.fg == Some(theme.subdued))
        }));
    }

    #[test]
    fn render_tool_call_diff_summary_counts() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "file.rs".into(),
                old_text: Some("line1\nline2\nline3\n".into()),
                new_text: "line1\nchanged\nline3\nnew_line\n".into(),
            }]),
        );

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );

        // Header should show +2 -1 (one changed + one added = 2 inserts, 1 delete)
        let header = lines[0].to_string();
        assert!(header.contains('+'), "should show additions: {header}");
        assert!(header.contains('-'), "should show removals: {header}");
    }

    #[test]
    fn render_tool_call_no_diff_for_read() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_1"),
            "Reading file.rs".into(),
            ToolKind::Read,
            ToolCallStatus::Completed,
            None,
        ));

        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );

        // Read tool calls should only have a header, no diff lines
        assert_eq!(
            lines.len(),
            1,
            "read tool call should only have header line"
        );
    }

    #[test]
    fn render_tool_call_diff_respects_max_lines() {
        use cyril_core::types::*;

        // Create a large diff that exceeds MAX_DIFF_LINES
        let old_text: String = (0..30).map(|i| format!("old line {i}\n")).collect();
        let new_text: String = (0..30).map(|i| format!("new line {i}\n")).collect();

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "big.rs".into(),
                old_text: Some(old_text),
                new_text,
            }]),
        );

        let theme = crate::traits::test_support::marker_theme();
        let mut lines: Vec<Line> = Vec::new();
        render_tool_call(&mut lines, &tc, &theme);

        // Should have header + at most 20 diff lines + "..." overflow
        let last_line = lines.last().map(|l| l.to_string()).unwrap_or_default();
        assert!(
            last_line.contains("..."),
            "large diff should show overflow indicator: {last_line}"
        );
        // Total lines should be capped (header + <=21 diff lines including overflow)
        assert!(
            lines.len() <= 23,
            "should be capped, got {} lines",
            lines.len()
        );
        assert_eq!(
            lines.last().and_then(|line| line.style.fg),
            Some(theme.subdued)
        );
    }

    #[test]
    fn render_tool_call_smart_labels() {
        use cyril_core::types::*;

        // Read with location
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "src/main.rs".into(),
                line: None,
            }]),
        );
        let mut lines = Vec::new();
        render_tool_call(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let header = lines[0].to_string();
        assert!(
            header.contains("Read(src/main.rs)"),
            "should show Read(path): {header}"
        );

        // Execute with command
        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_2"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            Some(serde_json::json!({"command": "cargo test"})),
        ));
        let mut lines = Vec::new();
        render_tool_call(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let header = lines[0].to_string();
        assert!(
            header.contains("Run(cargo test)"),
            "should show Run(cmd): {header}"
        );
    }

    #[test]
    fn execute_label_does_not_ellipsize_fewer_than_fifty_unicode_characters() {
        use cyril_core::types::*;

        let command = "界".repeat(20);
        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_unicode"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::Completed,
            Some(serde_json::json!({"command": command})),
        ));
        let mut lines = Vec::new();
        render_tool_call(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );

        assert_eq!(lines[0].spans[1].content, format!("Run({command})"));
    }

    #[test]
    fn steer_echo_renders_distinct_suffix_per_status() {
        use crate::traits::{ChatMessage, ChatMessageKind, SteerEchoStatus};

        // Stress fixture (Slice 1): one render per status, Unicode + arrow text.
        // Bug classes: ASCII assumption (Unicode must not panic/mangle) and a
        // tie-break that never fires (Applied suffix must differ from Queued).
        let statuses = [
            (SteerEchoStatus::Queued, "queued"),
            (SteerEchoStatus::Applied, "applied"),
            (SteerEchoStatus::Cleared, "cleared"),
            (SteerEchoStatus::Unsupported, "not supported"),
        ];
        let mut rendered = Vec::new();
        for (status, expected) in statuses {
            let msg = ChatMessage {
                kind: ChatMessageKind::SteerEcho {
                    text: "café→ stop".into(),
                    status,
                },
                timestamp: std::time::Instant::now(),
            };
            let mut lines = Vec::new();
            render_message(
                &mut lines,
                &msg,
                80,
                &crate::traits::test_support::marker_theme(),
            );
            let text = lines[0].to_string();
            assert!(
                text.contains("steer: café→ stop"),
                "Unicode steer text must render intact: {text:?}"
            );
            assert!(
                text.contains(expected),
                "status {status:?} must render suffix {expected:?}: {text:?}"
            );
            rendered.push(text);
        }
        assert_ne!(
            rendered[0], rendered[1],
            "Queued and Applied must render distinctly (tie-break wired)"
        );
        let unique: std::collections::HashSet<_> = rendered.iter().collect();
        assert_eq!(
            unique.len(),
            4,
            "all four statuses must render distinctly: {rendered:?}"
        );
    }

    #[test]
    fn chat_renders_interleaved_text_and_tool_calls_in_order() {
        use cyril_core::types::*;

        // Simulate a committed turn: text → tool call → text
        let state = MockTuiState {
            messages: vec![
                ChatMessage::agent_text("I'll edit that file.".into()),
                ChatMessage::tool_call(TrackedToolCall::new(
                    ToolCall::new(
                        ToolCallId::new("tc_1"),
                        "Editing main.rs".into(),
                        ToolKind::Write,
                        ToolCallStatus::Completed,
                        None,
                    )
                    .with_content(vec![ToolCallContent::Diff {
                        path: "src/main.rs".into(),
                        old_text: Some("old".into()),
                        new_text: "new".into(),
                    }]),
                )),
                ChatMessage::agent_text("Done editing.".into()),
            ],
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
            })
            .expect("draw");

        // Extract rendered text from the buffer
        let buffer = terminal.backend().buffer().clone();
        let rendered: String = (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer.cell((x, y)).map(|c| c.symbol()).unwrap_or(" "))
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        // Verify order: first text appears before tool call, tool call before second text
        let first_text_pos = rendered
            .find("edit that file")
            .expect("first text should render");
        let tool_call_pos = rendered
            .find("Edit(")
            .or_else(|| rendered.find("main.rs"))
            .expect("tool call should render");
        let second_text_pos = rendered
            .find("Done editing")
            .expect("second text should render");

        assert!(
            first_text_pos < tool_call_pos,
            "first text ({first_text_pos}) should appear before tool call ({tool_call_pos})"
        );
        assert!(
            tool_call_pos < second_text_pos,
            "tool call ({tool_call_pos}) should appear before second text ({second_text_pos})"
        );
    }

    #[test]
    fn chat_renders_streaming() {
        let state = MockTuiState {
            streaming_text: "Streaming **markdown** content".into(),
            ..Default::default()
        };

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
            })
            .expect("draw");
    }

    #[test]
    fn chat_renders_drill_in_when_subagent_focused() {
        use cyril_core::types::{AgentMessage, Notification, SessionId};

        let mut state = MockTuiState::default();
        // Add a main session message that should NOT appear during drill-in
        state
            .messages
            .push(ChatMessage::agent_text("main session text".into()));

        // Register a subagent in the tracker
        let sub_info = cyril_core::types::SubagentInfo::new(
            SessionId::new("sub-1"),
            "reviewer",
            "code-reviewer",
            "query",
            cyril_core::types::SubagentStatus::Working {
                message: Some("Running".into()),
            },
        );
        state
            .subagent_tracker
            .apply_notification(&Notification::SubagentListUpdated {
                subagents: vec![sub_info],
                pending_stages: vec![],
            });

        // Push a message into the subagent stream
        let sid = SessionId::new("sub-1");
        state.subagent_ui.apply_notification(
            &sid,
            &Notification::AgentMessage(AgentMessage {
                text: "subagent only text".into(),
                is_streaming: false,
            }),
        );
        state.subagent_ui.focus(sid);

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &state.theme);
            })
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer[(x as u16, y as u16)]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                })
            })
            .collect();

        assert!(
            text.contains("reviewer"),
            "drill-in header should show subagent name"
        );
        assert!(
            text.contains("[Esc] Back"),
            "drill-in should show back hint"
        );
        assert!(
            text.contains("subagent only text"),
            "drill-in should show subagent's messages"
        );
        assert!(
            !text.contains("main session text"),
            "drill-in should NOT show main session messages"
        );
    }

    #[test]
    fn chat_scroll_back_offsets_viewport() {
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(ChatMessage::agent_text(format!("Message {i}")));
        }

        // Follow mode — auto-scroll to bottom
        let state_follow = MockTuiState {
            messages: messages.clone(),
            chat_scroll_back: None,
            ..Default::default()
        };

        // Browse mode — scrolled up
        let state_browse = MockTuiState {
            messages,
            chat_scroll_back: Some(30),
            ..Default::default()
        };

        let backend_follow = TestBackend::new(80, 10);
        let mut term_follow = Terminal::new(backend_follow).expect("test terminal");
        term_follow
            .draw(|frame| render(frame, frame.area(), &state_follow, &state_follow.theme))
            .expect("draw");

        let backend_browse = TestBackend::new(80, 10);
        let mut term_browse = Terminal::new(backend_browse).expect("test terminal");
        term_browse
            .draw(|frame| render(frame, frame.area(), &state_browse, &state_browse.theme))
            .expect("draw");

        // Extract first line of each render to verify different content
        let follow_text: String = (0..80)
            .map(|x| {
                term_follow
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        let browse_text: String = (0..80)
            .map(|x| {
                term_browse
                    .backend()
                    .buffer()
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();

        assert_ne!(
            follow_text, browse_text,
            "follow mode and browse mode should show different content"
        );
    }

    #[test]
    fn chat_scroll_back_short_content_clamps_to_zero() {
        // When content fits in the viewport, browse mode should still render
        // correctly (offset clamps to 0, no underflow).
        let state = MockTuiState {
            messages: vec![ChatMessage::agent_text("Short".into())],
            chat_scroll_back: Some(100),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw should not panic with scroll_back exceeding content");
    }

    #[test]
    fn activity_indicator_shown_for_sending() {
        use std::time::Duration;

        let state = MockTuiState {
            activity: Activity::Sending,
            activity_elapsed: Some(Duration::from_secs(5)),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
            })
            .collect();
        assert!(
            text.contains("Thinking"),
            "Sending state should show Thinking indicator"
        );
        assert!(text.contains("5s"), "should show elapsed seconds");
    }

    #[test]
    fn activity_indicator_not_shown_for_idle() {
        let state = MockTuiState {
            activity: Activity::Idle,
            ..Default::default()
        };
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &state.theme))
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..24)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
            })
            .collect();
        assert!(
            !text.contains("Thinking"),
            "Idle state should not show activity indicator"
        );
    }

    #[test]
    fn render_tool_output_shell_with_exit_code() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "cargo test".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "test result: FAILED\n2 tests failed",
                "exit_status": 1
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("Exit: 1"), "should show non-zero exit code");
        assert!(text.contains("test result: FAILED"), "should show stdout");
        let theme = crate::traits::test_support::marker_theme();
        assert_eq!(lines[0].style.fg, Some(theme.emphasis));
        assert!(
            lines[1..]
                .iter()
                .all(|line| line.style.fg == Some(theme.subdued))
        );
    }

    #[test]
    fn render_tool_output_shell_zero_exit_code_hidden() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "cargo test".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "test result: ok",
                "exit_status": 0
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!text.contains("Exit:"), "zero exit code should be hidden");
        assert!(text.contains("test result: ok"), "should show stdout");
    }

    #[test]
    fn render_tool_output_failed_shows_error() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "shell".into(),
                ToolKind::Execute,
                ToolCallStatus::Failed,
                None,
            )
            .with_raw_output(Some(serde_json::json!("Command timed out"))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("Error: Command timed out"),
            "should show error"
        );
        assert_eq!(
            lines[0].style.fg,
            Some(crate::traits::test_support::marker_theme().subdued_negative)
        );
    }

    #[test]
    fn render_tool_output_read_shows_char_count() {
        use cyril_core::types::*;

        let content = "a".repeat(2500);
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Read(main.rs)".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({"items": [{"Text": content}]}))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("2.5k chars"),
            "should show char count: got {text}"
        );
        assert_eq!(
            lines[0].style.fg,
            Some(crate::traits::test_support::marker_theme().subdued)
        );
    }

    #[test]
    fn render_tool_output_read_counts_unicode_characters() {
        use cyril_core::types::*;

        let content = "界".repeat(800);
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_unicode"),
                "Read(unicode.txt)".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!({"items": [{"Text": content}]}))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );

        assert_eq!(lines[0].to_string().trim(), "800 chars");
    }

    #[test]
    fn render_tool_output_read_small_file_shows_raw_count() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "Read(small.rs)".into(),
                ToolKind::Read,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(
                serde_json::json!({"items": [{"Text": "hello world"}]}),
            )),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("11 chars"),
            "should show raw char count: got {text}"
        );
    }

    #[test]
    fn render_tool_output_write_skipped() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_raw_output(Some(serde_json::json!("written ok"))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        assert!(
            lines.is_empty(),
            "Write tools should not render output (diff is shown instead)"
        );
    }

    #[test]
    fn render_tool_output_truncates_long_output() {
        use cyril_core::types::*;

        let long_output: String = (0..20)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let tc = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("tc_1"),
                "shell".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "long-cmd"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": long_output,
                "exit_status": 0
            }))),
        );
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        let text: String = lines
            .iter()
            .map(|l| l.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            text.contains("...15 more lines"),
            "should show overflow indicator: got {text}"
        );
        // 5 visible lines + 1 overflow indicator = 6 total
        assert_eq!(lines.len(), 6, "should show 5 lines + overflow");
        assert!(lines.iter().all(|line| {
            line.style.fg == Some(crate::traits::test_support::marker_theme().subdued)
        }));
    }

    #[test]
    fn render_tool_output_in_progress_shows_nothing() {
        use cyril_core::types::*;

        let tc = TrackedToolCall::new(ToolCall::new(
            ToolCallId::new("tc_1"),
            "shell".into(),
            ToolKind::Execute,
            ToolCallStatus::InProgress,
            None,
        ));
        let mut lines = Vec::new();
        render_tool_output(
            &mut lines,
            &tc,
            &crate::traits::test_support::marker_theme(),
        );
        assert!(
            lines.is_empty(),
            "in-progress tools should not render output"
        );
    }
}
