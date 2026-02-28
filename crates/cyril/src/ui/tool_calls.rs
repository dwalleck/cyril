use std::path::Path;

use agent_client_protocol as acp;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};
use similar::{ChangeTag, TextDiff};

use super::highlight;

/// Cached diff summary for a tool call.
#[derive(Debug, Clone)]
pub struct DiffSummary {
    pub added: usize,
    pub removed: usize,
}

/// A tracked tool call storing the full ACP ToolCall plus cached diff summary.
#[derive(Debug, Clone)]
pub struct TrackedToolCall {
    pub tool_call: acp::ToolCall,
    pub diff_summary: Option<DiffSummary>,
}

impl TrackedToolCall {
    pub fn new(tool_call: acp::ToolCall) -> Self {
        let diff_summary = compute_diff_summary(&tool_call);
        Self {
            tool_call,
            diff_summary,
        }
    }

    pub fn id(&self) -> &acp::ToolCallId {
        &self.tool_call.tool_call_id
    }

    pub fn status(&self) -> acp::ToolCallStatus {
        self.tool_call.status
    }

    pub fn kind(&self) -> acp::ToolKind {
        self.tool_call.kind
    }

    /// Apply an update and recompute diff summary.
    pub fn apply_update(&mut self, fields: acp::ToolCallUpdateFields) {
        self.tool_call.update(fields);
        self.diff_summary = compute_diff_summary(&self.tool_call);
    }

    /// Generate a rich display label based on tool kind.
    pub fn display_label(&self) -> String {
        match self.tool_call.kind {
            acp::ToolKind::Read => {
                if let Some(path) = self.primary_path() {
                    format!("Read({})", short_path(&path))
                } else {
                    self.tool_call.title.clone()
                }
            }
            acp::ToolKind::Edit => {
                if let Some(path) = self.primary_path() {
                    format!("Edit({})", short_path(&path))
                } else {
                    self.tool_call.title.clone()
                }
            }
            acp::ToolKind::Execute => {
                if let Some(cmd) = self.extract_command() {
                    format!("Execute({cmd})")
                } else {
                    self.tool_call.title.clone()
                }
            }
            acp::ToolKind::Search => self.tool_call.title.clone(),
            acp::ToolKind::Think => "Thinking...".to_string(),
            _ => self.tool_call.title.clone(),
        }
    }

    /// Get primary file path from locations, then diff content, then raw_input.
    fn primary_path(&self) -> Option<String> {
        if let Some(loc) = self.tool_call.locations.first() {
            return Some(loc.path.to_string_lossy().to_string());
        }
        for content in &self.tool_call.content {
            if let acp::ToolCallContent::Diff(diff) = content {
                return Some(diff.path.to_string_lossy().to_string());
            }
        }
        if let Some(raw) = &self.tool_call.raw_input {
            if let Some(path) = raw.get("file_path").and_then(|v| v.as_str()) {
                return Some(path.to_string());
            }
            if let Some(path) = raw.get("path").and_then(|v| v.as_str()) {
                return Some(path.to_string());
            }
        }
        None
    }

    /// Extract command string from raw_input for Execute kind.
    fn extract_command(&self) -> Option<String> {
        let raw = self.tool_call.raw_input.as_ref()?;
        let cmd = raw.get("command").and_then(|v| v.as_str())?;
        if cmd.chars().count() > 40 {
            let truncated: String = cmd.chars().take(37).collect();
            Some(format!("{truncated}..."))
        } else {
            Some(cmd.to_string())
        }
    }
}

/// Show last 2 components of a path for brevity.
fn short_path(path: &str) -> String {
    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if parts.len() <= 2 {
        parts.join("/")
    } else {
        parts[parts.len() - 2..].join("/")
    }
}

/// Compute a diff summary from tool call content using `similar`.
fn compute_diff_summary(tool_call: &acp::ToolCall) -> Option<DiffSummary> {
    for content in &tool_call.content {
        if let acp::ToolCallContent::Diff(diff) = content {
            let old = diff.old_text.as_deref().unwrap_or("");
            let text_diff = TextDiff::from_lines(old, &diff.new_text);
            let mut added = 0usize;
            let mut removed = 0usize;
            for change in text_diff.iter_all_changes() {
                match change.tag() {
                    ChangeTag::Insert => added += 1,
                    ChangeTag::Delete => removed += 1,
                    ChangeTag::Equal => {}
                }
            }
            if added > 0 || removed > 0 {
                return Some(DiffSummary { added, removed });
            }
        }
    }
    None
}

fn kind_color(kind: acp::ToolKind) -> Color {
    match kind {
        acp::ToolKind::Edit => Color::Magenta,
        acp::ToolKind::Read => Color::Blue,
        acp::ToolKind::Execute => Color::Yellow,
        acp::ToolKind::Search => Color::Cyan,
        acp::ToolKind::Think => Color::DarkGray,
        acp::ToolKind::Delete => Color::Red,
        acp::ToolKind::Fetch => Color::Cyan,
        _ => Color::White,
    }
}

/// Maximum number of diff context + change lines to show per tool call.
const MAX_DIFF_LINES: usize = 20;

/// Render a tool call as one or more styled Lines for inline display in the chat.
///
/// For Edit tool calls with Diff content, renders the header line followed by
/// the actual changed lines (additions in green, removals in red, context in gray).
pub fn render_lines(tc: &TrackedToolCall) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    // Header line: icon + label + summary
    let (icon, icon_color) = match tc.status() {
        acp::ToolCallStatus::Pending => ("○", Color::DarkGray),
        acp::ToolCallStatus::InProgress => ("◐", Color::Yellow),
        acp::ToolCallStatus::Completed => ("●", Color::Green),
        acp::ToolCallStatus::Failed => ("✕", Color::Red),
        _ => ("?", Color::DarkGray),
    };

    let label = tc.display_label();
    let label_color = kind_color(tc.kind());

    let mut header_spans = vec![
        Span::styled(format!("  {icon} "), Style::default().fg(icon_color)),
        Span::styled(label, Style::default().fg(label_color)),
    ];

    if let Some(ref ds) = tc.diff_summary {
        header_spans.push(Span::styled(
            format!("  +{} -{}", ds.added, ds.removed),
            Style::default().fg(Color::DarkGray),
        ));
    }

    lines.push(Line::from(header_spans));

    // For Edit kinds, render the actual diff lines
    if tc.kind() == acp::ToolKind::Edit {
        render_diff_content(tc, &mut lines);
    }

    lines
}

/// Render the actual diff content (changed lines with context) below the header.
fn render_diff_content(tc: &TrackedToolCall, lines: &mut Vec<Line<'static>>) {
    for content in &tc.tool_call.content {
        if let acp::ToolCallContent::Diff(diff) = content {
            let ext = Path::new(&diff.path)
                .extension()
                .and_then(|e| e.to_str())
                .map(|s| s.to_string());

            let old = diff.old_text.as_deref().unwrap_or("");
            let text_diff = TextDiff::from_lines(old, &diff.new_text);

            let overflow_style = Style::default().fg(Color::DarkGray);
            let mut count = 0;

            for group in text_diff.grouped_ops(1) {
                for op in &group {
                    for change in text_diff.iter_changes(op) {
                        if count >= MAX_DIFF_LINES {
                            lines.push(Line::from(Span::styled(
                                "      ...",
                                overflow_style,
                            )));
                            return;
                        }

                        let line_text = change.value().trim_end_matches('\n');
                        let (prefix, diff_color) = match change.tag() {
                            ChangeTag::Delete => (" -", Color::Red),
                            ChangeTag::Insert => (" +", Color::Green),
                            ChangeTag::Equal => ("  ", Color::DarkGray),
                        };

                        let line_no = match change.tag() {
                            ChangeTag::Delete => change
                                .old_index()
                                .map(|i| i + 1)
                                .unwrap_or(0),
                            _ => change
                                .new_index()
                                .map(|i| i + 1)
                                .unwrap_or(0),
                        };

                        let gutter = Span::styled(
                            format!("    {line_no:>4}{prefix} "),
                            Style::default().fg(diff_color),
                        );

                        if change.tag() == ChangeTag::Equal {
                            lines.push(Line::from(vec![
                                gutter,
                                Span::styled(
                                    line_text.to_string(),
                                    Style::default().fg(Color::DarkGray),
                                ),
                            ]));
                        } else {
                            let highlighted = highlight::highlight_line(
                                line_text,
                                ext.as_deref(),
                            );
                            let mut spans = vec![gutter];
                            for (style, text) in highlighted {
                                let tinted_fg = match style.fg {
                                    Some(fg) => highlight::tint_with_diff_color(fg, diff_color),
                                    None => diff_color,
                                };
                                spans.push(Span::styled(
                                    text,
                                    Style::default().fg(tinted_fg),
                                ));
                            }
                            lines.push(Line::from(spans));
                        }

                        count += 1;
                    }
                }
            }
            // Only render the first Diff content block
            return;
        }
    }
}
