use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tui_textarea::TextArea;

use crate::commands::{self, AgentCommand, Suggestion};

/// State for the text input widget.
pub struct InputState {
    pub textarea: TextArea<'static>,
    /// Index of the currently highlighted autocomplete suggestion.
    pub autocomplete_selected: usize,
    /// Agent-provided commands (updated via ACP notifications).
    pub agent_commands: Vec<AgentCommand>,
}

impl Default for InputState {
    fn default() -> Self {
        let mut textarea = TextArea::default();
        textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Prompt (Enter to send, Shift+Enter for newline) "),
        );
        textarea.set_cursor_line_style(Style::default());
        textarea.set_style(Style::default().fg(Color::White));
        Self {
            textarea,
            autocomplete_selected: 0,
            agent_commands: Vec::new(),
        }
    }
}

impl InputState {
    /// Get the current input text and clear the textarea.
    pub fn take_input(&mut self) -> String {
        let text = self.textarea.lines().join("\n");
        self.textarea = TextArea::default();
        self.textarea.set_block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Prompt (Enter to send, Shift+Enter for newline) "),
        );
        self.textarea.set_cursor_line_style(Style::default());
        self.textarea.set_style(Style::default().fg(Color::White));
        self.autocomplete_selected = 0;
        text
    }

    pub fn is_empty(&self) -> bool {
        self.textarea.lines().iter().all(|l| l.is_empty())
    }

    /// Get the current single-line input text.
    pub fn current_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Get autocomplete suggestions for current input.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let text = self.current_text();
        let trimmed = text.trim();
        // Only show suggestions when typing a command (starts with / and no space yet)
        if trimmed.starts_with('/') && !trimmed.contains(' ') {
            commands::matching_suggestions(trimmed, &self.agent_commands)
        } else {
            Vec::new()
        }
    }

    /// Apply the selected autocomplete suggestion.
    pub fn apply_suggestion(&mut self) {
        let suggestions = self.suggestions();
        if let Some(cmd) = suggestions.get(self.autocomplete_selected) {
            // Replace input with the command
            let new_text = if cmd.takes_arg {
                format!("{} ", cmd.display_name)
            } else {
                cmd.display_name.clone()
            };
            self.textarea = TextArea::default();
            self.textarea.set_block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Prompt (Enter to send, Shift+Enter for newline) "),
            );
            self.textarea.set_cursor_line_style(Style::default());
            self.textarea.set_style(Style::default().fg(Color::White));
            self.textarea.insert_str(&new_text);
            self.autocomplete_selected = 0;
        }
    }

    pub fn autocomplete_up(&mut self) {
        let count = self.suggestions().len();
        if count > 0 {
            self.autocomplete_selected = self
                .autocomplete_selected
                .checked_sub(1)
                .unwrap_or(count - 1);
        }
    }

    pub fn autocomplete_down(&mut self) {
        let count = self.suggestions().len();
        if count > 0 {
            self.autocomplete_selected = (self.autocomplete_selected + 1) % count;
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &InputState) {
    frame.render_widget(&state.textarea, area);

    // Render autocomplete popup above the input
    let suggestions = state.suggestions();
    if !suggestions.is_empty() {
        let popup_height = suggestions.len() as u16 + 2; // +2 for borders
        let popup_y = area.y.saturating_sub(popup_height);
        let popup_area = Rect::new(area.x, popup_y, area.width.min(50), popup_height);

        frame.render_widget(Clear, popup_area);

        let lines: Vec<Line> = suggestions
            .iter()
            .enumerate()
            .map(|(i, cmd)| {
                let is_selected = i == state.autocomplete_selected;
                let style = if is_selected {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                Line::from(vec![
                    Span::styled(
                        format!("{:<20}", cmd.display_name),
                        style,
                    ),
                    Span::styled(
                        &cmd.description,
                        Style::default().fg(Color::DarkGray),
                    ),
                ])
            })
            .collect();

        let popup = Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        );

        frame.render_widget(popup, popup_area);
    }
}
