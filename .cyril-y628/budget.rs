use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cyril_core::types::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, PermissionRequest, SessionId,
    ToolCall, ToolCallId, ToolCallStatus, ToolKind,
};
use cyril_ui::{
    state::UiState,
    theme::{resolve, ColorMode, ThemeId},
    traits::TuiState,
    widgets::approval,
};
use ratatui::{backend::TestBackend, Terminal};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let theme = resolve(ThemeId::CyrilDark, ColorMode::TrueColor);
    let (responder, _receiver) = tokio::sync::oneshot::channel();
    let mut state = UiState::new(500);
    state.show_approval(PermissionRequest {
        session_id: SessionId::new("sess_budget"),
        tool_call: ToolCall::new(
            ToolCallId::new("budget"),
            "Bounded editor".into(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            None,
        ),
        message: "Reject with feedback".into(),
        options: vec![PermissionOption {
            id: PermissionOptionId::new("reject"),
            label: "Reject".into(),
            kind: PermissionOptionKind::RejectOnce,
            is_destructive: true,
        }],
        trust_options: vec![],
        can_reject_with_reason: true,
        responder,
    });
    assert!(state.approval_begin_feedback());
    state.approval_feedback_paste(&"界\n".repeat(2048));
    let mut terminal = Terminal::new(TestBackend::new(100, 32))?;
    let started = Instant::now();
    for _ in 0..10_000 {
        let current = state.approval().ok_or("approval missing")?;
        terminal.draw(|frame| approval::render(frame, frame.area(), 27, current, None, &theme))?;
    }
    let drawing = started.elapsed();
    let started = Instant::now();
    for _ in 0..10_000 {
        state.approval_feedback_key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        state.approval_feedback_key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::CONTROL));
    }
    let editing = started.elapsed();
    println!("draws=10000 scalars=4096 geometry=100x32 draw_seconds={:.6} mean_draw_ms={:.6} edit_cycles=10000 edit_seconds={:.6}", drawing.as_secs_f64(), drawing.as_secs_f64()*1000.0/10000.0, editing.as_secs_f64());
    assert!(
        drawing.as_secs_f64() <= 10.0,
        "render budget exceeded: {drawing:?}"
    );
    assert!(
        (drawing + editing).as_secs_f64() <= 10.0,
        "combined bound exceeded"
    );
    Ok(())
}
