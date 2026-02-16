use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

/// A single message in the chat history.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: Role,
    pub content: String,
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
    /// Content currently being streamed from the agent.
    pub streaming_content: String,
    /// Whether the agent is currently streaming.
    pub is_streaming: bool,
    /// Vertical scroll offset.
    pub scroll_offset: u16,
}

impl ChatState {
    pub fn add_user_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::User,
            content,
        });
    }

    pub fn begin_streaming(&mut self) {
        self.is_streaming = true;
        self.streaming_content.clear();
    }

    pub fn append_streaming(&mut self, text: &str) {
        self.streaming_content.push_str(text);
    }

    pub fn finish_streaming(&mut self) {
        if !self.streaming_content.is_empty() {
            self.messages.push(ChatMessage {
                role: Role::Agent,
                content: std::mem::take(&mut self.streaming_content),
            });
        }
        self.is_streaming = false;
    }

    #[allow(dead_code)]
    pub fn add_system_message(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: Role::System,
            content,
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

    // Build text lines from messages
    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let (label, label_style) = match msg.role {
            Role::User => ("You", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Role::Agent => ("Kiro", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Role::System => ("System", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        };

        lines.push(Line::from(Span::styled(
            format!("{label}:"),
            label_style,
        )));

        // Render content as plain text lines
        for text_line in msg.content.lines() {
            lines.push(Line::from(text_line.to_string()));
        }
        lines.push(Line::from(""));
    }

    // Add streaming content if active
    if state.is_streaming && !state.streaming_content.is_empty() {
        lines.push(Line::from(Span::styled(
            "Kiro:",
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        )));
        for text_line in state.streaming_content.lines() {
            lines.push(Line::from(text_line.to_string()));
        }
    } else if state.is_streaming {
        lines.push(Line::from(Span::styled(
            "Kiro is thinking...",
            Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
        )));
    }

    let total_lines = lines.len() as u16;
    let visible_height = inner.height;

    // Auto-scroll to bottom when scroll_offset is 0
    let scroll = if state.scroll_offset == 0 {
        total_lines.saturating_sub(visible_height)
    } else {
        total_lines
            .saturating_sub(visible_height)
            .saturating_sub(state.scroll_offset)
    };

    let paragraph = Paragraph::new(Text::from(lines))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    frame.render_widget(paragraph, area);

    // Scrollbar
    if total_lines > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total_lines as usize)
            .position(scroll as usize);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}
