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

/// Rows the chat viewport keeps under height pressure (cyril-a14l C1).
/// With surplus space `Min(CHAT_FLOOR)` grows exactly like the previous
/// `Min(5)` did, so roomy frames are unchanged (pinned by the slice-0
/// snapshots in `floor_tests`).
const CHAT_FLOOR: u16 = 3;
/// Minimum input box height under pressure: one content row plus borders.
const INPUT_FLOOR: u16 = 3;

fn draw_inner(frame: &mut Frame, state: &dyn TuiState) {
    let area = frame.area();
    let theme = state.theme();

    // Runtime-variable panel heights are owned by their widget's height_for().
    let crew_height = crate::widgets::crew_panel::height_for(state);
    let voice_height = crate::widgets::voice::height_for(state);
    let suggestions_height = crate::widgets::suggestions::height_for(state);
    let input_demand = crate::widgets::input::height_for(state);

    // Explicit vertical budget (cyril-a14l R1): the input may grow with its
    // draft only until chat would drop below its floor — its allocation is
    // decided here, not by the constraint solver, so the widget's
    // cursor-follow window always sees its real height.
    let avail = area
        .height
        .saturating_sub(2)
        .saturating_sub(crew_height)
        .saturating_sub(voice_height);
    let input_height = input_demand
        .min(avail.saturating_sub(CHAT_FLOOR))
        .max(INPUT_FLOOR.min(avail));
    if input_height < input_demand {
        tracing::trace!(
            input_demand,
            input_height,
            frame_height = area.height,
            "input height clamped by the vertical budget"
        );
    }

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
        Constraint::Min(CHAT_FLOOR),
        Constraint::Length(crew_height),
        Constraint::Length(voice_height),
        Constraint::Length(input_height),
        Constraint::Length(suggestions_height),
        Constraint::Length(1),
    ])
    .areas(area);

    crate::widgets::toolbar::render(frame, toolbar_area, state, &theme);
    crate::widgets::chat::render(frame, chat_area, state, &theme);
    if crew_height > 0 {
        crate::widgets::crew_panel::render(frame, crew_area, state, &theme);
    }
    if voice_height > 0 {
        crate::widgets::voice::render(frame, voice_area, state, &theme);
    }
    crate::widgets::input::render(frame, input_area, state, &theme);
    if suggestions_height > 0 {
        crate::widgets::suggestions::render(frame, suggestions_area, state, &theme);
    }
    crate::widgets::toolbar::render_status_bar(frame, status_area, state, &theme);

    // Overlays (rendered on top)
    if let Some(approval) = state.approval() {
        crate::widgets::approval::render(frame, area, input_area.y, approval, &theme);
    }
    if let Some(picker) = state.picker() {
        crate::widgets::picker::render(frame, area, input_area.y, picker, &theme);
    }
    if let Some(hooks) = state.hooks_panel() {
        crate::widgets::hooks_panel::render(frame, area, input_area.y, hooks, &theme);
    }
    if let Some(code_panel) = state.code_panel() {
        crate::widgets::code_panel::render(frame, area, input_area.y, code_panel, &theme);
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
                message_id: None,
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
            theme: crate::theme::resolve(
                crate::theme::ThemeId::CyrilDark,
                crate::theme::ColorMode::TrueColor,
            ),
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
        let expected = include_str!("../tests/fixtures/conversation-theme-baseline.tsv")
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
        let state = MockTuiState {
            theme: crate::theme::resolve(
                crate::theme::ThemeId::CyrilDark,
                crate::theme::ColorMode::TrueColor,
            ),
            ..MockTuiState::default()
        };
        let buffer = render_buffer(&state)?;
        insta::assert_debug_snapshot!("theme_seam_idle", buffer);
        Ok(())
    }

    #[test]
    fn theme_seam_tool_diff() -> anyhow::Result<()> {
        let buffer = render_buffer(&tool_diff_state())?;
        insta::assert_debug_snapshot!("theme_seam_tool_diff", buffer);
        Ok(())
    }

    /// cyril-dij8 C5: chrome surfaces in a full frame render from the
    /// state's ONE resolved theme — under the marker theme the toolbar and
    /// status-bar backgrounds carry marker `chrome` (Indexed 2) and the crew
    /// working icon carries marker `subdued_positive` (Indexed 25); an
    /// internal CyrilDark resolve inside a widget would surface Rgb values.
    /// (The single-resolve source count is pinned by
    /// `conversation_frame_uses_state_theme_once`.)
    #[test]
    fn chrome_frame_uses_state_theme() -> anyhow::Result<()> {
        use cyril_core::types::{Notification, SessionId, SubagentInfo, SubagentStatus};

        let mut state = MockTuiState::default();
        state
            .subagent_tracker
            .apply_notification(&Notification::SubagentListUpdated {
                subagents: vec![SubagentInfo::new(
                    SessionId::new("s0"),
                    "writer",
                    "writer",
                    "q",
                    SubagentStatus::Working { message: None },
                )],
                pending_stages: vec![],
            });
        let buffer = render_buffer(&state)?;

        let toolbar = buffer
            .cell((79, 0))
            .ok_or_else(|| anyhow::anyhow!("missing toolbar cell"))?;
        assert_eq!(toolbar.bg, ratatui::style::Color::Indexed(2));
        let status = buffer
            .cell((79, 23))
            .ok_or_else(|| anyhow::anyhow!("missing status cell"))?;
        assert_eq!(status.bg, ratatui::style::Color::Indexed(2));

        let mut crew_icon = None;
        for y in 0..24 {
            for x in 0..80 {
                if let Some(cell) = buffer.cell((x, y))
                    && cell.symbol() == "●"
                {
                    crew_icon = Some(cell);
                }
            }
        }
        let crew_icon = crew_icon.ok_or_else(|| anyhow::anyhow!("crew icon not rendered"))?;
        assert_eq!(crew_icon.fg, ratatui::style::Color::Indexed(25));
        Ok(())
    }

    /// cyril-nrnq C6: each modal overlay in a full frame renders from the
    /// state's ONE resolved theme — under the marker theme every modal cell
    /// carries marker Indexed values; any Rgb cell would betray an internal
    /// CyrilDark resolve inside a widget.
    #[test]
    fn modal_frame_uses_state_theme() -> anyhow::Result<()> {
        use ratatui::style::Color;

        use crate::traits::{ApprovalPhase, ApprovalState, HooksPanelState};
        use cyril_core::types::{
            CodePanelData, LspStatus, PermissionOption, PermissionOptionId, PermissionOptionKind,
            ToolCall, ToolCallId, ToolCallStatus, ToolKind,
        };

        let approval = ApprovalState {
            tool_call: ToolCall::new(
                ToolCallId::new("tc"),
                "cmd".into(),
                ToolKind::Execute,
                ToolCallStatus::Pending,
                None,
            ),
            message: "Allow?".into(),
            options: vec![PermissionOption {
                id: PermissionOptionId::new("a"),
                label: "Allow".into(),
                kind: PermissionOptionKind::AllowOnce,
                is_destructive: false,
            }],
            trust_options: vec![],
            selected: 0,
            phase: ApprovalPhase::SelectOption,
            responder: tokio::sync::oneshot::channel().0,
        };
        let hooks = HooksPanelState {
            hooks: vec![cyril_core::types::HookInfo {
                trigger: "t".into(),
                command: "c".into(),
                matcher: Some("m".into()),
            }],
            scroll_offset: 0,
        };
        let code = CodePanelData {
            status: LspStatus::Initialized,
            message: None,
            warning: None,
            root_path: None,
            detected_languages: vec![],
            project_markers: vec![],
            config_path: None,
            doc_url: None,
            lsps: vec![],
        };

        let approval_state = MockTuiState {
            approval: Some(approval),
            ..MockTuiState::default()
        };
        let hooks_state = MockTuiState {
            hooks_panel: Some(hooks),
            ..MockTuiState::default()
        };
        let code_state = MockTuiState {
            code_panel: Some(code),
            ..MockTuiState::default()
        };
        let picker_state = MockTuiState {
            picker: picker_state().picker,
            ..MockTuiState::default()
        };

        // Chrome widgets (toolbar, status bar) are cyril-dij8's batch and may
        // still paint hardcoded colors — scope the Rgb ban to cells the
        // OVERLAY itself changed vs a no-overlay frame.
        let base = render_buffer(&MockTuiState::default())?;
        let marker = crate::traits::test_support::marker_theme();
        for (name, state, signature) in [
            ("approval", &approval_state, marker.emphasis),
            ("picker", &picker_state, marker.accent_quinary),
            ("hooks", &hooks_state, marker.accent_violet),
            ("code", &code_state, marker.accent_quinary),
        ] {
            let buffer = render_buffer(state)?;
            let mut saw_signature = false;
            for (cell, base_cell) in buffer.content().iter().zip(base.content()) {
                if cell == base_cell {
                    continue; // untouched by the overlay
                }
                assert!(
                    !matches!(cell.fg, Color::Rgb(..)) && !matches!(cell.bg, Color::Rgb(..)),
                    "{name}: overlay painted an Rgb cell — a widget resolved its own theme"
                );
                if cell.fg == signature {
                    saw_signature = true;
                }
            }
            assert!(saw_signature, "{name}: signature marker role never painted");
        }
        Ok(())
    }

    #[test]
    fn theme_seam_picker() -> anyhow::Result<()> {
        let buffer = render_buffer(&picker_state())?;
        insta::assert_debug_snapshot!("theme_seam_picker", buffer);
        Ok(())
    }
}

#[cfg(test)]
mod conversation_baseline_compatibility {
    use std::time::Duration;

    use cyril_core::types::{
        Plan, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation, ToolCallStatus, ToolKind,
    };
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::buffer::Buffer;
    use ratatui::layout::{Constraint, Layout};
    use ratatui::style::Color;
    use ratatui::widgets::Paragraph;

    use crate::theme::{ColorMode, Theme, ThemeId};
    use crate::traits::test_support::MockTuiState;
    use crate::traits::{
        Activity, ChatMessage, ChatMessageKind, SteerEchoStatus, Suggestion, TrackedToolCall,
    };

    const PINNED_COMMIT: &str = "80f3ffa5a7ced20e33c9b98c782c08af704407d5";
    const FIXTURE: &str = include_str!("../tests/fixtures/conversation-theme-baseline.tsv");

    fn truecolor_theme() -> Theme {
        crate::theme::resolve(ThemeId::CyrilDark, ColorMode::TrueColor)
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct NormalizedCell {
        scene: &'static str,
        x: u16,
        y: u16,
        symbol: String,
        foreground: String,
        background: String,
        modifiers: u16,
    }

    fn differences(expected: &[NormalizedCell], actual: &[NormalizedCell]) -> Vec<String> {
        let mut failures = Vec::new();
        if expected.len() != actual.len() {
            failures.push(format!(
                "cell count: expected {}, actual {}",
                expected.len(),
                actual.len()
            ));
        }
        for (expected, actual) in expected.iter().zip(actual) {
            let location = format!("{}[{},{}]", expected.scene, expected.x, expected.y);
            for (field, expected, actual) in [
                ("scene", expected.scene, actual.scene),
                ("symbol", expected.symbol.as_str(), actual.symbol.as_str()),
                (
                    "foreground",
                    expected.foreground.as_str(),
                    actual.foreground.as_str(),
                ),
                (
                    "background",
                    expected.background.as_str(),
                    actual.background.as_str(),
                ),
            ] {
                if expected != actual {
                    failures.push(format!(
                        "{location} {field}: expected {expected:?}, actual {actual:?}"
                    ));
                }
            }
            if expected.x != actual.x || expected.y != actual.y {
                failures.push(format!(
                    "{location} coordinates: expected ({},{}), actual ({},{})",
                    expected.x, expected.y, actual.x, actual.y
                ));
            }
            if expected.modifiers != actual.modifiers {
                failures.push(format!(
                    "{location} modifiers: expected {}, actual {}",
                    expected.modifiers, actual.modifiers
                ));
            }
        }
        failures
    }

    fn steer(text: &str, status: SteerEchoStatus) -> ChatMessage {
        ChatMessage {
            kind: ChatMessageKind::SteerEcho {
                text: text.into(),
                status,
                message_id: None,
            },
            timestamp: std::time::Instant::now(),
        }
    }

    fn message_state(theme: Theme) -> MockTuiState {
        MockTuiState {
            messages: vec![
                ChatMessage::user_text("user".into()),
                ChatMessage::agent_text(String::new()),
                ChatMessage::thought("thought".into()),
                ChatMessage::plan(Plan::new(Vec::new())),
                ChatMessage::system("system".into()),
                ChatMessage::command_output("context".into(), String::new()),
                steer("queued", SteerEchoStatus::Queued),
                steer("applied", SteerEchoStatus::Applied),
                steer("cleared", SteerEchoStatus::Cleared),
                steer("unsupported", SteerEchoStatus::Unsupported),
            ],
            activity: Activity::ToolRunning,
            activity_elapsed: Some(Duration::from_secs(1)),
            theme,
            ..Default::default()
        }
    }

    fn render_message_scene(theme: Theme) -> anyhow::Result<Buffer> {
        let state = message_state(theme);
        let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
        terminal.draw(|frame| {
            crate::widgets::chat::render(frame, frame.area(), &state, &state.theme);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn tool_call(id: &str, title: &str, kind: ToolKind, status: ToolCallStatus) -> TrackedToolCall {
        TrackedToolCall::new(ToolCall::new(
            ToolCallId::new(id),
            title.into(),
            kind,
            status,
            None,
        ))
    }

    fn tool_states(theme: Theme) -> (MockTuiState, MockTuiState) {
        let old_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("old-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let new_text = (0..21)
            .map(|index| {
                if index % 2 == 0 {
                    format!("same-{index}")
                } else {
                    format!("new-{index}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        let write = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("write"),
                "write".into(),
                ToolKind::Write,
                ToolCallStatus::Completed,
                None,
            )
            .with_content(vec![ToolCallContent::Diff {
                path: "diff.rs".into(),
                old_text: Some(old_text),
                new_text,
            }])
            .with_locations(vec![ToolCallLocation {
                path: "diff.rs".into(),
                line: Some(1),
            }]),
        );
        let read = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("read"),
                "read".into(),
                ToolKind::Read,
                ToolCallStatus::Pending,
                None,
            )
            .with_locations(vec![ToolCallLocation {
                path: "read.rs".into(),
                line: None,
            }]),
        );
        let execute = TrackedToolCall::new(
            ToolCall::new(
                ToolCallId::new("execute"),
                "execute".into(),
                ToolKind::Execute,
                ToolCallStatus::Completed,
                Some(serde_json::json!({"command": "cargo test"})),
            )
            .with_raw_output(Some(serde_json::json!({
                "stdout": "line-1\nline-2\nline-3\nline-4\nline-5\nline-6",
                "exit_status": 1
            }))),
        );
        let right = vec![
            ChatMessage::tool_call(read),
            ChatMessage::tool_call(execute),
            ChatMessage::tool_call(tool_call(
                "search",
                "Search(marker)",
                ToolKind::Search,
                ToolCallStatus::InProgress,
            )),
            ChatMessage::tool_call(tool_call(
                "think",
                "think",
                ToolKind::Think,
                ToolCallStatus::Failed,
            )),
            ChatMessage::tool_call(tool_call(
                "fetch",
                "Fetch(url)",
                ToolKind::Fetch,
                ToolCallStatus::Pending,
            )),
            ChatMessage::tool_call(tool_call(
                "switch",
                "Switch(mode)",
                ToolKind::SwitchMode,
                ToolCallStatus::Completed,
            )),
            ChatMessage::tool_call(tool_call(
                "other",
                "Other(custom)",
                ToolKind::Other,
                ToolCallStatus::Failed,
            )),
        ];
        (
            MockTuiState {
                theme,
                messages: vec![ChatMessage::tool_call(write)],
                ..Default::default()
            },
            MockTuiState {
                theme,
                messages: right,
                ..Default::default()
            },
        )
    }

    fn render_tool_scene(theme: Theme) -> anyhow::Result<Buffer> {
        let (left_state, right_state) = tool_states(theme);
        let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
        terminal.draw(|frame| {
            let [left, right] =
                Layout::horizontal([Constraint::Length(40), Constraint::Length(40)])
                    .areas(frame.area());
            crate::widgets::chat::render(frame, left, &left_state, &left_state.theme);
            crate::widgets::chat::render(frame, right, &right_state, &right_state.theme);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn render_markdown_scene(theme: Theme) -> anyhow::Result<Buffer> {
        const HEADINGS: &str = "# H1\n## H2\n### H3\n#### H4\n##### H5\n###### H6";
        const STRUCTURE: &str = "- outer\n  - nested\n\n> quote 世界\n\n[repeat](https://example.com) [repeat](https://example.com)";
        const FORMATTING: &str = "| A | B |\n|---|---|\n| same | same |\n\ninline `code` and **bold** *italic* ~~strike~~\n\n---";
        const CODE: &str = "```rust\nfn syntax_rgb() -> u8 { 42 }\n```\n\n```mystery\nunknown_fallback 世界\n```\n\n```\nlanguage_absent\n```";
        let state = MockTuiState {
            theme,
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
        terminal.draw(|frame| {
            let [left, right] =
                Layout::horizontal([Constraint::Length(40), Constraint::Length(40)])
                    .areas(frame.area());
            let [headings, structure, formatting] = Layout::vertical([
                Constraint::Length(7),
                Constraint::Length(7),
                Constraint::Min(1),
            ])
            .areas(left);
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render_with_theme(
                    HEADINGS,
                    40,
                    &state.theme,
                )),
                headings,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render_with_theme(
                    STRUCTURE,
                    40,
                    &state.theme,
                )),
                structure,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render_with_theme(
                    FORMATTING,
                    40,
                    &state.theme,
                )),
                formatting,
            );
            frame.render_widget(
                Paragraph::new(crate::widgets::markdown::render_with_theme(
                    CODE,
                    40,
                    &state.theme,
                )),
                right,
            );
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn input_state(theme: Theme) -> MockTuiState {
        let suggestions = (0..21)
            .map(|index| {
                let text = match index {
                    7 | 8 => "duplicate".into(),
                    10 => "選択".into(),
                    11 => "with spaces".into(),
                    _ => format!("item-{index}"),
                };
                Suggestion {
                    text,
                    description: (index % 2 == 0).then(|| format!("description-{index}")),
                }
            })
            .collect();
        MockTuiState {
            theme,
            input_text: "first\nUnicode 世界\nthird".into(),
            input_cursor: "first\nUnicode ".len(),
            autocomplete_suggestions: suggestions,
            autocomplete_selected: Some(10),
            ..Default::default()
        }
    }

    fn render_input_scene(theme: Theme) -> anyhow::Result<Buffer> {
        let state = input_state(theme);
        let mut terminal = Terminal::new(TestBackend::new(80, 24))?;
        terminal.draw(|frame| {
            let [input, suggestions, _] = Layout::vertical([
                Constraint::Length(5),
                Constraint::Length(10),
                Constraint::Min(0),
            ])
            .areas(frame.area());
            crate::widgets::input::render(frame, input, &state, &state.theme);
            crate::widgets::suggestions::render(frame, suggestions, &state, &state.theme);
        })?;
        Ok(terminal.backend().buffer().clone())
    }

    fn normalized_color(color: Color) -> String {
        let rgb = match color {
            Color::Reset => return "DEFAULT".into(),
            Color::Black => (0x00, 0x00, 0x00),
            Color::Red => (0x80, 0x00, 0x00),
            Color::Green => (0x00, 0x80, 0x00),
            Color::Yellow => (0x80, 0x80, 0x00),
            Color::Blue => (0x00, 0x00, 0x80),
            Color::Magenta => (0x80, 0x00, 0x80),
            Color::Cyan => (0x00, 0x80, 0x80),
            Color::Gray => (0xc0, 0xc0, 0xc0),
            Color::DarkGray => (0x80, 0x80, 0x80),
            Color::LightRed => (0xff, 0x00, 0x00),
            Color::LightGreen => (0x00, 0xff, 0x00),
            Color::LightYellow => (0xff, 0xff, 0x00),
            Color::LightBlue => (0x00, 0x00, 0xff),
            Color::LightMagenta => (0xff, 0x00, 0xff),
            Color::LightCyan => (0x00, 0xff, 0xff),
            Color::White => (0xff, 0xff, 0xff),
            Color::Rgb(red, green, blue) => (red, green, blue),
            Color::Indexed(index) => return format!("INDEX:{index}"),
        };
        format!("RGB:{:02x}{:02x}{:02x}", rgb.0, rgb.1, rgb.2)
    }

    fn symbol_hex(symbol: &str) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut encoded = String::with_capacity(symbol.len() * 2);
        for byte in symbol.bytes() {
            encoded.push(HEX[(byte >> 4) as usize] as char);
            encoded.push(HEX[(byte & 0x0f) as usize] as char);
        }
        encoded
    }

    fn normalized_scene(
        scene: &'static str,
        buffer: &Buffer,
    ) -> anyhow::Result<Vec<NormalizedCell>> {
        let mut cells = Vec::with_capacity(1_920);
        for y in 0..24 {
            for x in 0..80 {
                let cell = buffer
                    .cell((x, y))
                    .ok_or_else(|| anyhow::anyhow!("missing {scene}[{x},{y}]"))?;
                cells.push(NormalizedCell {
                    scene,
                    x,
                    y,
                    symbol: symbol_hex(cell.symbol()),
                    foreground: normalized_color(cell.fg),
                    background: normalized_color(cell.bg),
                    modifiers: cell.modifier.bits(),
                });
            }
        }
        Ok(cells)
    }

    fn expected_cells() -> anyhow::Result<Vec<NormalizedCell>> {
        let mut lines = FIXTURE.lines();
        let expected_header = format!("commit\t{PINNED_COMMIT}");
        assert_eq!(
            lines.next(),
            Some(expected_header.as_str()),
            "baseline commit header must remain pinned"
        );
        assert_eq!(
            lines.next(),
            Some("scene\tx\ty\tsymbol_hex\tforeground\tbackground\tmodifier_bits")
        );
        lines
            .map(|line| {
                let fields = line.split('\t').collect::<Vec<_>>();
                Ok(NormalizedCell {
                    scene: match fields.first().copied() {
                        Some("messages") => "messages",
                        Some("tools") => "tools",
                        Some("markdown") => "markdown",
                        Some("input") => "input",
                        Some(scene) => {
                            return Err(anyhow::anyhow!("unknown fixture scene {scene}"));
                        }
                        None => return Err(anyhow::anyhow!("missing fixture scene")),
                    },
                    x: fields
                        .get(1)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture x"))?
                        .parse()?,
                    y: fields
                        .get(2)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture y"))?
                        .parse()?,
                    symbol: fields
                        .get(3)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture symbol"))?
                        .to_string(),
                    foreground: fields
                        .get(4)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture foreground"))?
                        .to_string(),
                    background: fields
                        .get(5)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture background"))?
                        .to_string(),
                    modifiers: fields
                        .get(6)
                        .ok_or_else(|| anyhow::anyhow!("missing fixture modifiers"))?
                        .parse()?,
                })
            })
            .collect()
    }

    fn scene_buffers(theme: Theme) -> anyhow::Result<[(&'static str, Buffer); 4]> {
        Ok([
            ("messages", render_message_scene(theme)?),
            ("tools", render_tool_scene(theme)?),
            ("markdown", render_markdown_scene(theme)?),
            ("input", render_input_scene(theme)?),
        ])
    }

    fn actual_cells() -> anyhow::Result<Vec<NormalizedCell>> {
        let mut cells = Vec::with_capacity(7_680);
        for (scene, buffer) in scene_buffers(truecolor_theme())? {
            cells.extend(normalized_scene(scene, &buffer)?);
        }
        Ok(cells)
    }

    fn theme_colors(theme: &Theme) -> [Color; 29] {
        [
            theme.canvas,
            theme.chrome,
            theme.code,
            theme.selection,
            theme.text,
            theme.muted,
            theme.border,
            theme.accent,
            theme.accent_alt,
            theme.user,
            theme.agent,
            theme.system,
            theme.info,
            theme.success,
            theme.warning,
            theme.danger,
            theme.diff_add,
            theme.diff_delete,
            theme.diff_context,
            theme.emphasis,
            theme.accent_tertiary,
            theme.accent_quaternary,
            theme.accent_quinary,
            theme.subdued,
            theme.subdued_positive,
            theme.subdued_negative,
            theme.soft_accent,
            theme.positive_accent,
            theme.inset_background,
        ]
    }

    fn validate_projected_color(
        marker_color: Color,
        actual_color: Color,
        marker_theme: &Theme,
        projected_theme: &Theme,
    ) -> Result<(), String> {
        if marker_color == Color::Reset {
            return (actual_color == Color::Reset)
                .then_some(())
                .ok_or_else(|| format!("default projected as {actual_color:?}"));
        }

        let marker_roles = theme_colors(marker_theme);
        if let Some(role_index) = marker_roles.iter().position(|color| *color == marker_color) {
            let expected = theme_colors(projected_theme)[role_index];
            return (actual_color == expected).then_some(()).ok_or_else(|| {
                format!("role {role_index} expected {expected:?}, actual {actual_color:?}")
            });
        }

        if !matches!(marker_color, Color::Rgb(_, _, _)) {
            return Err(format!("unknown marker color {marker_color:?}"));
        }
        let expected = if projected_theme.syntax.is_some() {
            marker_color
        } else {
            Color::Reset
        };
        (actual_color == expected)
            .then_some(())
            .ok_or_else(|| format!("syntax expected {expected:?}, actual {actual_color:?}"))
    }

    #[derive(Debug)]
    struct ModePass {
        label: String,
        no_color_non_reset: Option<usize>,
    }

    fn mode_matrix() -> anyhow::Result<Vec<ModePass>> {
        let mut marker_theme = crate::traits::test_support::marker_theme();
        marker_theme.syntax = truecolor_theme().syntax;
        let marker_scenes = scene_buffers(marker_theme)?;
        let modes = [
            ("truecolor", ColorMode::TrueColor),
            ("ansi256", ColorMode::Ansi256),
            ("ansi16", ColorMode::Ansi16),
            ("no-color", ColorMode::None),
        ];
        let mut passes = Vec::with_capacity(16);

        for (mode_label, mode) in modes {
            let projected_theme = crate::theme::resolve(ThemeId::CyrilDark, mode);
            let projected_scenes = scene_buffers(projected_theme)?;
            for ((marker_label, marker), (projected_label, projected)) in
                marker_scenes.iter().zip(&projected_scenes)
            {
                assert_eq!(marker_label, projected_label);
                let mut no_color_non_reset = 0usize;
                for (index, (marker_cell, projected_cell)) in
                    marker.content().iter().zip(projected.content()).enumerate()
                {
                    if marker_cell.symbol() != projected_cell.symbol()
                        || marker_cell.modifier != projected_cell.modifier
                    {
                        return Err(anyhow::anyhow!(
                            "{mode_label}/{marker_label} geometry drift at cell {index}"
                        ));
                    }
                    validate_projected_color(
                        marker_cell.fg,
                        projected_cell.fg,
                        &marker_theme,
                        &projected_theme,
                    )
                    .map_err(|reason| {
                        anyhow::anyhow!(
                            "{mode_label}/{marker_label} foreground cell {index}: {reason}"
                        )
                    })?;
                    validate_projected_color(
                        marker_cell.bg,
                        projected_cell.bg,
                        &marker_theme,
                        &projected_theme,
                    )
                    .map_err(|reason| {
                        anyhow::anyhow!(
                            "{mode_label}/{marker_label} background cell {index}: {reason}"
                        )
                    })?;
                    if mode == ColorMode::None {
                        no_color_non_reset += usize::from(projected_cell.fg != Color::Reset);
                        no_color_non_reset += usize::from(projected_cell.bg != Color::Reset);
                    }
                }
                passes.push(ModePass {
                    label: format!("{mode_label}/{marker_label}"),
                    no_color_non_reset: (mode == ColorMode::None).then_some(no_color_non_reset),
                });
            }
        }
        Ok(passes)
    }

    fn fixture_cell() -> NormalizedCell {
        NormalizedCell {
            scene: "messages",
            x: 0,
            y: 0,
            symbol: "59".into(),
            foreground: "RGB:8ab4f8".into(),
            background: "DEFAULT".into(),
            modifiers: 1,
        }
    }

    #[test]
    fn compatibility_reports_foreground_mutation() {
        let expected = fixture_cell();
        let mut actual = expected.clone();
        actual.foreground = "RGB:00ffff".into();
        let failures = differences(&[expected], &[actual]);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("foreground"));
        assert!(failures[0].contains("messages[0,0]"));
    }

    #[test]
    fn compatibility_reports_modifier_mutation() {
        let expected = fixture_cell();
        let mut actual = expected.clone();
        actual.modifiers = 0;
        let failures = differences(&[expected], &[actual]);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("modifiers"));
        assert!(failures[0].contains("messages[0,0]"));
    }

    #[test]
    fn mode_classifier_rejects_hardcoded_cyan_in_every_mode() {
        let marker = crate::traits::test_support::marker_theme();
        for mode in [
            ColorMode::TrueColor,
            ColorMode::Ansi256,
            ColorMode::Ansi16,
            ColorMode::None,
        ] {
            let projected = crate::theme::resolve(ThemeId::CyrilDark, mode);
            assert!(
                validate_projected_color(marker.text, Color::Cyan, &marker, &projected).is_err(),
                "hardcoded cyan accepted in {mode:?}"
            );
        }
    }

    #[test]
    fn mode_classifier_accepts_syntax_rgb_in_colored_modes() {
        let marker = crate::traits::test_support::marker_theme();
        let syntax = Color::Rgb(1, 2, 3);
        for mode in [ColorMode::TrueColor, ColorMode::Ansi256, ColorMode::Ansi16] {
            let projected = crate::theme::resolve(ThemeId::CyrilDark, mode);
            assert!(
                validate_projected_color(syntax, syntax, &marker, &projected).is_ok(),
                "syntax RGB rejected in {mode:?}"
            );
        }
    }

    #[test]
    fn mode_classifier_rejects_syntax_rgb_in_no_color() {
        let marker = crate::traits::test_support::marker_theme();
        let no_color = crate::theme::resolve(ThemeId::CyrilDark, ColorMode::None);
        assert!(
            validate_projected_color(Color::Rgb(1, 2, 3), Color::Rgb(1, 2, 3), &marker, &no_color,)
                .is_err()
        );
    }

    #[test]
    fn all_sixteen_scene_mode_combinations_pass() -> anyhow::Result<()> {
        let passes = mode_matrix()?;
        assert_eq!(passes.len(), 16);
        let labels = passes
            .iter()
            .map(|pass| pass.label.as_str())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(labels.len(), 16);
        let no_color_counts = passes
            .iter()
            .filter_map(|pass| pass.no_color_non_reset)
            .collect::<Vec<_>>();
        assert_eq!(no_color_counts, vec![0, 0, 0, 0]);
        Ok(())
    }

    #[test]
    fn migrated_scenes_match_all_pinned_cells() -> anyhow::Result<()> {
        let expected = expected_cells()?;
        let actual = actual_cells()?;
        assert_eq!(expected.len(), 7_680);
        assert_eq!(actual.len(), 7_680);

        let failures = differences(&expected, &actual);
        assert!(
            failures.is_empty(),
            "expected 0/7,680 differences, found {}:\n{}",
            failures.len(),
            failures.join("\n")
        );
        Ok(())
    }
}
