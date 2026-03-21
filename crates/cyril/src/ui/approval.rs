use agent_client_protocol as acp;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

/// State for a pending permission request.
#[derive(Debug)]
pub struct ApprovalState {
    #[allow(dead_code)]
    pub tool_call_id: String,
    pub title: Option<String>,
    /// Extra detail line (file path, command, etc.) extracted from the tool call.
    pub detail: Option<String>,
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

        // Extract detail from the tool call: file path or command being run.
        // Skip the detail if the title already contains the same info.
        let detail = Self::extract_detail(&request.tool_call).filter(|d| match &title {
            Some(t) => {
                let detail_value = d.split_once(": ").map(|(_, v)| v).unwrap_or(d);
                !t.contains(detail_value)
            }
            None => true,
        });

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
            detail,
            options,
            selected: 0,
        }
    }

    fn extract_detail(update: &acp::ToolCallUpdate) -> Option<String> {
        if let Some(ref raw) = update.fields.raw_input {
            // Shell command
            if let Some(cmd) = raw.get("command").and_then(|v| v.as_str()) {
                return Some(format!("Command: {cmd}"));
            }
            // Search query
            if let Some(query) = raw.get("query").and_then(|v| v.as_str()) {
                return Some(format!("Query: {query}"));
            }
            // URL fetch
            if let Some(url) = raw.get("url").and_then(|v| v.as_str()) {
                return Some(format!("URL: {url}"));
            }
            // File operation
            if let Some(path) = raw
                .get("file_path")
                .or_else(|| raw.get("path"))
                .and_then(|v| v.as_str())
            {
                return Some(format!("Path: {path}"));
            }
        }
        // Try locations
        if let Some(ref locations) = update.fields.locations {
            if let Some(loc) = locations.first() {
                return Some(format!("Path: {}", loc.path.display()));
            }
        }
        None
    }

    pub fn select_next(&mut self) {
        if !self.options.is_empty() {
            self.selected = (self.selected + 1) % self.options.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.options.is_empty() {
            self.selected = self
                .selected
                .checked_sub(1)
                .unwrap_or(self.options.len() - 1);
        }
    }

    pub fn selected_option_id(&self) -> Option<&str> {
        self.options.get(self.selected).map(|o| o.id.as_str())
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ApprovalState) {
    let detail_height: u16 = if state.detail.is_some() { 1 } else { 0 };
    let options_height = state.options.len() as u16;
    // 1 (title) + detail + 1 (blank separator) + options + 1 (hint) + 2 (borders)
    let content_height = 1 + detail_height + 1 + options_height + 1 + 2;

    let popup_area = centered_rect_fixed(60, content_height, area);

    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Permission Required ")
        .border_style(Style::default().fg(Color::Yellow));

    let inner = block.inner(popup_area);

    let chunks = Layout::vertical([
        Constraint::Length(1),              // title
        Constraint::Length(detail_height),  // detail (URL, command, etc.)
        Constraint::Length(1),              // blank separator
        Constraint::Length(options_height), // options
        Constraint::Length(1),              // hint
    ])
    .split(inner);

    // Title
    let desc = state
        .title
        .as_deref()
        .unwrap_or("The agent wants to perform an action");
    let desc_widget = Paragraph::new(desc).style(Style::default().fg(Color::White));
    frame.render_widget(desc_widget, chunks[0]);

    // Detail line (URL, command, file path, etc.)
    if let Some(ref detail) = state.detail {
        let detail_widget = Paragraph::new(detail.as_str())
            .style(Style::default().fg(Color::Cyan))
            .wrap(Wrap { trim: true });
        frame.render_widget(detail_widget, chunks[1]);
    }

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
    frame.render_widget(options_widget, chunks[3]);

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
    frame.render_widget(hint, chunks[4]);

    frame.render_widget(block, popup_area);
}

/// Center a popup with a percentage-based width and a fixed pixel height.
fn centered_rect_fixed(percent_x: u16, height: u16, area: Rect) -> Rect {
    let clamped_height = height.min(area.height);
    let vertical_padding = area.height.saturating_sub(clamped_height) / 2;

    let popup_layout = Layout::vertical([
        Constraint::Length(vertical_padding),
        Constraint::Length(clamped_height),
        Constraint::Min(0),
    ])
    .split(area);

    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(popup_layout[1])[1]
}
