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
