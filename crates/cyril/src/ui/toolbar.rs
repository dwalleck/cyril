use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// State for the toolbar/status bar.
#[derive(Debug, Default)]
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub session_id: Option<String>,
    pub is_busy: bool,
}

pub fn render(frame: &mut Frame, area: Rect, state: &ToolbarState) {
    let status = if state.is_busy { "working..." } else { "ready" };
    let status_color = if state.is_busy { Color::Yellow } else { Color::Green };

    let session_display = state
        .session_id
        .as_deref()
        .map(|id| &id[..8.min(id.len())])
        .unwrap_or("none");

    let line = Line::from(vec![
        Span::styled(" cyril ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(&state.agent_name, Style::default().fg(Color::White)),
        Span::styled(
            format!(" v{}", state.agent_version),
            Style::default().fg(Color::DarkGray),
        ),
        Span::raw(" | "),
        Span::styled(format!("session: {session_display}"), Style::default().fg(Color::DarkGray)),
        Span::raw(" | "),
        Span::styled(status, Style::default().fg(status_color)),
    ]);

    let paragraph = Paragraph::new(line)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}
