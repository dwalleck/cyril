use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders},
};
use tui_textarea::TextArea;

/// State for the text input widget.
pub struct InputState {
    pub textarea: TextArea<'static>,
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
        Self { textarea }
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
        text
    }

    pub fn is_empty(&self) -> bool {
        self.textarea.lines().iter().all(|l| l.is_empty())
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &InputState) {
    frame.render_widget(&state.textarea, area);
}
