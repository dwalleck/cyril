//! Concrete presentation selectors. Retained blocks own generated labels and shared
//! Markdown only; all other text resolves against the selected authoritative source.

use std::ops::Range;
use std::sync::Arc;

use ratatui::prelude::*;
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

use crate::theme::Theme;
use crate::tool_diff::{DiffPreview, DiffSource};
use crate::traits::{
    ChatMessage, ChatMessageKind, SteerEchoStatus, ToolText, ToolTextSource, TrackedToolCall,
};
use crate::widgets::chat_layout::wrap::{SourceLine, SourceSpan, WrappedRow, wrap_lines};
use crate::widgets::markdown;

#[derive(Debug)]
enum Source {
    Static(&'static str),
    Generated(String),
    MessageText,
    PlanTitle(usize),
    SteerNote,
    Tool(ToolTextSource),
    DiffRow {
        content: usize,
        row: usize,
    },
    DerivedSlice(usize),
    LiveThought,
    Markdown {
        backing: usize,
        line: usize,
        span: usize,
    },
}

#[derive(Debug)]
struct SourceSlot {
    source: Source,
    range: Range<usize>,
}

#[derive(Default)]
pub(super) struct PreparedBlock {
    sources: Vec<SourceSlot>,
    markdown: Vec<Arc<[Line<'static>]>>,
    pub rows: Vec<WrappedRow>,
}

#[derive(Clone, Copy, Default)]
pub(super) struct Sources<'a> {
    pub message: Option<&'a ChatMessage>,
    pub tool: Option<&'a TrackedToolCall>,
    pub thought: Option<&'a str>,
}

impl<'a> Sources<'a> {
    pub fn message(message: &'a ChatMessage) -> Self {
        Self {
            message: Some(message),
            tool: match message.kind() {
                ChatMessageKind::ToolCall(tool) => Some(tool),
                _ => None,
            },
            ..Self::default()
        }
    }

    fn external(self, source: &Source) -> Option<&'a str> {
        match source {
            Source::Static(text) => Some(*text),
            Source::MessageText => match self.message?.kind() {
                ChatMessageKind::UserText(text)
                | ChatMessageKind::Thought(text)
                | ChatMessageKind::System(text)
                | ChatMessageKind::CommandOutput { text, .. }
                | ChatMessageKind::SteerEcho { text, .. } => Some(text),
                _ => None,
            },
            Source::PlanTitle(index) => match self.message?.kind() {
                ChatMessageKind::Plan(plan) => {
                    plan.entries().get(*index).map(|entry| entry.title())
                }
                _ => None,
            },
            Source::SteerNote => match self.message?.kind() {
                ChatMessageKind::SteerEcho { note, .. } => note.as_deref(),
                _ => None,
            },
            Source::Tool(selector) => self.tool?.resolve_text_source(*selector),
            Source::DiffRow { content, row } => {
                let tool = self.tool?;
                let cyril_core::types::ToolCallContent::Diff {
                    old_text, new_text, ..
                } = tool.content().get(*content)?
                else {
                    return None;
                };
                let source = DiffSource {
                    old: old_text.as_deref().unwrap_or(""),
                    new: new_text,
                };
                Some(tool.diff_preview().rows().get(*row)?.text(source))
            }
            Source::LiveThought => self.thought,
            Source::Generated(_) | Source::DerivedSlice(_) | Source::Markdown { .. } => None,
        }
    }
}

impl PreparedBlock {
    pub fn source_text<'a>(&'a self, slot: usize, sources: Sources<'a>) -> &'a str {
        let slot = &self.sources[slot];
        let text = match &slot.source {
            Source::Generated(text) => text,
            Source::DerivedSlice(index) => self.source_text(*index, sources),
            Source::Markdown {
                backing,
                line,
                span,
            } => self.markdown[*backing][*line].spans[*span].content.as_ref(),
            source => match sources.external(source) {
                Some(text) => text,
                None => unreachable!("prepared selector matches current source"),
            },
        };
        &text[slot.range.clone()]
    }

    pub fn metrics(&self) -> (usize, usize, usize) {
        let runs = self.rows.iter().map(|row| row.runs.len()).sum();
        let owned = self
            .sources
            .iter()
            .map(|slot| match &slot.source {
                Source::Generated(text) => text.len(),
                _ => 0,
            })
            .sum::<usize>();
        (runs, self.sources.len(), owned)
    }

    pub fn shared_markdown(&self) -> &[Arc<[Line<'static>]>] {
        &self.markdown
    }
}

struct LogicalLine {
    spans: Vec<(usize, Style)>,
    style: Style,
    alignment: Alignment,
}

#[derive(Default)]
pub(super) struct Preparation {
    block: PreparedBlock,
    lines: Vec<LogicalLine>,
}

impl Preparation {
    fn source(&mut self, source: Source, range: Range<usize>) -> usize {
        let index = self.block.sources.len();
        self.block.sources.push(SourceSlot { source, range });
        index
    }

    fn literal(&mut self, text: &'static str) -> usize {
        self.source(Source::Static(text), 0..text.len())
    }

    fn generated(&mut self, text: String) -> usize {
        let len = text.len();
        self.source(Source::Generated(text), 0..len)
    }

    fn line(&mut self, spans: Vec<(usize, Style)>, style: Style) {
        self.lines.push(LogicalLine {
            spans,
            style,
            alignment: Alignment::Left,
        });
    }

    pub fn blank(&mut self) {
        self.line(Vec::new(), Style::default());
    }

    pub fn label(&mut self, text: &'static str, style: Style) {
        let source = self.literal(text);
        self.line(vec![(source, Style::default())], style);
    }

    pub fn generated_line(&mut self, text: String, style: Style) {
        let source = self.generated(text);
        self.line(vec![(source, Style::default())], style);
    }

    /// A generated prefix/body/suffix was one original Paragraph span. Preserve
    /// grapheme segmentation where a combining prefix of the body joins its label,
    /// without retaining a copy of the body. Only that genuinely derived boundary
    /// cluster is attached to the generated label.
    fn affixed(
        &mut self,
        prefix: &'static str,
        source: Source,
        range: Range<usize>,
        text: &str,
        mut suffix: Option<String>,
        style: Style,
    ) {
        let mut consumed = 0;
        let mut prefix_text = None;
        if !prefix.is_empty() && !text.is_empty() {
            let span = Span::raw(text);
            if let Some(first) = span.styled_graphemes(Style::default()).next()
                && first.symbol.as_ptr() == text.as_ptr()
                && !first.symbol.as_bytes()[0].is_ascii()
            {
                // Query the borrowed chunks before allocating crossing bytes.
                let mut cursor =
                    GraphemeCursor::new(prefix.len(), prefix.len() + first.symbol.len(), true);
                let crosses = loop {
                    match cursor.is_boundary(first.symbol, prefix.len()) {
                        Ok(boundary) => break !boundary,
                        Err(GraphemeIncomplete::PreContext(offset)) => {
                            cursor.provide_context(&prefix[..offset], 0)
                        }
                        Err(_) => unreachable!("the body chunk contains the affix boundary"),
                    }
                };
                if crosses {
                    consumed = first.symbol.len();
                    prefix_text = Some(format!("{prefix}{}", first.symbol));
                }
            }
        }
        let mut body_end = text.len();
        if let Some(suffix_text) = &mut suffix
            && let Some(first) = suffix_text.chars().next()
        {
            // Unicode Prepend scalars can join the generated status suffix's
            // first space. Keep that original single-span cluster intact too.
            for (offset, ch) in text.char_indices().rev() {
                if offset < consumed || ch.is_ascii() {
                    break;
                }
                // Exactly two Unicode scalars need at most eight UTF-8 bytes.
                let mut probe = [0; 8];
                let ch_len = ch.encode_utf8(&mut probe).len();
                let first_len = first.encode_utf8(&mut probe[ch_len..]).len();
                let Ok(probe_text) = std::str::from_utf8(&probe[..ch_len + first_len]) else {
                    unreachable!("two UTF-8 encoded scalars");
                };
                let span = Span::raw(probe_text);
                let joins = span
                    .styled_graphemes(Style::default())
                    .next()
                    .is_some_and(|grapheme| grapheme.symbol.len() > ch_len);
                if !joins {
                    break;
                }
                body_end = offset;
            }
            if body_end < text.len() {
                suffix_text.insert_str(0, &text[body_end..]);
            }
        }
        let mut spans = Vec::new();
        if let Some(text) = prefix_text {
            spans.push((self.generated(text), Style::default()));
        } else if !prefix.is_empty() {
            spans.push((self.literal(prefix), Style::default()));
        }
        spans.push((
            self.source(source, range.start + consumed..range.start + body_end),
            Style::default(),
        ));
        if let Some(suffix) = suffix {
            spans.push((self.generated(suffix), Style::default()));
        }
        self.line(spans, style);
    }

    pub fn markdown(&mut self, text: &str, width: u16, theme: &Theme) {
        let backing = self.block.markdown.len();
        let lines = markdown::render_with_theme(text, usize::from(width), theme);
        for (line_index, line) in lines.iter().enumerate() {
            let mut spans = Vec::with_capacity(line.spans.len());
            for (span_index, span) in line.spans.iter().enumerate() {
                let source = self.source(
                    Source::Markdown {
                        backing,
                        line: line_index,
                        span: span_index,
                    },
                    0..span.content.len(),
                );
                spans.push((source, span.style));
            }
            self.lines.push(LogicalLine {
                spans,
                style: line.style,
                alignment: line.alignment.unwrap_or(Alignment::Left),
            });
        }
        self.block.markdown.push(lines);
    }

    pub fn message(&mut self, message: &ChatMessage, width: u16, theme: &Theme) {
        match message.kind() {
            ChatMessageKind::UserText(text) => {
                self.label(
                    "You:",
                    Style::default().fg(theme.user).add_modifier(Modifier::BOLD),
                );
                for line in text.lines() {
                    self.affixed(
                        "  ",
                        Source::MessageText,
                        subslice(text, line),
                        line,
                        None,
                        Style::default(),
                    );
                }
            }
            ChatMessageKind::AgentText(text) => {
                self.label(
                    "Kiro:",
                    Style::default()
                        .fg(theme.agent)
                        .add_modifier(Modifier::BOLD),
                );
                self.markdown(text, width, theme);
            }
            ChatMessageKind::Thought(text) => self.thought(text, false, theme),
            ChatMessageKind::ToolCall(tool) => self.tool(tool, theme),
            ChatMessageKind::Plan(plan) => {
                self.label(
                    "Plan:",
                    Style::default()
                        .fg(theme.emphasis)
                        .add_modifier(Modifier::BOLD),
                );
                for (index, entry) in plan.entries().iter().enumerate() {
                    let prefix = match entry.status() {
                        cyril_core::types::PlanEntryStatus::Pending => "  ○ ",
                        cyril_core::types::PlanEntryStatus::InProgress => "  ◐ ",
                        cyril_core::types::PlanEntryStatus::Completed => "  ● ",
                        cyril_core::types::PlanEntryStatus::Failed => "  ✗ ",
                    };
                    self.affixed(
                        prefix,
                        Source::PlanTitle(index),
                        0..entry.title().len(),
                        entry.title(),
                        None,
                        Style::default(),
                    );
                }
            }
            ChatMessageKind::System(text) => {
                let style = Style::default()
                    .fg(theme.system)
                    .add_modifier(Modifier::ITALIC);
                for line in text.lines() {
                    self.affixed(
                        "",
                        Source::MessageText,
                        subslice(text, line),
                        line,
                        None,
                        style,
                    );
                }
            }
            ChatMessageKind::CommandOutput { command, text } => {
                self.generated_line(
                    format!("/{command}:"),
                    Style::default()
                        .fg(theme.soft_accent)
                        .add_modifier(Modifier::BOLD),
                );
                for line in text.lines() {
                    self.affixed(
                        "  ",
                        Source::MessageText,
                        subslice(text, line),
                        line,
                        None,
                        Style::default(),
                    );
                }
            }
            ChatMessageKind::SteerEcho {
                text, status, note, ..
            } => {
                let (suffix, color) = match status {
                    SteerEchoStatus::Queued => ("queued", theme.emphasis),
                    SteerEchoStatus::Applied => ("applied", theme.positive_accent),
                    SteerEchoStatus::Cleared => ("cleared", theme.subdued),
                    SteerEchoStatus::Unsupported => ("not supported", theme.subdued_negative),
                };
                let style = Style::default().fg(color).add_modifier(Modifier::ITALIC);
                let mut lines = text.lines();
                let first = lines.next().unwrap_or("");
                let range = if first.is_empty() {
                    0..0
                } else {
                    subslice(text, first)
                };
                self.affixed(
                    "  ↳ steer: ",
                    Source::MessageText,
                    range,
                    first,
                    Some(format!(" — {suffix}")),
                    style,
                );
                for line in lines {
                    self.affixed(
                        "    ",
                        Source::MessageText,
                        subslice(text, line),
                        line,
                        None,
                        style,
                    );
                }
                if let Some(note) = note.as_deref() {
                    let trimmed = note.trim();
                    if !trimmed.is_empty() {
                        let style = Style::default()
                            .fg(theme.subdued)
                            .add_modifier(Modifier::ITALIC);
                        for (index, line) in trimmed.lines().enumerate() {
                            let prefix = if index == 0 { "    ⤷ " } else { "      " };
                            self.affixed(
                                prefix,
                                Source::SteerNote,
                                subslice(note, line),
                                line,
                                None,
                                style,
                            );
                        }
                    }
                }
            }
        }
    }

    pub fn thought(&mut self, text: &str, live: bool, theme: &Theme) {
        let style = Style::default()
            .fg(theme.muted)
            .add_modifier(Modifier::ITALIC);
        if text.is_empty() {
            self.label("💭 ", style);
        } else {
            for (index, line) in text.lines().enumerate() {
                self.affixed(
                    if index == 0 { "💭 " } else { "   " },
                    if live {
                        Source::LiveThought
                    } else {
                        Source::MessageText
                    },
                    subslice(text, line),
                    line,
                    None,
                    style,
                );
            }
        }
    }

    pub fn child_header(&mut self, name: &str, theme: &Theme) {
        let name = self.generated(format!("─── {name} "));
        let back = self.literal("[Esc] Back");
        self.line(
            vec![
                (
                    name,
                    Style::default()
                        .fg(theme.soft_accent)
                        .add_modifier(Modifier::BOLD),
                ),
                (back, Style::default().fg(theme.muted)),
            ],
            Style::default(),
        );
        self.blank();
    }

    pub fn live_label(&mut self, name: Option<&str>, theme: &Theme) {
        let style = Style::default()
            .fg(theme.agent)
            .add_modifier(Modifier::BOLD);
        if let Some(name) = name {
            self.generated_line(format!("{name}:"), style);
        } else {
            self.label("Kiro:", style);
        }
    }

    pub fn activity(&mut self, key: Option<(bool, usize, u64)>, theme: &Theme) {
        let Some((running, spinner, seconds)) = key else {
            return;
        };
        let (label, color) = if running {
            ("Running...", theme.accent_quinary)
        } else {
            ("Thinking...", theme.muted)
        };
        let spinner = self.generated(format!("{} ", crate::spinner::SPINNER_CHARS[spinner]));
        let text = self.generated(format!("{label} {seconds}s"));
        self.line(
            vec![
                (spinner, Style::default().fg(color)),
                (text, Style::default().fg(color)),
            ],
            Style::default(),
        );
    }

    pub fn tool(&mut self, tool: &TrackedToolCall, theme: &Theme) {
        use cyril_core::types::{ToolCallStatus, ToolKind};
        let icon = match tool.status() {
            ToolCallStatus::InProgress => "⟳ ",
            ToolCallStatus::Pending => "⏳ ",
            ToolCallStatus::Completed => "✓ ",
            ToolCallStatus::Failed => "✗ ",
        };
        let label = match tool.kind() {
            ToolKind::Read => tool
                .primary_path()
                .map(|path| format!("Read({path})"))
                .unwrap_or_else(|| tool.title().to_string()),
            ToolKind::Write => tool
                .primary_path()
                .map(|path| format!("Edit({path})"))
                .unwrap_or_else(|| tool.title().to_string()),
            ToolKind::Execute => {
                if let Some(command) = tool.command_text() {
                    let mut chars = command.chars();
                    let display: String = chars.by_ref().take(50).collect();
                    if chars.next().is_some() {
                        format!("Run({display}...)")
                    } else {
                        format!("Run({display})")
                    }
                } else {
                    tool.title().to_string()
                }
            }
            ToolKind::Think => "Thinking...".to_string(),
            ToolKind::Search | ToolKind::Fetch | ToolKind::SwitchMode | ToolKind::Other => {
                tool.title().to_string()
            }
        };
        let status_color = match tool.status() {
            ToolCallStatus::Completed => theme.subdued_positive,
            ToolCallStatus::Failed => theme.subdued_negative,
            ToolCallStatus::Pending | ToolCallStatus::InProgress => theme.emphasis,
        };
        let kind_color = match tool.kind() {
            ToolKind::Read => theme.accent_tertiary,
            ToolKind::Write | ToolKind::SwitchMode => theme.accent_quaternary,
            ToolKind::Execute => theme.emphasis,
            ToolKind::Search | ToolKind::Fetch => theme.accent_quinary,
            ToolKind::Think => theme.subdued,
            ToolKind::Other => theme.text,
        };
        let icon = self.literal(icon);
        let label = self.generated(label);
        let mut spans = vec![
            (icon, Style::default().fg(status_color)),
            (label, Style::default().fg(kind_color)),
        ];
        let diff = tool.diff_view();
        if let Some((added, removed)) = diff.and_then(|(_, _, preview)| preview.summary()) {
            let summary = self.generated(format!("  +{added} -{removed}"));
            spans.push((summary, Style::default().fg(theme.subdued)));
        }
        self.line(spans, Style::default());
        if tool.status() == ToolCallStatus::Completed
            && tool.kind() == ToolKind::Write
            && let Some((content, source, preview)) = diff
        {
            self.diff(content, source, preview, theme);
        }
        self.output(tool, theme);
    }

    fn diff(
        &mut self,
        content: usize,
        source: DiffSource<'_>,
        preview: &DiffPreview,
        theme: &Theme,
    ) {
        for (index, row) in preview.rows().iter().enumerate() {
            let (marker, color) = match row.tag {
                similar::ChangeTag::Delete => ("- ", theme.subdued_negative),
                similar::ChangeTag::Insert => ("+ ", theme.subdued_positive),
                similar::ChangeTag::Equal => ("  ", theme.subdued),
            };
            let gutter = self.generated(format!("    {:>4} │{marker}", row.line_number));
            let text = self.source(
                Source::DiffRow {
                    content,
                    row: index,
                },
                0..row.text(source).len(),
            );
            self.line(
                vec![
                    (gutter, Style::default().fg(color)),
                    (text, Style::default().fg(color)),
                ],
                Style::default(),
            );
        }
        if preview.truncated() {
            self.label("      ...", Style::default().fg(theme.subdued));
        }
    }

    pub fn output(&mut self, tool: &TrackedToolCall, theme: &Theme) {
        use cyril_core::types::{ToolCallStatus, ToolKind};
        if tool.status() == ToolCallStatus::Failed {
            if let Some(error) = tool.error_text_view() {
                self.tool_text(error, true, theme);
            }
            return;
        }
        if tool.status() != ToolCallStatus::Completed || tool.kind() == ToolKind::Write {
            return;
        }
        if let Some(code) = tool.exit_code()
            && code != 0
        {
            self.generated_line(
                format!("    Exit: {code}"),
                Style::default().fg(theme.emphasis),
            );
        }
        if let Some(text) = tool.output_text_view() {
            if tool.kind() == ToolKind::Read {
                let text = match &text {
                    ToolText::Borrowed { text, .. } => *text,
                    ToolText::Derived(text) => text.as_str(),
                };
                let count = text.chars().count();
                let summary = if count < 1000 {
                    format!("{count} chars")
                } else {
                    format!("{:.1}k chars", count as f64 / 1000.0)
                };
                self.generated_line(format!("    {summary}"), Style::default().fg(theme.subdued));
            } else {
                self.tool_text(text, false, theme);
            }
        }
    }

    fn tool_text(&mut self, view: ToolText<'_>, error: bool, theme: &Theme) {
        let style = Style::default().fg(if error {
            theme.subdued_negative
        } else {
            theme.subdued
        });
        match view {
            ToolText::Borrowed { source, text } => {
                if error {
                    self.affixed(
                        "    Error: ",
                        Source::Tool(source),
                        0..text.len(),
                        text,
                        None,
                        style,
                    );
                } else {
                    for line in text.lines().take(5) {
                        self.affixed(
                            "    | ",
                            Source::Tool(source),
                            subslice(text, line),
                            line,
                            None,
                            style,
                        );
                    }
                    let count = text.lines().count();
                    if count > 5 {
                        self.generated_line(format!("    ...{} more lines", count - 5), style);
                    }
                }
            }
            ToolText::Derived(mut text) => {
                if error {
                    text.insert_str(0, "    Error: ");
                    self.generated_line(text, style);
                } else {
                    // The JSON serialization is genuinely derived backing, retained once.
                    let source = self.generated(text);
                    let text = match &self.block.sources[source].source {
                        Source::Generated(text) => text,
                        _ => unreachable!(),
                    };
                    let ranges: Vec<_> = text
                        .lines()
                        .take(5)
                        .map(|line| subslice(text, line))
                        .collect();
                    let count = text.lines().count();
                    for range in ranges {
                        let prefix = self.literal("    | ");
                        let slice = self.source(Source::DerivedSlice(source), range);
                        self.line(
                            vec![(prefix, Style::default()), (slice, Style::default())],
                            style,
                        );
                    }
                    if count > 5 {
                        self.generated_line(format!("    ...{} more lines", count - 5), style);
                    }
                }
            }
        }
    }

    pub fn finish(mut self, sources: Sources<'_>, width: u16) -> PreparedBlock {
        let lines: Vec<_> = self
            .lines
            .iter()
            .map(|line| SourceLine {
                spans: line
                    .spans
                    .iter()
                    .map(|(source, style)| SourceSpan {
                        source: *source,
                        offset: 0,
                        text: self.block.source_text(*source, sources),
                        style: *style,
                    })
                    .collect(),
                style: line.style,
                alignment: line.alignment,
            })
            .collect();
        let rows = wrap_lines(&lines, width);
        self.block.rows = rows;
        self.block
    }

    #[cfg(test)]
    pub fn into_lines<'a>(self, sources: Sources<'a>) -> Vec<Line<'a>> {
        // Snapshot the production presentation for existing shape/style fixtures.
        // This adapter does not implement presentation or wrapping.
        let block = self.block;
        self.lines
            .into_iter()
            .map(|line| {
                let spans = line
                    .spans
                    .into_iter()
                    .map(|(index, style)| {
                        let slot = &block.sources[index];
                        let content = if let Some(text) = sources.external(&slot.source) {
                            std::borrow::Cow::Borrowed(
                                text.get(slot.range.clone()).expect("test source range"),
                            )
                        } else {
                            std::borrow::Cow::Owned(block.source_text(index, sources).to_string())
                        };
                        Span::styled(content, style)
                    })
                    .collect::<Vec<_>>();
                Line::from(spans)
                    .style(line.style)
                    .alignment(line.alignment)
            })
            .collect()
    }
}

fn subslice(source: &str, slice: &str) -> Range<usize> {
    let start = slice.as_ptr().addr() - source.as_ptr().addr();
    start..start + slice.len()
}

#[cfg(test)]
mod tests {
    use crate::widgets::chat_layout::wrap::{
        assert_repaired_rows, legacy_reference_is_unsafe, repaired_window,
    };
    use ratatui::buffer::Buffer;
    use ratatui::widgets::{Paragraph, Widget, Wrap};

    use super::*;

    fn assert_reference(
        preparation: Preparation,
        sources: Sources<'_>,
        reference: Vec<Line<'_>>,
        width: u16,
    ) -> PreparedBlock {
        let block = preparation.finish(sources, width);
        if width == 0 {
            assert!(block.rows.is_empty());
            return block;
        }
        let background = Style::default().fg(Color::White).bg(Color::DarkGray);
        // Classify the ORIGINAL single-span affixed presentation, not the corrected rows or a
        // Unicode/width heuristic. Real Paragraph remains the oracle for every safe input.
        let mut slot = 0;
        let original: Vec<_> = reference
            .iter()
            .map(|line| SourceLine {
                spans: line
                    .spans
                    .iter()
                    .map(|span| {
                        let source = slot;
                        slot += 1;
                        SourceSpan {
                            source,
                            offset: 0,
                            text: span.content.as_ref(),
                            style: span.style,
                        }
                    })
                    .collect(),
                style: line.style,
                alignment: line.alignment.unwrap_or(Alignment::Left),
            })
            .collect();
        let unsafe_reference = legacy_reference_is_unsafe(&original, width);
        if unsafe_reference {
            assert_repaired_rows(
                &original,
                &block.rows,
                |slot| block.source_text(slot, sources),
                width,
            );
        }
        let paragraph = Paragraph::new(reference)
            .style(background)
            .wrap(Wrap { trim: false });
        if !unsafe_reference {
            assert_eq!(block.rows.len(), paragraph.line_count(width));
        }
        for start in 0..block.rows.len() {
            let area = Rect::new(3, 2, width, 3);
            let expected = if unsafe_reference {
                repaired_window(
                    &block.rows,
                    |slot| block.source_text(slot, sources),
                    start,
                    area,
                    background,
                )
            } else {
                let mut expected = Buffer::empty(area);
                paragraph
                    .clone()
                    .scroll((start as u16, 0))
                    .render(area, &mut expected);
                expected
            };
            let mut actual = Buffer::empty(area);
            actual.set_style(area, background);
            for (y, row) in block.rows[start..].iter().take(3).enumerate() {
                crate::widgets::chat_layout::paint_row(&mut actual, area, y as u16, row, |slot| {
                    block.source_text(slot, sources)
                });
            }
            assert_eq!(actual, expected, "width {width}, row {start}");
        }
        block
    }

    #[test]
    fn generated_affix_carry_repair_has_literal_rows_cells_and_styles() {
        type CarryFixture<'a> = (&'a str, &'a [&'a str], &'a [[&'a str; 2]]);
        let theme = crate::traits::test_support::marker_theme();
        let header = Style::default().fg(theme.user).add_modifier(Modifier::BOLD);
        let fixtures: &[CarryFixture<'_>] = &[
            (
                "a界a",
                &["Yo", "u:", "  ", "a", "界", "a"],
                &[
                    ["Y", "o"],
                    ["u", ":"],
                    [" ", " "],
                    ["a", " "],
                    ["界", " "],
                    ["a", " "],
                ],
            ),
            (
                "a🌞b",
                &["Yo", "u:", "  ", "a", "🌞", "b"],
                &[
                    ["Y", "o"],
                    ["u", ":"],
                    [" ", " "],
                    ["a", " "],
                    ["🌞", " "],
                    ["b", " "],
                ],
            ),
            (
                "a\u{3000}b",
                &["Yo", "u:", "  ", "a", "\u{3000}", "b"],
                &[
                    ["Y", "o"],
                    ["u", ":"],
                    [" ", " "],
                    ["a", " "],
                    ["\u{3000}", " "],
                    ["b", " "],
                ],
            ),
            (
                "\u{301}a界b",
                &["Yo", "u:", "  \u{301}", "a", "界", "b"],
                &[
                    ["Y", "o"],
                    ["u", ":"],
                    [" ", " \u{301}"],
                    ["a", " "],
                    ["界", " "],
                    ["b", " "],
                ],
            ),
            // The spacing mark joins the LAST prefix space. Its width-two cluster must be
            // emitted intact before "s", rather than starting "s" outside the physical row.
            (
                "\u{903}s",
                &["Yo", "u:", " ", " \u{903}", "s"],
                &[
                    ["Y", "o"],
                    ["u", ":"],
                    [" ", " "],
                    [" \u{903}", " "],
                    ["s", " "],
                ],
            ),
        ];
        let background = Style::default().fg(Color::White).bg(Color::DarkGray);
        for &(body, expected_rows, expected_cells) in fixtures {
            let message = ChatMessage::user_text(body.into());
            let sources = Sources::message(&message);
            let mut preparation = Preparation::default();
            preparation.message(&message, 2, &theme);
            let affixed = format!("  {body}");
            let original = SourceLine {
                spans: vec![SourceSpan {
                    source: 0,
                    offset: 0,
                    text: &affixed,
                    style: Style::default(),
                }],
                style: Style::default(),
                alignment: Alignment::Left,
            };
            assert!(legacy_reference_is_unsafe(&[original], 2), "{body:?}");
            let block = assert_reference(
                preparation,
                sources,
                vec![Line::styled("You:", header), Line::raw(affixed)],
                2,
            );
            let actual_rows: Vec<String> = block
                .rows
                .iter()
                .map(|row| {
                    row.runs
                        .iter()
                        .map(|run| &block.source_text(run.source, sources)[run.range.clone()])
                        .collect()
                })
                .collect();
            assert_eq!(actual_rows, expected_rows, "{body:?}");
            for (index, row) in block.rows.iter().enumerate() {
                let style = if index < 2 { header } else { Style::default() };
                assert!(row.runs.iter().all(|run| run.style == style));
            }
            for start in 0..=expected_rows.len() + 1 {
                let area = Rect::new(3, 2, 2, 3);
                let mut expected = Buffer::empty(area);
                expected.set_style(area, background);
                for (y, cells) in expected_cells.iter().skip(start).take(3).enumerate() {
                    for (x, symbol) in cells.iter().enumerate() {
                        let cell = &mut expected[(3 + x as u16, 2 + y as u16)];
                        cell.set_symbol(symbol);
                        if start + y < 2 {
                            cell.set_style(header);
                        }
                    }
                }
                let mut actual = Buffer::empty(area);
                actual.set_style(area, background);
                for (y, row) in block.rows.iter().skip(start).take(3).enumerate() {
                    crate::widgets::chat_layout::paint_row(
                        &mut actual,
                        area,
                        y as u16,
                        row,
                        |slot| block.source_text(slot, sources),
                    );
                }
                assert_eq!(actual, expected, "{body:?}, start={start}");
            }
        }
    }

    #[test]
    fn generated_affixes_preserve_original_single_span_graphemes_and_controls() {
        let theme = crate::traits::test_support::marker_theme();
        let noncrossing = format!("👩{} ordinary body", "\u{200d}💻".repeat(4096));
        let crossing = format!("{}body{}", "\u{301}".repeat(4096), "\u{600}".repeat(4096));
        for body in [
            "plain words wrap here",
            "\u{301}leading combining",
            "\u{903}spacing mark",
            "\u{600}",
            "\u{301}\u{600}\u{600}",
            "trailing\u{600}\u{600}",
            "界界 👩\u{200d}💻 e\u{301}",
            "\t\u{301}controls\rlast",
            noncrossing.as_str(),
            crossing.as_str(),
        ] {
            for width in [0, 1, 2, 3, 7, 40] {
                let message = ChatMessage::user_text(body.into());
                let mut preparation = Preparation::default();
                preparation.message(&message, width, &theme);
                assert_reference(
                    preparation,
                    Sources::message(&message),
                    vec![
                        Line::styled(
                            "You:",
                            Style::default().fg(theme.user).add_modifier(Modifier::BOLD),
                        ),
                        Line::raw(format!("  {body}")),
                    ],
                    width,
                );
                let message = ChatMessage {
                    kind: ChatMessageKind::SteerEcho {
                        text: body.into(),
                        status: SteerEchoStatus::Queued,
                        message_id: None,
                        note: None,
                    },
                    timestamp: std::time::Instant::now(),
                };
                let mut preparation = Preparation::default();
                preparation.message(&message, width, &theme);
                assert_reference(
                    preparation,
                    Sources::message(&message),
                    vec![Line::styled(
                        format!("  ↳ steer: {body} — queued"),
                        Style::default()
                            .fg(theme.emphasis)
                            .add_modifier(Modifier::ITALIC),
                    )],
                    width,
                );
            }
        }
    }

    #[test]
    fn noncrossing_first_grapheme_keeps_its_original_source_range() {
        let text = format!("header\n👩{} ordinary body", "\u{200d}💻".repeat(4096));
        let message = ChatMessage::user_text(text);
        let mut preparation = Preparation::default();
        preparation.message(&message, 32, &crate::traits::test_support::marker_theme());
        let block = preparation.finish(Sources::message(&message), 32);
        assert_eq!(
            block.metrics().2,
            0,
            "a noncrossing grapheme is not generated text"
        );
        let ChatMessageKind::UserText(current) = message.kind() else {
            panic!("user source");
        };
        let body = current.split_once('\n').expect("second source line").1;
        let range = subslice(current, body);
        let slot = block
            .sources
            .iter()
            .position(|slot| matches!(slot.source, Source::MessageText) && slot.range == range)
            .expect("second line retains its authoritative selector range");
        let resolved = block.source_text(slot, Sources::message(&message));
        assert_eq!(resolved, body);
        assert_eq!(resolved.as_ptr(), body.as_ptr());
    }

    #[test]
    fn plain_preparation_retains_selectors_not_the_source_allocation() {
        let text = "authoritative plain source ".repeat(4096);
        let message = ChatMessage::system(text.clone());
        let mut preparation = Preparation::default();
        preparation.message(&message, 32, &crate::traits::test_support::marker_theme());
        let block = preparation.finish(Sources::message(&message), 32);
        assert_eq!(
            block.metrics().2,
            0,
            "plain source must not be retained as owned text"
        );
        assert!(matches!(block.sources[0].source, Source::MessageText));
        drop(message);
        let moved = ChatMessage::system(text);
        let ChatMessageKind::System(current) = moved.kind() else {
            panic!("system source");
        };
        let resolved = block.source_text(0, Sources::message(&moved));
        assert_eq!(resolved, current);
        assert_eq!(resolved.as_ptr(), current.as_ptr());
    }
}
