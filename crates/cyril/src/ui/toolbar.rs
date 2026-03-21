use std::time::Instant;

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Gauge, Paragraph},
    Frame,
};

use cyril_core::session::SessionContext;

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

/// State for the toolbar/status bar.
#[derive(Debug, Default)]
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub is_busy: bool,
    /// Current tool activity detail (e.g. "reading file.rs", "executing shell").
    /// Shown in the toolbar instead of generic "working..." when present.
    pub busy_detail: Option<String>,
    /// When the current busy period started (for elapsed time display).
    pub busy_since: Option<Instant>,
    /// The --agent value passed at startup (e.g. "sonnet").
    pub selected_agent: Option<String>,
    /// Whether mouse capture is active (false = copy mode).
    pub mouse_captured: bool,
}

pub fn render(frame: &mut Frame, area: Rect, state: &ToolbarState, session: &SessionContext) {
    let status: String;
    let status_color;

    if state.is_busy {
        let detail = state.busy_detail.as_deref().unwrap_or("working");
        let elapsed = state.busy_since.map(|t| t.elapsed().as_secs()).unwrap_or(0);
        let spinner_idx = if let Some(t) = state.busy_since {
            (t.elapsed().as_millis() / 80) as usize % SPINNER_FRAMES.len()
        } else {
            0
        };
        let spinner = SPINNER_FRAMES[spinner_idx];
        status = format!("{spinner} {detail} ({elapsed}s)");
        status_color = Color::Yellow;
    } else {
        status = "ready".to_string();
        status_color = Color::Green;
    };

    let session_display: &str = match session.id.as_ref() {
        None => "none",
        Some(id) => {
            let s: &str = &id.0;
            &s[..8.min(s.len())]
        }
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
        .current_mode_id()
        .or(state.selected_agent.as_deref());
    if let Some(mode) = mode_display {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format!("[{mode}]"),
            Style::default().fg(Color::Magenta),
        ));
    }

    if let Some(model) = session.current_model() {
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
    spans.push(Span::styled(
        status.clone(),
        Style::default().fg(status_color),
    ));

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
