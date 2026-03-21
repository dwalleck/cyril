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

/// What the toolbar is currently showing for the activity indicator.
#[derive(Debug, Clone)]
pub enum ToolbarActivity {
    /// Agent is idle, ready for input.
    Ready,
    /// Prompt sent, waiting for first response.
    Waiting { since: Instant },
    /// A tool is running (reading, executing, searching).
    ToolCall { detail: String, since: Instant },
    /// Agent is streaming text -- the content itself is the indicator.
    Streaming,
}

impl Default for ToolbarActivity {
    fn default() -> Self {
        Self::Ready
    }
}

/// State for the toolbar/status bar.
#[derive(Debug, Default)]
pub struct ToolbarState {
    pub agent_name: String,
    pub agent_version: String,
    pub activity: ToolbarActivity,
    /// The --agent value passed at startup (e.g. "sonnet").
    pub selected_agent: Option<String>,
    /// Whether mouse capture is active (false = copy mode).
    pub mouse_captured: bool,
}

impl ToolbarState {
    pub fn on_prompt_sent(&mut self) {
        self.activity = ToolbarActivity::Waiting {
            since: Instant::now(),
        };
    }

    pub fn on_tool_call_chunk(&mut self, detail: impl Into<String>) {
        self.activity = ToolbarActivity::ToolCall {
            detail: detail.into(),
            since: Instant::now(),
        };
    }

    pub fn on_agent_message(&mut self) {
        self.activity = ToolbarActivity::Streaming;
    }

    pub fn on_turn_end(&mut self) {
        self.activity = ToolbarActivity::Ready;
    }

    pub fn is_busy(&self) -> bool {
        !matches!(self.activity, ToolbarActivity::Ready)
    }
}

pub fn render(frame: &mut Frame, area: Rect, state: &ToolbarState, session: &SessionContext) {
    let (status, status_color) = match &state.activity {
        ToolbarActivity::Ready => ("ready".to_string(), Color::Green),
        ToolbarActivity::Waiting { since } => {
            let elapsed = since.elapsed().as_secs();
            let idx = (since.elapsed().as_millis() / 80) as usize % SPINNER_FRAMES.len();
            (
                format!("{} working ({elapsed}s)", SPINNER_FRAMES[idx]),
                Color::Yellow,
            )
        }
        ToolbarActivity::ToolCall { detail, since } => {
            let elapsed = since.elapsed().as_secs();
            let idx = (since.elapsed().as_millis() / 80) as usize % SPINNER_FRAMES.len();
            (
                format!("{} {detail} ({elapsed}s)", SPINNER_FRAMES[idx]),
                Color::Yellow,
            )
        }
        ToolbarActivity::Streaming => ("streaming".to_string(), Color::Cyan),
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
    spans.push(Span::styled(status, Style::default().fg(status_color)));

    let line = Line::from(spans);

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}

/// Render the status bar with context usage gauge and optional credit gauge.
pub fn render_status_bar(frame: &mut Frame, area: Rect, session: &SessionContext) {
    let context_pct = session.context_usage_pct().unwrap_or(0.0);

    // Context gauge
    render_gauge(frame, area, 0, "context ", context_pct);

    // Credit gauge (right side)
    if let Some(credit) = session.credit_usage() {
        let credit_pct = if credit.limit > 0.0 {
            (credit.used / credit.limit * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let label = format!("credits {:.0}/{:.0} ", credit.used, credit.limit);
        let label_width = label.len() as u16;
        let gauge_width: u16 = 20;
        let total = label_width + gauge_width;
        let start_x = area.width.saturating_sub(total);

        let label_area = Rect::new(area.x + start_x, area.y, label_width.min(area.width), 1);
        let label_widget = Paragraph::new(Span::styled(label, Style::default().fg(Color::Gray)));
        frame.render_widget(label_widget, label_area);

        render_gauge(frame, area, (start_x + label_width) as u16, "", credit_pct);
    }
}

fn render_gauge(frame: &mut Frame, area: Rect, x_offset: u16, label_text: &str, pct: f64) {
    let bar_color = if pct > 80.0 {
        Color::Red
    } else if pct > 50.0 {
        Color::Yellow
    } else {
        Color::Green
    };

    let label_width = label_text.len() as u16;
    let gauge_width: u16 = 32;

    if !label_text.is_empty() {
        let label_area = Rect::new(area.x + x_offset, area.y, label_width.min(area.width), 1);
        let label = Paragraph::new(Span::styled(label_text, Style::default().fg(Color::Gray)));
        frame.render_widget(label, label_area);
    }

    let gauge_x = area.x + x_offset + label_width;
    let gauge_area = Rect::new(
        gauge_x,
        area.y,
        gauge_width.min(area.width.saturating_sub(gauge_x)),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_is_ready() {
        let state = ToolbarState::default();
        assert!(matches!(state.activity, ToolbarActivity::Ready));
    }

    #[test]
    fn prompt_sent_transitions_to_waiting() {
        let mut state = ToolbarState::default();
        state.on_prompt_sent();
        assert!(matches!(state.activity, ToolbarActivity::Waiting { .. }));
    }

    #[test]
    fn tool_call_from_waiting() {
        let mut state = ToolbarState::default();
        state.on_prompt_sent();
        state.on_tool_call_chunk("reading file.rs");
        match &state.activity {
            ToolbarActivity::ToolCall { detail, .. } => {
                assert_eq!(detail, "reading file.rs");
            }
            other => panic!("Expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn agent_message_transitions_to_streaming() {
        let mut state = ToolbarState::default();
        state.on_prompt_sent();
        state.on_agent_message();
        assert!(matches!(state.activity, ToolbarActivity::Streaming));
    }

    #[test]
    fn tool_call_from_streaming() {
        let mut state = ToolbarState::default();
        state.on_prompt_sent();
        state.on_agent_message();
        state.on_tool_call_chunk("executing shell");
        match &state.activity {
            ToolbarActivity::ToolCall { detail, .. } => {
                assert_eq!(detail, "executing shell");
            }
            other => panic!("Expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn turn_end_from_any_state() {
        for start in ["waiting", "tool_call", "streaming"] {
            let mut state = ToolbarState::default();
            state.on_prompt_sent();
            match start {
                "tool_call" => state.on_tool_call_chunk("test"),
                "streaming" => state.on_agent_message(),
                _ => {}
            }
            state.on_turn_end();
            assert!(matches!(state.activity, ToolbarActivity::Ready));
        }
    }

    #[test]
    fn tool_call_resets_timer() {
        let mut state = ToolbarState::default();
        state.on_prompt_sent();
        state.on_tool_call_chunk("first");
        let first_since = match &state.activity {
            ToolbarActivity::ToolCall { since, .. } => *since,
            _ => panic!("Expected ToolCall"),
        };
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.on_tool_call_chunk("second");
        let second_since = match &state.activity {
            ToolbarActivity::ToolCall { since, .. } => *since,
            _ => panic!("Expected ToolCall"),
        };
        assert!(second_since > first_since);
    }

    #[test]
    fn is_busy_reflects_activity() {
        let mut state = ToolbarState::default();
        assert!(!state.is_busy());
        state.on_prompt_sent();
        assert!(state.is_busy());
        state.on_agent_message();
        assert!(state.is_busy());
        state.on_turn_end();
        assert!(!state.is_busy());
    }
}
