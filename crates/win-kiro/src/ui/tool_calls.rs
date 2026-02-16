use agent_client_protocol as acp;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

/// A tracked tool call with its current status.
#[derive(Debug, Clone)]
pub struct TrackedToolCall {
    pub id: String,
    pub title: String,
    pub status: acp::ToolCallStatus,
}

/// State for the tool calls panel.
#[derive(Debug, Default)]
pub struct ToolCallsState {
    pub active_calls: Vec<TrackedToolCall>,
}

impl ToolCallsState {
    pub fn add_tool_call(&mut self, id: String, title: String) {
        self.active_calls.push(TrackedToolCall {
            id,
            title,
            status: acp::ToolCallStatus::Pending,
        });
    }

    pub fn update_tool_call(&mut self, id: &str, status: Option<acp::ToolCallStatus>, title: Option<String>) {
        if let Some(tc) = self.active_calls.iter_mut().find(|tc| tc.id == id) {
            if let Some(s) = status {
                tc.status = s;
            }
            if let Some(t) = title {
                tc.title = t;
            }
        }
    }

    pub fn clear_completed(&mut self) {
        self.active_calls.retain(|tc| {
            !matches!(tc.status, acp::ToolCallStatus::Completed | acp::ToolCallStatus::Failed)
        });
    }

    pub fn has_active(&self) -> bool {
        !self.active_calls.is_empty()
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ToolCallsState) {
    let items: Vec<ListItem> = state
        .active_calls
        .iter()
        .map(|tc| {
            let (icon, color) = match tc.status {
                acp::ToolCallStatus::Pending => ("○", Color::DarkGray),
                acp::ToolCallStatus::InProgress => ("◐", Color::Yellow),
                acp::ToolCallStatus::Completed => ("●", Color::Green),
                acp::ToolCallStatus::Failed => ("✕", Color::Red),
                _ => ("?", Color::DarkGray),
            };

            ListItem::new(Line::from(vec![
                Span::styled(format!("{icon} "), Style::default().fg(color)),
                Span::styled(&tc.title, Style::default().fg(Color::White)),
            ]))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Tools ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(list, area);
}
