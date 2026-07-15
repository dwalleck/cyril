use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::palette;
use crate::theme::Theme;
use crate::traits::{Activity, TuiState};

/// Render the toolbar (top line).
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState, theme: &Theme) {
    let mut parts: Vec<Span> = Vec::new();

    // Activity indicator
    match state.activity() {
        Activity::Idle | Activity::Ready => {}
        Activity::Sending | Activity::Waiting => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(theme.emphasis),
            ));
        }
        Activity::Streaming => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(theme.subdued_positive),
            ));
        }
        Activity::ToolRunning => {
            let idx = spinner_index(state);
            parts.push(Span::styled(
                format!("{} ", palette::SPINNER_CHARS[idx]),
                Style::default().fg(theme.accent_quinary),
            ));
        }
    }

    // Session label
    if let Some(label) = state.session_label() {
        parts.push(Span::styled(
            label.to_string(),
            Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
        ));
    } else {
        parts.push(Span::styled(
            "No session",
            Style::default().fg(theme.subdued),
        ));
    }

    // Mode
    if let Some(mode) = state.current_mode() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            mode.to_string(),
            Style::default().fg(theme.accent_quinary),
        ));
    }

    // Model
    if let Some(model) = state.current_model() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            model.to_string(),
            Style::default().fg(theme.accent_quaternary),
        ));
    }

    // Thinking-effort level (only present under thinking models, Kiro 2.5.0+)
    if let Some(effort) = state.effort() {
        parts.push(Span::raw(" "));
        parts.push(Span::styled(
            format!("◇ {effort}"),
            Style::default().fg(theme.emphasis),
        ));
    }

    // Queued steers (K1b) — mid-turn steers awaiting pickup at a tool boundary.
    let steers = state.steering_queued();
    if steers >= 1 {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            format!("⇄ {steers} steer{}", if steers == 1 { "" } else { "s" }),
            Style::default().fg(theme.emphasis),
        ));
    }

    // Code intelligence indicator
    if state.code_intelligence_active() {
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            "✦ code intel",
            Style::default().fg(theme.accent_quinary),
        ));
    }

    // Elapsed time for active operations
    if let Some(elapsed) = state.activity_elapsed() {
        let secs = elapsed.as_secs();
        parts.push(Span::raw(" · "));
        parts.push(Span::styled(
            format!("{secs}s"),
            Style::default().fg(theme.subdued),
        ));
    }

    let line = Line::from(parts);
    let toolbar = Paragraph::new(line).style(Style::default().bg(theme.chrome));

    frame.render_widget(toolbar, area);
}

/// Render the bottom status bar (context usage + credits).
///
/// The line does not wrap, so the KAS breakdown bar (~70 cols) is appended
/// only when the whole line — including every segment after it (stop reason,
/// tokens, credits, SCROLL hint) — fits within `area` (cyril-mdbp). When it
/// does not fit, the bar is omitted entirely; the "Context: N%" scalar and
/// the trailing affordances always stay.
pub fn render_status_bar(frame: &mut Frame, area: Rect, state: &dyn TuiState, theme: &Theme) {
    let full = Line::from(status_bar_spans(state, theme, true));
    let line = if full.width() <= usize::from(area.width) {
        full
    } else {
        Line::from(status_bar_spans(state, theme, false))
    };
    let bar = Paragraph::new(line).style(Style::default().bg(theme.chrome));

    frame.render_widget(bar, area);
}

/// Build the status-bar spans, with or without the KAS breakdown bar.
fn status_bar_spans(
    state: &dyn TuiState,
    theme: &Theme,
    include_breakdown: bool,
) -> Vec<Span<'static>> {
    let mut parts: Vec<Span> = Vec::new();

    // Context usage gauge
    if let Some(pct) = state.context_usage() {
        let color = if pct > 90.0 {
            theme.subdued_negative
        } else if pct > 70.0 {
            theme.emphasis
        } else {
            theme.subdued_positive
        };
        parts.push(Span::styled(
            format!("Context: {pct:.0}%"),
            Style::default().fg(color),
        ));
    }

    // KAS context breakdown bar (KAS-2b, cyril-5et2): one labeled category per
    // wire bucket, aggregate-only — no per-item drill-in (cyril-1116). Fixed
    // five buckets → O(1).
    if include_breakdown && let Some(bd) = state.context_breakdown() {
        if !parts.is_empty() {
            parts.push(Span::raw(" · "));
        }
        let cats = [
            ("Context Files", bd.context_files().percent()),
            ("Session Files", bd.session_files().percent()),
            ("Tools", bd.tools().percent()),
            ("Prompts", bd.your_prompts().percent()),
            ("Responses", bd.kiro_responses().percent()),
        ];
        for (i, (label, pct)) in cats.iter().enumerate() {
            if i > 0 {
                parts.push(Span::raw("  "));
            }
            parts.push(Span::styled(
                format!("{label} {pct:.0}%"),
                Style::default().fg(theme.subdued),
            ));
        }
    }

    // Stop reason warning (when last turn didn't end normally)
    if let Some(turn) = state.last_turn() {
        use cyril_core::types::StopReason;
        let (label, color) = match turn.stop_reason() {
            // EndTurn's empty label is never pushed, so its color is inert
            // (pinned by the dij8 probe: no White cell in any status scene).
            StopReason::EndTurn => ("", theme.text),
            StopReason::MaxTokens => ("Token limit", theme.emphasis),
            StopReason::MaxTurnRequests => ("Turn limit", theme.emphasis),
            StopReason::Refusal => ("Refused", theme.subdued_negative),
            StopReason::Cancelled => ("Cancelled", theme.subdued),
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
            if let Some(cached) = tokens.cached()
                && cached > 0
            {
                token_text.push_str(&format!(" / {} cached", format_token_count(cached)));
            }
            parts.push(Span::styled(token_text, Style::default().fg(theme.subdued)));
        }
    }

    // Credit usage
    if let Some((used, limit)) = state.credit_usage() {
        if !parts.is_empty() {
            parts.push(Span::raw(" · "));
        }
        parts.push(Span::styled(
            format!("Credits: ${used:.2}/${limit:.2}"),
            Style::default().fg(theme.subdued),
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
                .fg(theme.emphasis)
                .add_modifier(Modifier::BOLD),
        ));
    }

    if parts.is_empty() {
        parts.push(Span::styled("cyril", Style::default().fg(theme.subdued)));
    }

    parts
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn cyril_dark() -> Theme {
        crate::theme::resolve(
            crate::theme::ThemeId::CyrilDark,
            crate::theme::ColorMode::TrueColor,
        )
    }

    #[test]
    fn toolbar_renders_no_session() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state, &cyril_dark());
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
                render(frame, frame.area(), &state, &cyril_dark());
            })
            .expect("draw");
    }

    #[test]
    fn toolbar_renders_effort_when_present() {
        let state = MockTuiState {
            current_model: Some("claude-opus-4.8".into()),
            effort: Some(cyril_core::types::EffortLevel::High),
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &cyril_dark()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        let text: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(text.contains("high"), "effort should render, got: {text:?}");
    }

    #[test]
    fn toolbar_omits_effort_when_absent() {
        let state = MockTuiState {
            current_model: Some("claude-haiku-4.5".into()),
            effort: None,
            ..Default::default()
        };
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), &state, &cyril_dark()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        let text: String = (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            !text.contains("◇"),
            "no effort badge when absent, got: {text:?}"
        );
    }

    // cyril-bm1j Slice 8 / claim C8: toolbar chip iff steering_queued() >= 1.
    fn toolbar_text(state: &MockTuiState) -> String {
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render(frame, frame.area(), state, &cyril_dark()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        (0..80)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn renders_steer_chip_when_queued() {
        // 0 -> no chip.
        let none = toolbar_text(&MockTuiState {
            steering_queued: 0,
            ..Default::default()
        });
        assert!(
            !none.contains("steer") && !none.contains("⇄"),
            "no chip at 0: {none:?}"
        );

        // 2 -> chip shows the count.
        let two = toolbar_text(&MockTuiState {
            session_label: Some("s".into()),
            steering_queued: 2,
            ..Default::default()
        });
        assert!(
            two.contains('2') && two.contains("steer"),
            "chip at 2: {two:?}"
        );

        // 1 -> count present.
        let one = toolbar_text(&MockTuiState {
            session_label: Some("s".into()),
            steering_queued: 1,
            ..Default::default()
        });
        assert!(
            one.contains('1') && one.contains("steer"),
            "chip at 1: {one:?}"
        );

        // Adversarial: large count on an 80-wide terminal renders without panic.
        let _ = toolbar_text(&MockTuiState {
            session_label: Some("s".into()),
            steering_queued: 999,
            ..Default::default()
        });
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
                render_status_bar(frame, frame.area(), &state, &cyril_dark());
            })
            .expect("draw");
    }

    #[test]
    fn status_bar_renders_breakdown_bar() {
        // Slice 4 / claim C7. Five DISTINCT percents so a label<->value
        // transposition surfaces (the real frame's three 0%s would hide it). Each
        // of the five labels must appear paired with its own percent; the type
        // carries no items, so nothing itemized can render.
        use cyril_core::types::{ContextBreakdown, ContextBucket};
        let bd = ContextBreakdown::new(
            ContextBucket::new(1, 11.0),
            ContextBucket::new(2, 22.0),
            ContextBucket::new(3, 33.0),
            ContextBucket::new(4, 44.0),
            ContextBucket::new(5, 55.0),
        );
        let state = MockTuiState {
            context_breakdown: Some(bd),
            ..Default::default()
        };
        let backend = TestBackend::new(120, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_status_bar(frame, frame.area(), &state, &cyril_dark()))
            .expect("draw");
        let buffer = terminal.backend().buffer();
        let text: String = (0..buffer.area.width)
            .map(|x| buffer[(x, 0)].symbol())
            .collect();
        for expect in [
            "Context Files 11%",
            "Session Files 22%",
            "Tools 33%",
            "Prompts 44%",
            "Responses 55%",
        ] {
            assert!(
                text.contains(expect),
                "status bar missing {expect:?}; got: {text:?}"
            );
        }
    }

    #[test]
    fn status_bar_renders_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render_status_bar(frame, frame.area(), &state, &cyril_dark());
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
                render_status_bar(frame, frame.area(), &state, &cyril_dark());
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
                render_status_bar(frame, frame.area(), &state, &cyril_dark());
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
                render_status_bar(frame, frame.area(), &state, &cyril_dark());
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

    // cyril-mdbp: the KAS breakdown bar must never clip trailing affordances
    // (credits, SCROLL hint) off a narrow status line.
    fn status_bar_text(state: &MockTuiState, width: u16) -> String {
        let backend = TestBackend::new(width, 1);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| render_status_bar(frame, frame.area(), state, &cyril_dark()))
            .expect("draw");
        let buf = terminal.backend().buffer();
        (0..width)
            .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    fn breakdown_browse_state() -> MockTuiState {
        use cyril_core::types::{ContextBreakdown, ContextBucket};
        MockTuiState {
            context_usage: Some(75.0),
            context_breakdown: Some(ContextBreakdown::new(
                ContextBucket::new(1, 11.0),
                ContextBucket::new(2, 22.0),
                ContextBucket::new(3, 33.0),
                ContextBucket::new(4, 44.0),
                ContextBucket::new(5, 55.0),
            )),
            credit_usage: Some((5.25, 10.0)),
            chat_scroll_back: Some(10),
            ..Default::default()
        }
    }

    #[test]
    fn narrow_status_bar_keeps_scroll_hint_over_breakdown() {
        let text = status_bar_text(&breakdown_browse_state(), 80);
        assert!(
            text.contains("SCROLL"),
            "scroll hint must survive on a narrow terminal: {text:?}"
        );
        assert!(
            text.contains("Credits: $5.25/$10.00"),
            "credits must survive on a narrow terminal: {text:?}"
        );
        assert!(
            !text.contains("Context Files"),
            "breakdown bar must be omitted when it would clip trailing segments: {text:?}"
        );
        assert!(
            text.contains("Context: 75%"),
            "the scalar context gauge always stays: {text:?}"
        );
    }

    #[test]
    fn wide_status_bar_keeps_breakdown_bar() {
        let text = status_bar_text(&breakdown_browse_state(), 200);
        for expect in [
            "Context: 75%",
            "Context Files 11%",
            "Session Files 22%",
            "Tools 33%",
            "Prompts 44%",
            "Responses 55%",
            "Credits: $5.25/$10.00",
            "SCROLL",
        ] {
            assert!(
                text.contains(expect),
                "wide status bar missing {expect:?}: {text:?}"
            );
        }
    }

    #[test]
    fn format_token_count_formats_correctly() {
        assert_eq!(format_token_count(500), "500");
        assert_eq!(format_token_count(1500), "1.5k");
        assert_eq!(format_token_count(1_200_000), "1.2M");
    }
}
