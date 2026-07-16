//! cyril-a14l prove-it-prototype probe: what does the REAL frame allocate at
//! 60×16 today? Renders the production `render::draw` (not a re-derivation of
//! its constraints) and dumps the buffer per state.
//!
//! Run: `cargo test -p cyril-ui probe_a14l -- --nocapture`
//! Oracle: real binary in a 60×16 tmux pane (see .cyril-a14l/findings.md).

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;

use crate::traits::test_support::MockTuiState;
use crate::traits::{ApprovalPhase, ApprovalState, ChatMessage, Suggestion};

fn render_at(state: &MockTuiState, width: u16, height: u16) -> anyhow::Result<Buffer> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| crate::render::draw(frame, state))?;
    Ok(terminal.backend().buffer().clone())
}

fn dump(label: &str, buffer: &Buffer) {
    let area = *buffer.area();
    println!("=== {label} ({}x{}) ===", area.width, area.height);
    for y in 0..area.height {
        let row: String = (0..area.width)
            .map(|x| {
                buffer
                    .cell((x, y))
                    .map_or(" ", ratatui::buffer::Cell::symbol)
            })
            .collect();
        println!("{y:2}|{row}|");
    }
    let cursors = buffer
        .content()
        .iter()
        .filter(|cell| cell.symbol() == "\u{2588}")
        .count();
    println!("-- cursor cells visible: {cursors}");
}

fn messages() -> Vec<ChatMessage> {
    (1..=6)
        .map(|index| ChatMessage::user_text(format!("chat-{index}")))
        .collect()
}

fn big_draft() -> String {
    (1..=10)
        .map(|index| format!("draft-{index}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn file_suggestions() -> Vec<Suggestion> {
    (1..=10)
        .map(|index| Suggestion {
            text: format!("@probe-file-{index}.rs"),
            description: None,
        })
        .collect()
}

fn approval() -> ApprovalState {
    use cyril_core::types::{
        PermissionOption, PermissionOptionId, PermissionOptionKind, ToolCall, ToolCallId,
        ToolCallStatus, ToolKind,
    };
    let option = |id: &str, label: &str| PermissionOption {
        id: PermissionOptionId::new(id),
        label: label.into(),
        kind: PermissionOptionKind::AllowOnce,
        is_destructive: false,
    };
    ApprovalState {
        tool_call: ToolCall::new(
            ToolCallId::new("probe"),
            "Run `cargo test`".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            None,
        ),
        message: "Allow cargo test?".into(),
        options: vec![option("y", "Yes"), option("a", "Always"), option("n", "No")],
        trust_options: vec![],
        selected: 0,
        phase: ApprovalPhase::SelectOption,
        responder: tokio::sync::oneshot::channel().0,
    }
}

fn picker() -> crate::traits::PickerState {
    use cyril_core::types::CommandOption;
    let option = |label: &str| CommandOption {
        label: label.into(),
        value: label.to_lowercase(),
        description: Some(format!("{label} description")),
        group: None,
        is_current: false,
    };
    crate::traits::PickerState {
        title: "Select model".into(),
        options: vec![option("Sonnet"), option("Opus"), option("Haiku")],
        filter: String::new(),
        filtered_indices: vec![0, 1, 2],
        selected: 0,
    }
}

#[test]
fn probe_60x16_dump() -> anyhow::Result<()> {
    let draft = big_draft();

    // S1: large multiline draft. Control at 80×24, then the 60×16 floor.
    let s1 = MockTuiState {
        messages: messages(),
        input_text: draft.clone(),
        input_cursor: draft.len(),
        ..Default::default()
    };
    dump("S1-control big-draft", &render_at(&s1, 80, 24)?);
    dump("S1 big-draft", &render_at(&s1, 60, 16)?);

    // S2: autocomplete active with a one-line input.
    let s2 = MockTuiState {
        messages: messages(),
        input_text: "@probe".into(),
        input_cursor: "@probe".len(),
        autocomplete_suggestions: file_suggestions(),
        autocomplete_selected: Some(0),
        ..Default::default()
    };
    dump("S2 autocomplete", &render_at(&s2, 60, 16)?);

    // S2b: selection deep in the list — does the ▸ marker stay visible in the
    // squeezed suggestion area?
    let s2b = MockTuiState {
        messages: messages(),
        input_text: "@probe".into(),
        input_cursor: "@probe".len(),
        autocomplete_suggestions: file_suggestions(),
        autocomplete_selected: Some(7),
        ..Default::default()
    };
    dump("S2b autocomplete-selected-7", &render_at(&s2b, 60, 16)?);

    // S3: worst case — max draft demand and max suggestion demand together.
    let s3 = MockTuiState {
        messages: messages(),
        input_text: draft.clone(),
        input_cursor: draft.len(),
        autocomplete_suggestions: file_suggestions(),
        autocomplete_selected: Some(0),
        ..Default::default()
    };
    dump("S3 draft+suggestions", &render_at(&s3, 60, 16)?);

    // S4: approval modal over a one-line input.
    let s4 = MockTuiState {
        messages: messages(),
        input_text: "reply".into(),
        input_cursor: "reply".len(),
        approval: Some(approval()),
        ..Default::default()
    };
    dump("S4 approval-overlay", &render_at(&s4, 60, 16)?);

    // S5: picker overlay over a one-line input.
    let s5 = MockTuiState {
        messages: messages(),
        input_text: "reply".into(),
        input_cursor: "reply".len(),
        picker: Some(picker()),
        ..Default::default()
    };
    dump("S5 picker-overlay", &render_at(&s5, 60, 16)?);
    Ok(())
}

/// Design falsifier (cyril-a14l design.md claim C2 mechanism): determine
/// whether `Paragraph::scroll((y, 0))` with `Wrap { trim: false }` offsets by
/// post-wrap VISUAL rows or pre-wrap LOGICAL lines. The cursor-follow design
/// is only correct under one of the two.
#[test]
fn falsifier_paragraph_scroll_semantics() -> anyhow::Result<()> {
    use ratatui::text::Line;
    use ratatui::widgets::{Paragraph, Wrap};

    // One 30-char logical line (wraps to 3 visual rows at width 10), then END.
    let long: String = "abcdefghij0123456789ABCDEFGHIJ".into();
    let paragraph = Paragraph::new(vec![Line::from(long), Line::from("END")])
        .wrap(Wrap { trim: false })
        .scroll((1, 0));
    let mut terminal = Terminal::new(TestBackend::new(10, 2))?;
    terminal.draw(|frame| frame.render_widget(paragraph, frame.area()))?;
    let buffer = terminal.backend().buffer().clone();
    let row0: String = (0..10)
        .map(|x| {
            buffer
                .cell((x, 0))
                .map_or(" ", ratatui::buffer::Cell::symbol)
        })
        .collect();
    println!("scroll(1) row0 = {row0:?}");
    println!(
        "verdict: {}",
        if row0.starts_with("0123456789") {
            "VISUAL rows (post-wrap)"
        } else if row0.starts_with("END") {
            "LOGICAL lines (pre-wrap)"
        } else {
            "NEITHER — investigate"
        }
    );
    Ok(())
}

/// Design falsifier (cyril-a14l claim C11): no state × size combination in the
/// adversarial matrix reaches the panic fallback ("Render error" banner) today.
#[test]
fn falsifier_no_fallback_size_sweep() -> anyhow::Result<()> {
    let draft = big_draft();
    let states = [
        MockTuiState::default(),
        MockTuiState {
            messages: messages(),
            input_text: draft.clone(),
            input_cursor: draft.len(),
            autocomplete_suggestions: file_suggestions(),
            autocomplete_selected: Some(7),
            ..Default::default()
        },
        MockTuiState {
            messages: messages(),
            input_text: "reply".into(),
            input_cursor: 5,
            approval: Some(approval()),
            ..Default::default()
        },
        MockTuiState {
            messages: messages(),
            picker: Some(picker()),
            ..Default::default()
        },
    ];
    let mut fallbacks = 0u32;
    for (index, state) in states.iter().enumerate() {
        for width in [1u16, 2, 5, 10, 20, 59, 60, 61, 80, 200] {
            for height in [1u16, 2, 3, 5, 8, 15, 16, 17, 24, 100] {
                let buffer = render_at(state, width, height)?;
                let text: String = buffer.content().iter().map(|c| c.symbol()).collect();
                if text.contains("Render error") {
                    println!("FALLBACK at state{index} {width}x{height}");
                    fallbacks += 1;
                }
            }
        }
    }
    println!("fallbacks: {fallbacks} / 400 renders");
    Ok(())
}
