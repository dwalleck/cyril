use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};
use tui_textarea::{CursorMove, TextArea};

use crate::commands::{self, AgentCommand, Suggestion};
use crate::file_completer::{self, AtContext, FileCompleter, FileSuggestion};

/// Which autocomplete popup is active.
pub enum ActivePopup {
    None,
    SlashCommand,
    FilePath(AtContext),
}

/// Create a fresh textarea with standard styling.
fn make_textarea() -> TextArea<'static> {
    let mut textarea = TextArea::default();
    textarea.set_block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Prompt (Enter to send, Shift+Enter for newline) "),
    );
    textarea.set_cursor_line_style(Style::default());
    textarea.set_style(Style::default().fg(Color::White));
    textarea
}

/// State for the text input widget.
pub struct InputState {
    pub textarea: TextArea<'static>,
    /// Index of the currently highlighted autocomplete suggestion.
    pub autocomplete_selected: usize,
    /// Agent-provided commands (updated via ACP notifications).
    pub agent_commands: Vec<AgentCommand>,
    /// File completer for @-triggered autocomplete.
    pub file_completer: Option<FileCompleter>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            textarea: make_textarea(),
            autocomplete_selected: 0,
            agent_commands: Vec::new(),
            file_completer: None,
        }
    }
}

impl InputState {
    /// Get the current input text and clear the textarea.
    pub fn take_input(&mut self) -> String {
        let text = self.textarea.lines().join("\n");
        self.textarea = make_textarea();
        self.autocomplete_selected = 0;
        text
    }

    pub fn is_empty(&self) -> bool {
        self.textarea.lines().iter().all(|l| l.is_empty())
    }

    /// Get the current input text.
    pub fn current_text(&self) -> String {
        self.textarea.lines().join("\n")
    }

    /// Determine which autocomplete popup should be active.
    pub fn active_popup(&self) -> ActivePopup {
        // Slash commands take priority
        if !self.command_suggestions().is_empty() {
            return ActivePopup::SlashCommand;
        }

        // Check for @-trigger
        if self.file_completer.is_some() {
            let (row, col) = self.textarea.cursor();
            let lines: Vec<String> = self.textarea.lines().iter().map(|s| s.to_string()).collect();
            if let Some(ctx) = file_completer::find_at_trigger(&lines, row, col) {
                if !ctx.query.is_empty() {
                    return ActivePopup::FilePath(ctx);
                }
            }
        }

        ActivePopup::None
    }

    /// Get slash command autocomplete suggestions for current input.
    pub fn command_suggestions(&self) -> Vec<Suggestion> {
        let text = self.current_text();
        let trimmed = text.trim();
        if trimmed.starts_with('/') && !trimmed.contains(' ') {
            commands::matching_suggestions(trimmed, &self.agent_commands)
        } else {
            Vec::new()
        }
    }

    /// Get file path suggestions for the given @-trigger context.
    pub fn file_suggestions(&mut self, ctx: &AtContext) -> Vec<FileSuggestion> {
        if let Some(ref mut completer) = self.file_completer {
            completer.suggestions(&ctx.query, 10)
        } else {
            Vec::new()
        }
    }

    /// Returns true if any popup has results (used for key routing).
    pub fn has_suggestions(&mut self) -> bool {
        match self.active_popup() {
            ActivePopup::None => false,
            ActivePopup::SlashCommand => true, // already checked non-empty in active_popup
            ActivePopup::FilePath(ctx) => !self.file_suggestions(&ctx).is_empty(),
        }
    }

    /// Apply the selected autocomplete suggestion based on which popup is active.
    pub fn apply_suggestion(&mut self) {
        match self.active_popup() {
            ActivePopup::None => {}
            ActivePopup::SlashCommand => self.apply_command_suggestion(),
            ActivePopup::FilePath(ctx) => self.apply_file_suggestion(&ctx),
        }
    }

    fn apply_command_suggestion(&mut self) {
        let suggestions = self.command_suggestions();
        if let Some(cmd) = suggestions.get(self.autocomplete_selected) {
            let new_text = if cmd.takes_arg {
                format!("{} ", cmd.display_name)
            } else {
                cmd.display_name.clone()
            };
            self.textarea = make_textarea();
            self.textarea.insert_str(&new_text);
            self.autocomplete_selected = 0;
        }
    }

    fn apply_file_suggestion(&mut self, ctx: &AtContext) {
        let suggestions = self.file_suggestions(ctx);
        if let Some(file) = suggestions.get(self.autocomplete_selected) {
            let lines: Vec<String> = self.textarea.lines().iter().map(|s| s.to_string()).collect();
            let row = ctx.row;

            if let Some(line) = lines.get(row) {
                // Build the new line: everything before '@' + '@path ' + everything after the query
                let before = &line[..ctx.at_col];
                let after = &line[ctx.cursor_col..];
                let replacement = format!("@{} ", file.path);
                let new_line = format!("{before}{replacement}{after}");

                // Rebuild all lines with this replacement
                let mut new_lines: Vec<String> = lines.clone();
                new_lines[row] = new_line;
                let full_text = new_lines.join("\n");

                // Calculate where the cursor should end up
                let cursor_target_col = ctx.at_col + replacement.len();

                // Recreate textarea with new content
                self.textarea = make_textarea();
                self.textarea.insert_str(&full_text);

                // Move cursor to the correct position:
                // After insert_str, cursor is at the end. Move to target position.
                self.textarea.move_cursor(CursorMove::Top);
                self.textarea.move_cursor(CursorMove::Head);
                for _ in 0..row {
                    self.textarea.move_cursor(CursorMove::Down);
                }
                for _ in 0..cursor_target_col {
                    self.textarea.move_cursor(CursorMove::Forward);
                }

                self.autocomplete_selected = 0;
            }
        }
    }

    pub fn autocomplete_up(&mut self) {
        let count = self.suggestion_count();
        if count > 0 {
            self.autocomplete_selected = self
                .autocomplete_selected
                .checked_sub(1)
                .unwrap_or(count - 1);
        }
    }

    pub fn autocomplete_down(&mut self) {
        let count = self.suggestion_count();
        if count > 0 {
            self.autocomplete_selected = (self.autocomplete_selected + 1) % count;
        }
    }

    fn suggestion_count(&mut self) -> usize {
        match self.active_popup() {
            ActivePopup::None => 0,
            ActivePopup::SlashCommand => self.command_suggestions().len(),
            ActivePopup::FilePath(ctx) => self.file_suggestions(&ctx).len(),
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &mut InputState) {
    frame.render_widget(&state.textarea, area);

    match state.active_popup() {
        ActivePopup::None => {}
        ActivePopup::SlashCommand => render_command_popup(frame, area, state),
        ActivePopup::FilePath(ctx) => render_file_popup(frame, area, state, &ctx),
    }
}

fn render_command_popup(frame: &mut Frame, area: Rect, state: &InputState) {
    let suggestions = state.command_suggestions();
    if suggestions.is_empty() {
        return;
    }

    let popup_height = suggestions.len() as u16 + 2;
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
                Span::styled(format!("{:<20}", cmd.display_name), style),
                Span::styled(&cmd.description, Style::default().fg(Color::DarkGray)),
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

fn render_file_popup(frame: &mut Frame, area: Rect, state: &mut InputState, ctx: &AtContext) {
    let suggestions = state.file_suggestions(ctx);
    if suggestions.is_empty() {
        return;
    }

    let popup_height = (suggestions.len() as u16).min(10) + 2;
    let popup_y = area.y.saturating_sub(popup_height);
    let popup_area = Rect::new(area.x, popup_y, area.width.min(60), popup_height);

    frame.render_widget(Clear, popup_area);

    let lines: Vec<Line> = suggestions
        .iter()
        .enumerate()
        .map(|(i, file)| {
            let is_selected = i == state.autocomplete_selected;
            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };
            Line::from(Span::styled(&file.path, style))
        })
        .collect();

    let popup = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Files ")
            .border_style(Style::default().fg(Color::Blue)),
    );

    frame.render_widget(popup, popup_area);
}
