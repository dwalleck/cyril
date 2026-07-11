use ratatui::Frame;
use ratatui::layout::{Constraint, Layout};
use ratatui::widgets::Paragraph;

use crate::traits::TuiState;

/// Draw the full TUI frame. Panic-safe wrapper with fallback rendering.
pub fn draw(frame: &mut Frame, state: &dyn TuiState) {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        draw_inner(frame, state);
    }));
    if result.is_err() {
        draw_fallback(frame);
    }
}

fn draw_inner(frame: &mut Frame, state: &dyn TuiState) {
    let area = frame.area();
    let theme = state.theme();

    // Runtime-variable panel heights are owned by their widget's height_for().
    let crew_height = crate::widgets::crew_panel::height_for(state);
    let voice_height = crate::widgets::voice::height_for(state);
    let suggestions_height = crate::widgets::suggestions::height_for(state);
    let input_height = crate::widgets::input::height_for(state);

    let [
        toolbar_area,
        chat_area,
        crew_area,
        voice_area,
        input_area,
        suggestions_area,
        status_area,
    ] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(5),
        Constraint::Length(crew_height),
        Constraint::Length(voice_height),
        Constraint::Length(input_height),
        Constraint::Length(suggestions_height),
        Constraint::Length(1),
    ])
    .areas(area);

    crate::widgets::toolbar::render(frame, toolbar_area, state);
    crate::widgets::chat::render(frame, chat_area, state, &theme);
    if crew_height > 0 {
        crate::widgets::crew_panel::render(frame, crew_area, state);
    }
    if voice_height > 0 {
        crate::widgets::voice::render(frame, voice_area, state);
    }
    crate::widgets::input::render(frame, input_area, state);
    if suggestions_height > 0 {
        crate::widgets::suggestions::render(frame, suggestions_area, state);
    }
    crate::widgets::toolbar::render_status_bar(frame, status_area, state);

    // Overlays (rendered on top)
    if let Some(approval) = state.approval() {
        crate::widgets::approval::render(frame, area, approval);
    }
    if let Some(picker) = state.picker() {
        crate::widgets::picker::render(frame, area, picker);
    }
    if let Some(hooks) = state.hooks_panel() {
        crate::widgets::hooks_panel::render(frame, area, hooks);
    }
    if let Some(code_panel) = state.code_panel() {
        crate::widgets::code_panel::render(frame, area, code_panel);
    }
}

fn draw_fallback(frame: &mut Frame) {
    let text = Paragraph::new("Render error — press Ctrl+C to quit");
    frame.render_widget(text, frame.area());
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::traits::test_support::MockTuiState;
    use crate::traits::{Activity, ChatMessage, ChatMessageKind, SteerEchoStatus};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;

    fn render_buffer(state: &MockTuiState) -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        terminal.draw(|frame| super::draw(frame, state))?;
        Ok(terminal.backend().buffer().clone())
    }

    fn baseline_steer(text: &str, status: SteerEchoStatus) -> ChatMessage {
        ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: text.to_string(),
                status,
            },
            timestamp: std::time::Instant::now(),
        }
    }

    fn baseline_message_state() -> MockTuiState {
        MockTuiState {
            theme: crate::theme::resolve(
                crate::theme::ThemeId::CyrilDark,
                crate::theme::ColorMode::TrueColor,
            ),
            messages: vec![
                ChatMessage::user_text("user".into()),
                ChatMessage::agent_text(String::new()),
                ChatMessage::thought("thought".into()),
                ChatMessage::plan(cyril_core::types::Plan::new(Vec::new())),
                ChatMessage::system("system".into()),
                ChatMessage::command_output("context".into(), String::new()),
                baseline_steer("queued", SteerEchoStatus::Queued),
                baseline_steer("applied", SteerEchoStatus::Applied),
                baseline_steer("cleared", SteerEchoStatus::Cleared),
                baseline_steer("unsupported", SteerEchoStatus::Unsupported),
            ],
            activity: Activity::ToolRunning,
            activity_elapsed: Some(Duration::from_secs(1)),
            ..MockTuiState::default()
        }
    }

    fn render_baseline_message_buffer() -> anyhow::Result<Buffer> {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend)?;
        let state = baseline_message_state();
        terminal.draw(|frame| {
            crate::widgets::chat::render(frame, frame.area(), &state, &state.theme);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn symbol_hex(symbol: &str) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";

        let mut encoded = String::with_capacity(symbol.len() * 2);
        for byte in symbol.as_bytes() {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        encoded
    }

    fn picker_state() -> MockTuiState {
        use crate::traits::PickerState;
        use cyril_core::types::CommandOption;

        let option =
            |label: &str, value: &str, description: &str, group: &str, is_current: bool| {
                CommandOption {
                    label: label.into(),
                    value: value.into(),
                    description: Some(description.into()),
                    group: Some(group.into()),
                    is_current,
                }
            };

        MockTuiState {
            picker: Some(PickerState {
                title: "Select model".into(),
                options: vec![
                    option(
                        "Claude Sonnet",
                        "claude-sonnet",
                        "Balanced speed and reasoning",
                        "Anthropic",
                        true,
                    ),
                    option(
                        "Claude Opus",
                        "claude-opus",
                        "Deep reasoning for complex changes",
                        "Anthropic",
                        false,
                    ),
                    option(
                        "GPT-5",
                        "gpt-5",
                        "General-purpose coding model",
                        "OpenAI",
                        false,
                    ),
                    option(
                        "GPT-5 Mini",
                        "gpt-5-mini",
                        "Fast, economical edits",
                        "OpenAI",
                        false,
                    ),
                ],
                filter: String::new(),
                filtered_indices: vec![0, 1, 2, 3],
                selected: 1,
            }),
            session_label: Some("theme-contract".into()),
            ..MockTuiState::default()
        }
    }

    fn tool_diff_state() -> MockTuiState {
        use crate::traits::{Activity, ChatMessage, TrackedToolCall};
        use cyril_core::types::{
            ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
        };

        let tool_call = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("theme-seam-diff"),
                "Editing src/greeting.rs".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "src/greeting.rs".into(),
                old_text: Some(
                    "fn greet() {\n    println!(\"Hello, 世界\");\n    let status = \"old\";\n}\n"
                        .into(),
                ),
                new_text: "fn greet() {\n    println!(\"Hello, Cyril 🚀\");\n    let status = \"ready\";\n}\n"
                    .into(),
            }])
            .with_locations(vec![ToolCallLocation {
                path: "src/greeting.rs".into(),
                line: Some(1),
            }]),
        );

        MockTuiState {
            theme: crate::theme::resolve(
                crate::theme::ThemeId::CyrilDark,
                crate::theme::ColorMode::TrueColor,
            ),
            messages: vec![
                ChatMessage::user_text("Update the greeting without losing Unicode.".into()),
                ChatMessage::agent_text("I updated the Rust greeting and status.".into()),
                ChatMessage::tool_call(tool_call),
            ],
            activity: Activity::Ready,
            session_label: Some("theme-contract".into()),
            current_mode: Some("code".into()),
            current_model: Some("claude-sonnet".into()),
            ..MockTuiState::default()
        }
    }

    #[test]
    fn draw_fallback_does_not_panic() {
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                super::draw_fallback(frame);
            })
            .expect("draw should succeed");
    }

    #[test]
    fn draw_with_mock_state_does_not_panic() {
        let state = MockTuiState::default();
        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).expect("test terminal");
        terminal
            .draw(|frame| {
                super::draw(frame, &state);
            })
            .expect("draw should succeed");
    }

    #[test]
    fn conversation_message_shape_matches_pinned_baseline() -> anyhow::Result<()> {
        let expected = include_str!("fixtures/conversation-theme-baseline.tsv")
            .lines()
            .skip(2)
            .filter_map(|line| {
                let fields: Vec<_> = line.split('\t').collect();
                (fields.first() == Some(&"messages")).then_some(fields)
            })
            .map(|fields| {
                Ok((
                    fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing symbol field"))?
                        .to_string(),
                    fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing modifier field"))?
                        .parse::<u16>()?,
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let buffer = render_baseline_message_buffer()?;
        let actual = buffer
            .content()
            .iter()
            .map(|cell| (symbol_hex(cell.symbol()), cell.modifier.bits()))
            .collect::<Vec<_>>();

        assert_eq!(actual.len(), 1_920);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[test]
    fn conversation_frame_uses_state_theme_once() -> anyhow::Result<()> {
        use crate::traits::ChatMessage;

        let state = MockTuiState {
            messages: vec![ChatMessage::user_text("marker".into())],
            ..Default::default()
        };
        let buffer = render_buffer(&state)?;
        let user_label = buffer
            .cell((0, 1))
            .ok_or_else(|| anyhow::anyhow!("missing first chat cell"))?;
        assert_eq!(user_label.symbol(), "Y");
        assert_eq!(user_label.fg, state.theme.user);

        let (production, _) = include_str!("render.rs")
            .split_once("#[cfg(test)]")
            .ok_or_else(|| anyhow::anyhow!("missing test module boundary"))?;
        assert_eq!(production.matches("state.theme()").count(), 1);
        Ok(())
    }

    #[test]
    fn theme_seam_idle() -> anyhow::Result<()> {
        let buffer = render_buffer(&MockTuiState::default())?;
        insta::assert_debug_snapshot!("theme_seam_idle", buffer);
        Ok(())
    }

    #[test]
    fn theme_seam_tool_diff() -> anyhow::Result<()> {
        let buffer = render_buffer(&tool_diff_state())?;
        insta::assert_debug_snapshot!("theme_seam_tool_diff", buffer);
        Ok(())
    }

    #[test]
    fn theme_seam_picker() -> anyhow::Result<()> {
        let buffer = render_buffer(&picker_state())?;
        insta::assert_debug_snapshot!("theme_seam_picker", buffer);
        Ok(())
    }
}
