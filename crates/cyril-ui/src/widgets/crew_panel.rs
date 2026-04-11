use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use cyril_core::types::SubagentStatus;

use crate::traits::TuiState;

/// Maximum number of row lines visible in the crew panel (excluding borders).
/// When the total number of subagents + pending stages exceeds this, the panel
/// shows `MAX_CREW_ROWS - 1` entries plus a "+N more" overflow indicator.
pub const MAX_CREW_ROWS: u16 = 6;

/// Border overhead (top + bottom) for the crew panel's bordered block.
const BORDER_LINES: u16 = 2;

/// Compute the total panel height (including borders) needed for the current
/// subagent + pending stage count. Returns 0 when the panel should not be shown.
///
/// This is the single source of truth for crew panel sizing — both `render.rs`
/// (for layout constraints) and `render()` below call it.
pub fn height_for(state: &dyn TuiState) -> u16 {
    let tracker = state.subagent_tracker();
    let total = tracker.subagents().len() + tracker.pending_stages().len();
    if total == 0 {
        0
    } else {
        let row_count = (total as u16).min(MAX_CREW_ROWS);
        row_count + BORDER_LINES
    }
}

/// Render the crew panel (subagent status bar).
/// Renders nothing if there are no subagents and no pending stages.
/// Returns the number of lines rendered (0 if nothing was drawn).
pub fn render(frame: &mut Frame, area: Rect, state: &dyn TuiState) -> u16 {
    let tracker = state.subagent_tracker();
    let subagents = tracker.subagents();
    let pending = tracker.pending_stages();

    if subagents.is_empty() && pending.is_empty() {
        return 0;
    }

    // Sort subagents deterministically by session_name for stable display order.
    let mut sorted_subagents: Vec<_> = subagents.values().collect();
    sorted_subagents.sort_by(|a, b| a.session_name().cmp(b.session_name()));

    // Sort pending stages deterministically by name.
    let mut sorted_pending: Vec<_> = pending.iter().collect();
    sorted_pending.sort_by(|a, b| a.name().cmp(b.name()));

    let total_rows = subagents.len() + pending.len();
    let overflow = total_rows > MAX_CREW_ROWS as usize;
    // When overflowing, reserve one line for the "+N more" indicator.
    let visible_capacity = if overflow {
        (MAX_CREW_ROWS - 1) as usize
    } else {
        MAX_CREW_ROWS as usize
    };

    let mut lines: Vec<Line> = Vec::new();
    let mut emitted = 0;

    // Group header — show crew group name if all subagents share one
    let groups = tracker.groups();
    let header = match groups.as_slice() {
        [] => "subagents".to_string(),
        [only] => format!("crew: {only}"),
        many => format!("{} crews", many.len()),
    };

    // One line per subagent (up to visible_capacity)
    for info in &sorted_subagents {
        if emitted >= visible_capacity {
            break;
        }
        let (icon, icon_color, status_text) = match info.status() {
            SubagentStatus::Working { message } => (
                "●",
                Color::Green,
                message.as_deref().unwrap_or("Working").to_string(),
            ),
            SubagentStatus::Terminated => ("◆", Color::DarkGray, "Terminated".to_string()),
        };

        lines.push(Line::from(vec![
            Span::styled(format!("{icon} "), Style::default().fg(icon_color)),
            Span::styled(
                format!("{:<20} ", info.session_name()),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(status_text, Style::default().fg(Color::DarkGray)),
        ]));
        emitted += 1;
    }

    // One line per pending stage (up to remaining visible_capacity)
    for stage in &sorted_pending {
        if emitted >= visible_capacity {
            break;
        }
        let deps = if stage.depends_on().is_empty() {
            "Waiting".to_string()
        } else {
            format!("Waiting (depends: {})", stage.depends_on().join(", "))
        };
        lines.push(Line::from(vec![
            Span::styled("○ ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:<20} ", stage.name()),
                Style::default().fg(Color::Gray),
            ),
            Span::styled(deps, Style::default().fg(Color::DarkGray)),
        ]));
        emitted += 1;
    }

    // Overflow indicator — show how many entries are hidden.
    if overflow {
        let hidden = total_rows - emitted;
        lines.push(Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(
                format!("+{hidden} more"),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    }

    let panel_height = lines.len() as u16 + BORDER_LINES;

    // Clamp to available area
    let actual_height = panel_height.min(area.height);
    let panel_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: actual_height,
    };

    let block = Block::default().borders(Borders::ALL).title(Span::styled(
        format!(" {header} "),
        Style::default().fg(Color::Cyan),
    ));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, panel_area);

    actual_height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::test_support::MockTuiState;
    use cyril_core::types::{PendingStage, SessionId, SubagentInfo, SubagentStatus};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_working(id: &str, name: &str, group: Option<&str>) -> SubagentInfo {
        SubagentInfo::new(
            SessionId::new(id),
            name,
            name,
            "query",
            SubagentStatus::Working {
                message: Some("Running".into()),
            },
            group.map(String::from),
            None,
            vec![],
        )
    }

    #[test]
    fn render_returns_zero_when_empty() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut height = 0;
        terminal
            .draw(|frame| {
                height = render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");
        assert_eq!(height, 0);
    }

    #[test]
    fn render_shows_single_subagent() {
        let mut state = MockTuiState::default();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents: vec![make_working("s1", "reviewer", Some("crew-a"))],
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut height = 0;
        terminal
            .draw(|frame| {
                height = render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");
        // 1 subagent line + 2 border lines = 3
        assert_eq!(height, 3);
    }

    #[test]
    fn render_shows_pending_stage_with_dependencies() {
        let mut state = MockTuiState::default();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents: vec![make_working("s1", "reviewer", Some("crew-a"))],
            pending_stages: vec![PendingStage::new(
                "summary",
                None,
                Some("crew-a".into()),
                None,
                vec!["reviewer".into()],
            )],
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut height = 0;
        terminal
            .draw(|frame| {
                height = render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");
        // 1 subagent + 1 pending + 2 borders = 4
        assert_eq!(height, 4);
    }

    #[test]
    fn render_multiple_subagents_deterministic_order() {
        let mut state = MockTuiState::default();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents: vec![
                make_working("s2", "zebra", Some("crew-a")),
                make_working("s1", "alpha", Some("crew-a")),
            ],
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 10);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");

        let buffer = terminal.backend().buffer();
        let text: String = (0..10)
            .flat_map(|y| {
                (0..80).map(move |x| {
                    buffer[(x as u16, y as u16)]
                        .symbol()
                        .chars()
                        .next()
                        .unwrap_or(' ')
                })
            })
            .collect();

        let alpha_pos = text.find("alpha").expect("alpha should be rendered");
        let zebra_pos = text.find("zebra").expect("zebra should be rendered");
        assert!(alpha_pos < zebra_pos, "alpha should come before zebra");
    }

    fn buffer_text(terminal: &Terminal<TestBackend>, w: u16, h: u16) -> String {
        let buffer = terminal.backend().buffer();
        (0..h)
            .flat_map(|y| {
                (0..w).map(move |x| buffer[(x, y)].symbol().chars().next().unwrap_or(' '))
            })
            .collect()
    }

    #[test]
    fn height_for_returns_zero_when_no_subagents() {
        let state = MockTuiState::default();
        assert_eq!(height_for(&state), 0);
    }

    #[test]
    fn height_for_accounts_for_borders() {
        let mut state = MockTuiState::default();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents: vec![make_working("s1", "reviewer", Some("crew-a"))],
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);
        // 1 row + 2 borders = 3
        assert_eq!(height_for(&state), 3);
    }

    #[test]
    fn height_for_clamps_to_max() {
        let mut state = MockTuiState::default();
        // 10 subagents → should clamp to MAX_CREW_ROWS + 2 = 8
        let subagents: Vec<SubagentInfo> = (0..10)
            .map(|i| make_working(&format!("s{i}"), &format!("agent-{i:02}"), Some("crew-a")))
            .collect();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents,
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);
        assert_eq!(height_for(&state), MAX_CREW_ROWS + BORDER_LINES);
    }

    #[test]
    fn render_shows_overflow_indicator_when_too_many_rows() {
        let mut state = MockTuiState::default();
        // 10 subagents — should show MAX_CREW_ROWS - 1 (5) rows + "+5 more"
        let subagents: Vec<SubagentInfo> = (0..10)
            .map(|i| make_working(&format!("s{i}"), &format!("agent-{i:02}"), Some("crew-a")))
            .collect();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents,
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        let mut height = 0;
        terminal
            .draw(|frame| {
                height = render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");

        // Total height: MAX_CREW_ROWS (5 visible rows + 1 overflow indicator) + 2 borders = 8
        assert_eq!(height, MAX_CREW_ROWS + BORDER_LINES);

        let text = buffer_text(&terminal, 80, 12);
        // 10 total - 5 visible = 5 hidden
        assert!(
            text.contains("+5 more"),
            "expected '+5 more' overflow indicator, got buffer:\n{text}"
        );
        // First 5 sorted agents should be visible
        assert!(text.contains("agent-00"));
        assert!(text.contains("agent-04"));
        // Agents 5-9 should NOT be visible
        assert!(!text.contains("agent-09"));
    }

    #[test]
    fn render_exactly_at_max_shows_all_rows_no_overflow() {
        let mut state = MockTuiState::default();
        // MAX_CREW_ROWS (6) subagents — no overflow, all should render
        let subagents: Vec<SubagentInfo> = (0..MAX_CREW_ROWS as usize)
            .map(|i| make_working(&format!("s{i}"), &format!("agent-{i:02}"), Some("crew-a")))
            .collect();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents,
            pending_stages: vec![],
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");

        let text = buffer_text(&terminal, 80, 12);
        // No overflow indicator
        assert!(
            !text.contains("more"),
            "unexpected overflow indicator, got buffer:\n{text}"
        );
        // All 6 agents visible
        for i in 0..MAX_CREW_ROWS {
            assert!(
                text.contains(&format!("agent-{i:02}")),
                "agent-{i:02} should be visible"
            );
        }
    }

    #[test]
    fn render_overflow_counts_include_pending_stages() {
        let mut state = MockTuiState::default();
        // 3 subagents + 5 pending stages = 8 total rows, over MAX_CREW_ROWS=6
        let subagents: Vec<SubagentInfo> = (0..3)
            .map(|i| make_working(&format!("s{i}"), &format!("agent-{i:02}"), Some("crew-a")))
            .collect();
        let pending: Vec<PendingStage> = (0..5)
            .map(|i| {
                PendingStage::new(
                    format!("stage-{i:02}"),
                    None,
                    Some("crew-a".into()),
                    None,
                    vec![],
                )
            })
            .collect();
        let notif = cyril_core::types::Notification::SubagentListUpdated {
            subagents,
            pending_stages: pending,
        };
        state.subagent_tracker.apply_notification(&notif);

        let backend = TestBackend::new(80, 12);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                render(frame, frame.area(), &state);
            })
            .expect("draw should succeed");

        let text = buffer_text(&terminal, 80, 12);
        // 8 total - 5 visible = 3 hidden
        assert!(
            text.contains("+3 more"),
            "expected '+3 more' overflow indicator, got buffer:\n{text}"
        );
    }
}
