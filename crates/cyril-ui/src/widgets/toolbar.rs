use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::palette;
use crate::traits::{Activity, TuiState};

/// Render the toolbar (top line).
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut parts: Vec<Span> = Vec::new();

    // Activity indicator
    match state.activity() {
        Activity::Idle | Activity::Ready => {}
        Activity::Sending | Activity::Waiting => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(Color::Yellow),
            ));
        }
        Activity::Streaming => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(Color::Green),
            ));
        }
        Activity::ToolRunning => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(Color::Cyan),
            ));
        }
    }

    // Session label
    if let Some(label) = state.session_label() {
        parts.push(Span::styled(
            label.to_string(),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));
    } else {
        parts.push(Span::styled(
            "No session",
            Style::default().fg(Color::DarkGray),
        ));
    }

    // Mode
    if let Some(mode) = state.current_mode() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            mode.to_string(),
            Style::default().fg(Color::Cyan),
        ));
    }

    // Model
    if let Some(model) = state.current_model() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            model.to_string(),
            Style::default().fg(Color::Magenta),
        ));
    }

    // Code intelligence indicator
    if state.code_intelligence_active() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            "✦ code intel",
            Style::default().fg(Color::Cyan),
        ));
    }

    // Elapsed time for active operations
    if let Some(elapsed) = state.activity_elapsed() {
        let secs = elapsed.as_secs();
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            format!("{secs}s"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(parts);
    let toolbar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 46)));

    frame.render_widget(toolbar, area);
}

/// Render the bottom status bar (context usage + credits).
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &dyn TuiState) {
    let mut parts: Vec<Span> = Vec::new();

    // Context usage gauge
    if let Some(pct) = state.context_usage() {
        let color = if pct > 90.0 {
            Color::Red
        } else if pct > 70.0 {
            Color::Yellow
        } else {
            Color::Green
        };
        parts.push(Span::styled(
            format!("Context: {pct:.0}%"),
            Style::default().fg(color),
        ));
    }

    // Stop reason warning (when last turn didn't end normally)
    if let Some(turn) = state.last_turn() {
        use cyril_core::types::StopReason;
        let (label, color) = match turn.stop_reason() {
            StopReason::EndTurn => ("", Color::White),
            StopReason::MaxTokens => ("Token limit", Color::Yellow),
            StopReason::MaxTurnRequests => ("Turn limit", Color::Yellow),
            StopReason::Refusal => ("Refused", Color::Red),
            StopReason::Cancelled => ("Cancelled", Color::DarkGray),
        };
        if !label.is_empty() {
            if !parts.is_empty() {
                parts.push(Span::raw(" · "));
            }
            parts.push(Span::styled(
                label,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
        }

        // Token counts from last turn
        if let Some(tokens) = turn.token_counts() {
            if !parts.is_empty() {
                parts.push(Span::raw(" · "));
            }
            let input = format_token_count(tokens.input());
            let output = format_token_count(tokens.output());
            let mut token_text = format!("{input} in / {output} out");
            if let Some(cached) = tokens.cached() {
                if cached > 0 {
                    token_text.push_str(&format!(" / {} cached", format_token_count(cached)));
                }
            }
            parts.push(Span::styled(
                token_text,
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    // Credit usage
    if let Some((used, limit)) = state.credit_usage() {
        if !parts.is_empty() {
            parts.push(Span::raw(" · "));
        }
        parts.push(Span::styled(
            format!("Credits: ${used:.2}/${limit:.2}"),
            Style::default().fg(Color::DarkGray),
        ));
    }

    // In browse mode, prompt the user to return to follow mode with PgDn.
    if state.chat_scroll_back().is_some() {
        if !parts.is_empty() {
            parts.push(Span::raw(" · "));
        }
        parts.push(Span::styled(
            "SCROLL \u{2193} PgDn",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if parts.is_empty() {
        parts.push(Span::styled("cyril", Style::default().fg(Color::DarkGray)));
    }

    let line = Line::from(parts);
    let bar = Paragraph::new(line).style(Style::default().bg(Color::Rgb(30, 30, 46)));

    frame.render_widget(bar, area);
}

fn format_token_count(count: u64) -> String {
    if count < 1000 {
        format!("{count}")
    } else if count < 1_000_000 {
        format!("{:.1}k", count as f64 / 1000.0)
    } else {
        format!("{:.1}M", count as f64 / 1_000_000.0)
    }
}

fn spinner_index(state: &dyn TuiState) -> usize {
    state
        .activity_elapsed()
        .map(|d| {
            (d.as_millis() / palette::SPINNER_FRAME_MS) as usize % palette::SPINNER_CHARS.len()
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn toolbar_renders_no_session() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
        // If we get here, rendering succeeded
    }

    #[test]
    fn toolbar_renders_with_session() {
        let state = MockTuiState {
            session_label: Some("my-session".into()),
            current_mode: Some("code".into()),
            current_model: Some("claude-sonnet-4".into()),
            ..Default::default()
        };

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_renders_context_usage() {
        let state = MockTuiState {
            context_usage: Some(75.0),
            credit_usage: Some((5.25, 10.0)),
            ..Default::default()
        };

        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_shows_scroll_indicator_in_browse_mode() {
        let state = MockTuiState {
            chat_scroll_back: Some(10),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");

        let buffer = terminal.backend().buffer();
        let text: String = (0..80)
            .map(|x| {
                buffer
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        assert!(
            text.contains("SCROLL"),
            "status bar should show SCROLL indicator in browse mode: {text}"
        );
    }

    #[test]
    fn status_bar_shows_token_limit_warning() {
        let state = MockTuiState {
            last_turn: Some(cyril_core::types::TurnSummary::new(
                cyril_core::types::StopReason::MaxTokens,
                None,
                None,
            )),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = (0..80)
            .map(|x| {
                buffer
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        assert!(
            text.contains("Token limit"),
            "should show token limit warning: {text}"
        );
    }

    #[test]
    fn status_bar_shows_token_counts() {
        let state = MockTuiState {
            last_turn: Some(cyril_core::types::TurnSummary::new(
                cyril_core::types::StopReason::EndTurn,
                Some(cyril_core::types::TokenCounts::new(1500, 800, Some(300))),
                None,
            )),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state);
            })
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = (0..80)
            .map(|x| {
                buffer
                    .cell((x, 0))
                    .map(|c| c.symbol().to_string())
                    .unwrap_or_default()
            })
            .collect();
        assert!(text.contains("1.5k in"), "should show input tokens: {text}");
        assert!(
            text.contains("800 out"),
            "should show output tokens: {text}"
        );
    }

    #[test]
    fn format_token_count_formats_correctly() {
        assert_eq!(format_token_count(500), "500");
        assert_eq!(format_token_count(1500), "1.5k");
        assert_eq!(format_token_count(1_200_000), "1.2M");
    }
}
