use agent_client_protocol as acp;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

use super::tool_calls::{self, TrackedToolCall};

/// A content block within a message or streaming response.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    Text(String),
    ToolCall(TrackedToolCall),
}

/// A single message in the chat history.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub blocks: Vec<ContentBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Role {
    User,
    Agent,
    #[allow(dead_code)]
    System,
}

/// State for the chat display.
#[derive(Debug, Default)]
pub struct ChatState {
    pub messages: Vec<ChatMessage>,
    /// Ordered content blocks being streamed (text interleaved with tool calls).
    pub stream_blocks: Vec<ContentBlock>,
    /// Whether the agent is currently streaming.
    pub is_streaming: bool,
    /// Vertical scroll offset.
    pub scroll_offset: u16,
}

impl ChatState {
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::User,
            blocks: vec![ContentBlock::Text(content)],
        });
    }

    pub fn begin_streaming(&mut self) {
        self.is_streaming = true;
        self.stream_blocks.clear();
    }

    pub fn append_streaming(&mut self, text: &str) {
        if let Some(ContentBlock::Text(ref mut s)) = self.stream_blocks.last_mut() {
            s.push_str(text);
        } else {
            self.stream_blocks
                .push(ContentBlock::Text(text.to_string()));
        }
    }

    pub fn add_tool_call(&mut self, tool_call: acp::ToolCall) {
        self.stream_blocks
            .push(ContentBlock::ToolCall(TrackedToolCall::new(tool_call)));
    }

    pub fn update_tool_call(&mut self, update: acp::ToolCallUpdate) {
        // Search backwards since recent tool calls are most likely to be updated
        for block in self.stream_blocks.iter_mut().rev() {
            if let ContentBlock::ToolCall(ref mut tc) = block {
                if *tc.id() == update.tool_call_id {
                    tc.apply_update(update.fields);
                    return;
                }
            }
        }
    }

    pub fn finish_streaming(&mut self) {
        if !self.stream_blocks.is_empty() {
            let blocks = std::mem::take(&mut self.stream_blocks);
            self.messages.push(ChatMessage {
                role: Role::Agent,
                blocks,
            });
        }
        self.is_streaming = false;
    }

    #[allow(dead_code)]
    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::System,
            blocks: vec![ContentBlock::Text(content)],
        });
    }

    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_add(3);
    }

    pub fn scroll_down(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(3);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ChatState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat ");

    let inner = block.inner(area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let (label, label_style) = role_style(&msg.role);

        lines.push(Line::from(Span::styled(
            format!("{label}:"),
            label_style,
        )));

        render_blocks(&msg.blocks, &msg.role, &mut lines);
        lines.push(Line::from(""));
    }

    // Streaming content
    if state.is_streaming && !state.stream_blocks.is_empty() {
        lines.push(Line::from(Span::styled(
            "Kiro:",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )));
        render_blocks(&state.stream_blocks, &Role::Agent, &mut lines);
    } else if state.is_streaming {
        lines.push(Line::from(Span::styled(
            "Kiro is thinking...",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::ITALIC),
        )));
    }

    let visible_height = inner.height as usize;

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false });

    let total_lines = paragraph.line_count(area.width);

    let scroll = if state.scroll_offset == 0 {
        total_lines.saturating_sub(visible_height)
    } else {
        total_lines
            .saturating_sub(visible_height)
            .saturating_sub(state.scroll_offset as usize)
    };

    let scroll_u16 = scroll.min(u16::MAX as usize) as u16;
    let paragraph = paragraph.scroll((scroll_u16, 0));

    frame.render_widget(paragraph, area);

    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines).position(scroll);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

fn role_style(role: &Role) -> (&'static str, Style) {
    match role {
        Role::User => (
            "You",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Role::Agent => (
            "Kiro",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Role::System => (
            "System",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    }
}

fn render_blocks(blocks: &[ContentBlock], role: &Role, lines: &mut Vec<Line<'static>>) {
    for block in blocks {
        match block {
            ContentBlock::Text(text) => {
                if *role == Role::Agent {
                    lines.extend(super::markdown::render(text));
                } else {
                    for text_line in text.lines() {
                        lines.push(Line::from(text_line.to_string()));
                    }
                }
            }
            ContentBlock::ToolCall(tc) => {
                // Skip tool calls with kind=Other — these are "planning" steps
                // from the agent that lack useful info (no path, no diff).
                if tc.kind() != acp::ToolKind::Other {
                    lines.extend(tool_calls::render_lines(tc));
                }
            }
        }
    }
}
