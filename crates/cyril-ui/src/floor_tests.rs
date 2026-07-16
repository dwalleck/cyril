//! cyril-a14l regression fences: the 60×16 floor contract and the roomy
//! (≥80×24) parity baseline. See `.cyril-a14l/design.md` for the claim
//! table these tests fence (C1–C11).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use crate::traits::test_support::MockTuiState;
use crate::traits::{ApprovalPhase, ApprovalState, ChatMessage, Suggestion};

fn render_frame(state: &MockTuiState, width: u16, height: u16) -> anyhow::Result<Buffer> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| crate::render::draw(frame, state))?;
    Ok(terminal.backend().buffer().clone())
}

fn chat_messages(count: usize) -> Vec<ChatMessage> {
    (1..=count)
        .map(|index| ChatMessage::user_text(format!("chat-{index}")))
        .collect()
}

fn suggestions(count: usize) -> Vec<Suggestion> {
    (1..=count)
        .map(|index| Suggestion {
            text: format!("@fence-file-{index}.rs"),
            description: None,
        })
        .collect()
}

fn approval_state(option_count: usize) -> ApprovalState {
    use cyril_core::types::{
        PermissionOption, PermissionOptionId, PermissionOptionKind, ToolCall, ToolCallId,
        ToolCallStatus, ToolKind,
    };
    ApprovalState {
        tool_call: ToolCall::new(
            ToolCallId::new("fence"),
            "Run `cargo test`".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            None,
        ),
        message: "Allow cargo test?".into(),
        options: (0..option_count)
            .map(|index| PermissionOption {
                id: PermissionOptionId::new(format!("opt-{index}")),
                label: format!("Option {index}"),
                kind: PermissionOptionKind::AllowOnce,
                is_destructive: false,
            })
            .collect(),
        trust_options: vec![],
        selected: 0,
        phase: ApprovalPhase::SelectOption,
        responder: tokio::sync::oneshot::channel().0,
    }
}

fn buffer_rows(buffer: &Buffer) -> Vec<String> {
    let area = *buffer.area();
    (0..area.height)
        .map(|y| {
            (0..area.width)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map_or(" ", ratatui::buffer::Cell::symbol)
                })
                .collect::<String>()
                .trim_end()
                .to_string()
        })
        .collect()
}

/// cyril-a14l C7 (slice 3): a picker clamped above the input keeps the
/// selected `▸` row visible (cc5e's height-adaptive window at the new,
/// smaller popup height) and paints nothing at or below `input_top`.
#[test]
fn picker_clamped_above_input_keeps_selection_visible() -> anyhow::Result<()> {
    use crate::traits::PickerState;
    use cyril_core::types::CommandOption;

    let options: Vec<CommandOption> = (0..4)
        .map(|index| CommandOption {
            label: format!("model-{index}"),
            value: format!("model-{index}"),
            description: Some(format!("description-{index}")),
            group: None,
            is_current: index == 0,
        })
        .collect();
    let picker = PickerState {
        title: "Select model".into(),
        options,
        filter: String::new(),
        filtered_indices: vec![0, 1, 2, 3],
        selected: 3,
    };
    let state = MockTuiState::default();
    let mut terminal = Terminal::new(TestBackend::new(60, 16))?;
    terminal.draw(|frame| {
        crate::widgets::picker::render(frame, frame.area(), 8, &picker, &state.theme);
    })?;
    let rows = buffer_rows(terminal.backend().buffer());
    assert!(
        rows.iter().any(|row| row.contains("▸ model-3")),
        "selected picker row missing: {rows:?}"
    );
    for (index, row) in rows.iter().enumerate().skip(8) {
        assert_eq!(row, "", "picker bled into row {index}: {rows:?}");
    }
    Ok(())
}

/// cyril-a14l C7 (slice 3): a hooks panel clamped above the input windows
/// its scrolled rows within the smaller popup and paints nothing at or
/// below `input_top`.
#[test]
fn hooks_clamped_above_input_windows_scrolled_rows() -> anyhow::Result<()> {
    use crate::traits::HooksPanelState;
    use cyril_core::types::HookInfo;

    let hooks = HooksPanelState {
        hooks: (0..12)
            .map(|index| HookInfo {
                trigger: format!("trigger-{index}"),
                command: format!("command-{index}"),
                matcher: None,
            })
            .collect(),
        scroll_offset: 8,
    };
    let state = MockTuiState::default();
    let mut terminal = Terminal::new(TestBackend::new(60, 16))?;
    terminal.draw(|frame| {
        crate::widgets::hooks_panel::render(frame, frame.area(), 8, &hooks, &state.theme);
    })?;
    let rows = buffer_rows(terminal.backend().buffer());
    assert!(
        rows.iter().any(|row| row.contains("trigger-8")),
        "scrolled hook row missing: {rows:?}"
    );
    for (index, row) in rows.iter().enumerate().skip(8) {
        assert_eq!(row, "", "hooks panel bled into row {index}: {rows:?}");
    }
    Ok(())
}

/// Absolute row of the input box's top border, parsed from the rendered
/// frame (the input block is the only bordered widget whose top border
/// carries the " > " title) — independent of the layout arithmetic.
fn input_top_row(buffer: &Buffer) -> Option<u16> {
    let area = *buffer.area();
    (0..area.height).find(|&y| {
        let row: String = (0..area.width)
            .map(|x| {
                buffer
                    .cell((x, y))
                    .map_or(" ", ratatui::buffer::Cell::symbol)
            })
            .collect();
        row.contains("┌ > ")
    })
}

/// cyril-a14l C7 fence (slice 4): for every overlay kind × frame size ×
/// input shape, every cell the overlay changes sits strictly between the
/// toolbar and the input's top border. Probe S4/S5 showed the pre-a14l
/// popups covering input rows 10–11 at 60×16 — this fence fails on that
/// geometry.
#[test]
fn modals_never_cover_input() -> anyhow::Result<()> {
    use crate::traits::{HooksPanelState, PickerState};
    use cyril_core::types::{CodePanelData, HookInfo, LspStatus};

    let big_draft = (1..=10)
        .map(|index| format!("draft-{index}"))
        .collect::<Vec<_>>()
        .join("\n");

    type OverlayMutator = Box<dyn Fn(&mut MockTuiState)>;
    let overlay_variants: Vec<(&str, OverlayMutator)> = vec![
        (
            "approval-select",
            Box::new(|state: &mut MockTuiState| state.approval = Some(approval_state(3))),
        ),
        (
            "approval-trust",
            Box::new(|state: &mut MockTuiState| {
                use cyril_core::types::{PermissionOptionId, TrustOption};
                let mut approval = approval_state(1);
                approval.trust_options = (0..3)
                    .map(|index| TrustOption {
                        label: format!("Tier {index}"),
                        display: format!("pattern-{index}"),
                        setting_key: "allowedCommands".into(),
                        patterns: vec![format!("pattern-{index}")],
                    })
                    .collect();
                approval.selected = 2;
                approval.phase = crate::traits::ApprovalPhase::SelectTrust {
                    chosen_option_id: PermissionOptionId::new("opt-0"),
                };
                state.approval = Some(approval);
            }),
        ),
        (
            "picker",
            Box::new(|state: &mut MockTuiState| {
                use cyril_core::types::CommandOption;
                state.picker = Some(PickerState {
                    title: "Select model".into(),
                    options: (0..4)
                        .map(|index| CommandOption {
                            label: format!("model-{index}"),
                            value: format!("model-{index}"),
                            description: Some("description".into()),
                            group: None,
                            is_current: false,
                        })
                        .collect(),
                    filter: String::new(),
                    filtered_indices: vec![0, 1, 2, 3],
                    selected: 3,
                });
            }),
        ),
        (
            "hooks",
            Box::new(|state: &mut MockTuiState| {
                state.hooks_panel = Some(HooksPanelState {
                    hooks: (0..12)
                        .map(|index| HookInfo {
                            trigger: format!("trigger-{index}"),
                            command: format!("command-{index}"),
                            matcher: None,
                        })
                        .collect(),
                    scroll_offset: 0,
                });
            }),
        ),
        (
            "code",
            Box::new(|state: &mut MockTuiState| {
                state.code_panel = Some(CodePanelData {
                    status: LspStatus::Initialized,
                    message: Some("ready".into()),
                    warning: None,
                    root_path: Some("/repo".into()),
                    detected_languages: vec!["rust".into()],
                    project_markers: vec!["Cargo.toml".into()],
                    config_path: None,
                    doc_url: None,
                    lsps: vec![],
                });
            }),
        ),
    ];

    for (width, height) in [(60u16, 16u16), (80, 24)] {
        for (input_label, input_text) in [("one-line", "reply"), ("max-draft", big_draft.as_str())]
        {
            let base_state = MockTuiState {
                messages: chat_messages(6),
                input_text: input_text.into(),
                input_cursor: input_text.len(),
                ..Default::default()
            };
            let base = render_frame(&base_state, width, height)?;
            let input_top = input_top_row(&base).ok_or_else(|| {
                anyhow::anyhow!("no input border in base frame {width}x{height} {input_label}")
            })?;

            for (overlay_label, mutate) in &overlay_variants {
                let mut state = MockTuiState {
                    messages: chat_messages(6),
                    input_text: input_text.into(),
                    input_cursor: input_text.len(),
                    ..Default::default()
                };
                mutate(&mut state);
                let overlaid = render_frame(&state, width, height)?;
                for (index, (cell, base_cell)) in
                    overlaid.content().iter().zip(base.content()).enumerate()
                {
                    if cell == base_cell {
                        continue;
                    }
                    let y = u16::try_from(index / usize::from(width))?;
                    anyhow::ensure!(
                        y >= 1 && y < input_top,
                        "{overlay_label} at {width}x{height}/{input_label}: changed cell on \
                         row {y} (input_top={input_top})"
                    );
                }
            }
        }
    }
    Ok(())
}

/// C6 (slice 0): the roomy 80×24 frame is pinned BEFORE any layout change
/// on this branch — later slices must keep all three scenes byte-identical.
#[test]
fn roomy_frame_matches_main_fixture() -> anyhow::Result<()> {
    let idle = MockTuiState {
        messages: chat_messages(6),
        ..Default::default()
    };
    insta::assert_debug_snapshot!("roomy_idle_80x24", render_frame(&idle, 80, 24)?);

    let draft = "draft-1\ndraft-2\ndraft-3";
    let inflow = MockTuiState {
        messages: chat_messages(6),
        input_text: draft.into(),
        input_cursor: draft.len(),
        autocomplete_suggestions: suggestions(10),
        autocomplete_selected: Some(5),
        ..Default::default()
    };
    insta::assert_debug_snapshot!("roomy_inflow_80x24", render_frame(&inflow, 80, 24)?);

    let approval = MockTuiState {
        messages: chat_messages(6),
        input_text: "reply".into(),
        input_cursor: "reply".len(),
        approval: Some(approval_state(3)),
        ..Default::default()
    };
    insta::assert_debug_snapshot!("roomy_approval_80x24", render_frame(&approval, 80, 24)?);
    Ok(())
}
