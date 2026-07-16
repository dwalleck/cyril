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

/// The input box rect parsed from a frame: (top row, bottom row). Bottom is
/// the matching `└` row below the top border.
fn input_rect_rows(buffer: &Buffer) -> Option<(u16, u16)> {
    let top = input_top_row(buffer)?;
    let area = *buffer.area();
    let bottom = (top + 1..area.height)
        .find(|&y| buffer.cell((0, y)).is_some_and(|cell| cell.symbol() == "└"))?;
    Some((top, bottom))
}

/// Cell coordinates of a `█` found inside the input content rect.
type CursorCells = Vec<(u16, u16)>;

/// Input content rows (between the borders), trimmed, plus the cell
/// coordinates of every `█` inside the content rect.
fn input_content(buffer: &Buffer) -> Option<(Vec<String>, CursorCells)> {
    let (top, bottom) = input_rect_rows(buffer)?;
    let area = *buffer.area();
    let mut rows = Vec::new();
    let mut cursors = Vec::new();
    for y in top + 1..bottom {
        let row: String = (1..area.width - 1)
            .map(|x| {
                let symbol = buffer
                    .cell((x, y))
                    .map_or(" ", ratatui::buffer::Cell::symbol);
                if symbol == "\u{2588}" {
                    cursors.push((x, y));
                }
                symbol
            })
            .collect();
        rows.push(row.trim_end().to_string());
    }
    Some((rows, cursors))
}

/// cyril-a14l C2 fence (slice 6): a 10-line draft with the cursor at the
/// end keeps exactly one cursor block visible inside the input content
/// rect at 60×16, at the position the char-wrap oracle + follow-window
/// formula predict. Pre-a14l code showed ZERO cursor cells here (probe S1).
#[test]
fn input_cursor_always_visible() -> anyhow::Result<()> {
    let draft = (1..=10)
        .map(|index| format!("draft-{index}"))
        .collect::<Vec<_>>()
        .join("\n");
    let state = MockTuiState {
        messages: chat_messages(6),
        input_text: draft.clone(),
        input_cursor: draft.len(),
        ..Default::default()
    };
    let buffer = render_frame(&state, 60, 16)?;
    let (rows, cursors) = input_content(&buffer)
        .ok_or_else(|| anyhow::anyhow!("input box not found in 60x16 frame"))?;

    // Independent expectation: 10 visual rows (no line wraps at width 58,
    // per the committed python-oracle fixture draft-10/58 case), cursor on
    // visual row 9 at char col 8; window start = 9 - (content_rows - 1).
    let content_rows = rows.len();
    anyhow::ensure!(content_rows >= 1, "input has no content rows");
    let start = 9usize.saturating_sub(content_rows - 1);
    anyhow::ensure!(
        rows[0].starts_with(&format!("draft-{}", start + 1)),
        "window start drifted: first visible row {:?}, expected draft-{}",
        rows[0],
        start + 1
    );
    anyhow::ensure!(
        rows[content_rows - 1].starts_with("draft-10\u{2588}"),
        "cursor row not visible: last row {:?}",
        rows[content_rows - 1]
    );
    anyhow::ensure!(
        cursors.len() == 1,
        "expected exactly one cursor cell in the input rect, found {cursors:?}"
    );
    Ok(())
}

/// cyril-a14l C3 fence (slice 6): the visible input window is exactly the
/// slice of logical lines containing the cursor, in order, for cursor at
/// start / middle / end of a 30-line unicode draft. Expected windows are
/// computed here from pure line arithmetic — independent of wrapped_rows.
#[test]
fn input_scroll_window_matches_oracle() -> anyhow::Result<()> {
    let lines: Vec<String> = (1..=30).map(|index| format!("世界-{index}")).collect();
    let text = lines.join("\n");
    let middle_cursor: usize = lines[..14].iter().map(|line| line.len() + 1).sum();

    for (label, cursor, cursor_line) in [
        ("start", 0usize, 0usize),
        ("middle", middle_cursor, 14),
        ("end", text.len(), 29),
    ] {
        let state = MockTuiState {
            messages: chat_messages(6),
            input_text: text.clone(),
            input_cursor: cursor,
            ..Default::default()
        };
        let buffer = render_frame(&state, 60, 16)?;
        let (rows, cursors) = input_content(&buffer)
            .ok_or_else(|| anyhow::anyhow!("{label}: input box not found"))?;
        let content_rows = rows.len();
        let start = cursor_line.saturating_sub(content_rows - 1);

        let expected: Vec<String> = (start..(start + content_rows).min(30))
            .map(|index| {
                if index == cursor_line {
                    // The three fixtures put the cursor at a line start
                    // (start/middle) or at the very end of the text.
                    let line = &lines[index];
                    if cursor == text.len() {
                        format!("{line}\u{2588}")
                    } else {
                        format!("\u{2588}{line}")
                    }
                } else {
                    lines[index].clone()
                }
            })
            .collect();
        // Buffer cells: a wide char renders as its symbol plus a blank
        // continuation cell — expand expectations to cell space.
        let expected: Vec<String> = expected
            .iter()
            .map(|line| {
                use unicode_width::UnicodeWidthChar;
                line.chars()
                    .flat_map(|character| {
                        std::iter::once(character.to_string()).chain(std::iter::repeat_n(
                            " ".to_string(),
                            character.width().unwrap_or(0).saturating_sub(1),
                        ))
                    })
                    .collect::<String>()
            })
            .collect();
        anyhow::ensure!(
            rows[..expected.len()] == expected[..],
            "{label}: window mismatch\n got: {rows:?}\n want: {expected:?}"
        );
        anyhow::ensure!(
            cursors.len() == 1,
            "{label}: expected one cursor cell, found {cursors:?}"
        );
    }
    Ok(())
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

/// cyril-a14l C1 fence (slice 7): at every size ≥60×16, for adversarial
/// state shapes (max draft, draft+suggestions, crew panel present), the
/// frame keeps the toolbar, the status row, ≥3 chat rows, and an input of
/// ≥3 rows with both borders. A budget that forgets the crew rows in its
/// arithmetic over-allocates the input and fails the chat-floor half.
#[test]
fn layout_floors_hold_across_adversarial_matrix() -> anyhow::Result<()> {
    use cyril_core::types::{Notification, SessionId, SubagentInfo, SubagentStatus};

    let draft = (1..=10)
        .map(|index| format!("draft-{index}"))
        .collect::<Vec<_>>()
        .join("\n");

    let mut crew_state = MockTuiState {
        messages: chat_messages(6),
        input_text: draft.clone(),
        input_cursor: draft.len(),
        ..Default::default()
    };
    crew_state
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

    let states = [
        (
            "max-draft",
            MockTuiState {
                messages: chat_messages(6),
                input_text: draft.clone(),
                input_cursor: draft.len(),
                ..Default::default()
            },
        ),
        (
            "draft+suggestions",
            MockTuiState {
                messages: chat_messages(6),
                input_text: draft.clone(),
                input_cursor: draft.len(),
                autocomplete_suggestions: suggestions(10),
                autocomplete_selected: Some(7),
                ..Default::default()
            },
        ),
        ("crew+draft", crew_state),
    ];

    for (label, state) in &states {
        let crew = crate::widgets::crew_panel::height_for(state);
        let voice = crate::widgets::voice::height_for(state);
        for (width, height) in [(60u16, 16u16), (60, 17), (61, 20), (80, 24)] {
            let buffer = render_frame(state, width, height)?;
            let (input_top, input_bottom) = input_rect_rows(&buffer)
                .ok_or_else(|| anyhow::anyhow!("{label} {width}x{height}: input box not found"))?;
            anyhow::ensure!(
                input_bottom - input_top + 1 >= 3,
                "{label} {width}x{height}: input shrank below its floor \
                 (rows {input_top}..={input_bottom})"
            );
            let chat_rows = input_top
                .saturating_sub(1)
                .saturating_sub(crew)
                .saturating_sub(voice);
            anyhow::ensure!(
                chat_rows >= 3,
                "{label} {width}x{height}: chat floor broken ({chat_rows} rows, \
                 input_top={input_top}, crew={crew}, voice={voice})"
            );
            let status_row: String = (0..width)
                .map(|x| {
                    buffer
                        .cell((x, height - 1))
                        .map_or(" ", ratatui::buffer::Cell::symbol)
                })
                .collect();
            anyhow::ensure!(
                !status_row.trim().is_empty(),
                "{label} {width}x{height}: status row missing"
            );
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
