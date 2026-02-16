use agent_client_protocol as acp;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

/// State for a pending permission request.
#[derive(Debug)]
pub struct ApprovalState {
    #[allow(dead_code)]
    pub tool_call_id: String,
    pub title: Option<String>,
    pub options: Vec<ApprovalOption>,
    pub selected: usize,
}

#[derive(Debug)]
pub struct ApprovalOption {
    pub id: String,
    pub name: String,
    #[allow(dead_code)]
    pub kind: acp::PermissionOptionKind,
}

impl ApprovalState {
    pub fn from_request(request: &acp::RequestPermissionRequest) -> Self {
        let title = request.tool_call.fields.title.clone();
        let options: Vec<ApprovalOption> = request
            .options
            .iter()
            .map(|o| ApprovalOption {
                id: o.option_id.to_string(),
                name: o.name.clone(),
                kind: o.kind.clone(),
            })
            .collect();

        Self {
            tool_call_id: request.tool_call.tool_call_id.to_string(),
            title,
            options,
            selected: 0,
        }
    }

    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = self.selected.checked_sub(1).unwrap_or(self.options.len() - 1);
        }
    }

    pub fn selected_option_id(&self) -> Option<&str> {
        self.options.get(self.selected).map(|o| o.id.as_str())
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ApprovalState) {
    // Center the popup
    let popup_area = centered_rect(60, 40, area);

    // Clear the area behind the popup
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Permission Required ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(3), // description
        Constraint::Min(1),   // options
        Constraint::Length(1), // hint
    ])
    .split(inner);

    // Description
    let desc = state
        .title
        .as_deref()
        .unwrap_or("The agent wants to perform an action");
    let desc_widget = Paragraph::new(desc)
        .style(Style::default().fg(Color::White))
        .wrap(Wrap { trim: true });
    frame.render_widget(desc_widget, chunks[0]);

    // Options
    let mut option_lines: Vec<Line> = Vec::new();
    for (i, opt) in state.options.iter().enumerate() {
        let is_selected = i == state.selected;
        let prefix = if is_selected { "▸ " } else { "  " };
        let style = if is_selected {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::White)
        };
        option_lines.push(Line::from(Span::styled(
            format!("{prefix}{}", opt.name),
            style,
        )));
    }
    let options_widget = Paragraph::new(option_lines);
    frame.render_widget(options_widget, chunks[1]);

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
    frame.render_widget(hint, chunks[2]);

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
