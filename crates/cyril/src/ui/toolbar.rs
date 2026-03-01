use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use cyril_core::session::SessionContext;

/// State for the toolbar/status bar.
#[derive(Debug, Default)]
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub is_busy: bool,
    /// The --agent value passed at startup (e.g. "sonnet").
    pub selected_agent: Option<String>,
    /// Whether mouse capture is active (false = copy mode).
    pub mouse_captured: bool,
}

pub fn render(frame: &mut Frame, area: Rect, state: &ToolbarState, session: &SessionContext) {
    let status = if state.is_busy { "working..." } else { "ready" };
    let status_color = if state.is_busy {
        Color::Yellow
    } else {
        Color::Green
    };

    let session_id_string = session
        .id
        .as_ref()
        .map(|id| id.to_string())
        .unwrap_or_default();
    let session_display = if session_id_string.is_empty() {
        "none"
    } else {
        &session_id_string[..8.min(session_id_string.len())]
    };

    let mut spans = vec![
        Span::styled(
            " cyril ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
        Span::styled(&state.agent_name, Style::default().fg(Color::White)),
        Span::styled(
            format!(" v{}", state.agent_version),
            Style::default().fg(Color::DarkGray),
        ),
    ];

    let mode_display = session
        .current_mode_id
        .as_ref()
        .or(state.selected_agent.as_ref());
    if let Some(mode) = mode_display {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{mode}]"),
            Style::default().fg(Color::Magenta),
        ));
    }

    let current_model = session.current_model();
    if let Some(ref model) = current_model {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("({model})"),
            Style::default().fg(Color::Blue),
        ));
    }

    spans.push(Span::raw(" | "));
    spans.push(Span::styled(
        format!("session: {session_display}"),
        Style::default().fg(Color::DarkGray),
    ));

    if !state.mouse_captured {
        spans.push(Span::raw(" | "));
        spans.push(Span::styled(
            "COPY",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    spans.push(Span::raw(" | "));
    spans.push(Span::styled(status, Style::default().fg(status_color)));

    let line = Line::from(spans);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}

/// Render a compact context usage gauge (1 row, 40 chars, left-aligned).
pub fn render_context_bar(frame: &mut Frame, area: Rect, pct: f64) {
    let bar_color = if pct > 80.0 {
        Color::Red
    } else if pct > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let label_width: u16 = 8; // "context "
    let gauge_width: u16 = 32;

    // Render "context " label
    let label_area = Rect::new(area.x, area.y, label_width.min(area.width), 1);
    let label = Paragraph::new(Span::styled("context ", Style::default().fg(Color::Gray)));
    frame.render_widget(label, label_area);

    // Render gauge bar
    let gauge_x = area.x + label_width;
    let gauge_area = Rect::new(
        gauge_x,
        area.y,
        gauge_width.min(area.width.saturating_sub(label_width)),
        1,
    );
    let ratio = (pct / 100.0).clamp(0.0, 1.0);

    let gauge = Gauge::default()
        .gauge_style(Style::default().fg(bar_color).bg(Color::Rgb(60, 60, 60)))
        .ratio(ratio)
        .label(Span::styled(
            format!("{pct:.0}%"),
            Style::default().fg(Color::White),
        ))
        .use_unicode(true);

    frame.render_widget(gauge, gauge_area);
}
