use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
};

/// A generic picker popup for selecting from a list of options.
#[derive(Debug)]
pub struct PickerState {
    pub title: String,
    pub options: Vec<PickerOption>,
    pub selected: usize,
    /// What kind of action to perform on confirm.
    pub action: PickerAction,
    /// Scroll offset for long lists.
    scroll_offset: usize,
}

#[derive(Debug, Clone)]
pub struct PickerOption {
    pub value: String,
    pub label: String,
    pub active: bool,
}

/// What the picker should do when an option is confirmed.
#[derive(Debug, Clone)]
pub enum PickerAction {
    SetModel,
}

impl PickerState {
    pub fn new(title: impl Into<String>, options: Vec<PickerOption>, action: PickerAction) -> Self {
        // Pre-select the active option if there is one
        let selected = options
            .iter()
            .position(|o| o.active)
            .unwrap_or(0);

        Self {
            title: title.into(),
            options,
            selected,
            action,
            scroll_offset: 0,
        }
    }

    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
            self.ensure_visible();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.options.len() - 1);
            self.ensure_visible();
        }
    }

    pub fn selected_value(&self) -> Option<&str> {
        self.options.get(self.selected).map(|o| o.value.as_str())
    }

    fn ensure_visible(&mut self) {
        // Keep selected item within the visible window
        let max_visible = 12;
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + max_visible {
            self.scroll_offset = self.selected - max_visible + 1;
        }
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &PickerState) {
    let popup_area = centered_rect(50, 60, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", state.title))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(popup_area);

    let chunks = Layout::vertical([
        Constraint::Min(1),    // options list
        Constraint::Length(1), // hint
    ])
    .split(inner);

    // Options (with scrolling)
    let visible_height = chunks[0].height as usize;
    let total = state.options.len();
    let start = state.scroll_offset;
    let end = (start + visible_height).min(total);

    let mut option_lines: Vec<Line> = Vec::new();
    for i in start..end {
        let opt = &state.options[i];
        let is_selected = i == state.selected;
        let prefix = if is_selected { "▸ " } else { "  " };

        let mut spans = vec![Span::styled(
            format!("{prefix}{}", opt.label),
            if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            },
        )];

        if opt.active {
            spans.push(Span::styled(
                " (active)",
                Style::default().fg(Color::Green),
            ));
        }

        option_lines.push(Line::from(spans));
    }
    let options_widget = Paragraph::new(option_lines);
    frame.render_widget(options_widget, chunks[0]);

    // Scrollbar if needed
    if total > visible_height {
        let mut scrollbar_state = ScrollbarState::new(total).position(state.selected);
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        frame.render_stateful_widget(scrollbar, chunks[0], &mut scrollbar_state);
    }

    // Hint
    let hint = Paragraph::new(Line::from(vec![
        Span::styled("↑↓", Style::default().fg(Color::Cyan)),
        Span::raw(" select  "),
        Span::styled("Enter", Style::default().fg(Color::Cyan)),
        Span::raw(" confirm  "),
        Span::styled("Esc", Style::default().fg(Color::Cyan)),
        Span::raw(" cancel"),
    ]))
    .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hint, chunks[1]);

    frame.render_widget(block, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let popup_layout = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
