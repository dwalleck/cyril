use cyril_core::types::{
    PermissionOption, PermissionOptionId, PermissionOptionKind, PermissionRequest,
    PermissionResponse, SessionId, ToolCall, ToolCallId, ToolCallStatus, ToolKind,
};
use cyril_ui::state::UiState;
use cyril_ui::traits::TuiState;
use tokio::sync::oneshot;

fn request(label: &str) -> (PermissionRequest, oneshot::Receiver<PermissionResponse>) {
    let (responder, receiver) = oneshot::channel();
    let request = PermissionRequest {
        session_id: SessionId::new("main"),
        tool_call: ToolCall::new(
            ToolCallId::new(label),
            label.to_owned(),
            ToolKind::Execute,
            ToolCallStatus::Pending,
            None,
        ),
        message: label.to_owned(),
        options: vec![PermissionOption {
            id: PermissionOptionId::new("allow"),
            label: "Allow".to_owned(),
            kind: PermissionOptionKind::AllowOnce,
            is_destructive: false,
        }],
        trust_options: Vec::new(),
        responder,
    };
    (request, receiver)
}

fn receiver_state(receiver: &mut oneshot::Receiver<PermissionResponse>) -> &'static str {
    match receiver.try_recv() {
        Ok(PermissionResponse::Selected { .. }) => "selected",
        Ok(PermissionResponse::Cancel) => "cancelled",
        Err(oneshot::error::TryRecvError::Empty) => "pending",
        Err(oneshot::error::TryRecvError::Closed) => "closed",
    }
}

fn main() {
    let (first, mut first_rx) = request("first");
    let (second, mut second_rx) = request("second");
    let mut state = UiState::new(8);
    state.show_approval(first);
    state.show_approval(second);

    println!(
        "head1={}",
        TuiState::approval(&state).map_or("none", |a| &a.message)
    );
    state.approval_confirm();
    println!("first_after_resolution={}", receiver_state(&mut first_rx));
    println!("second_after_resolution={}", receiver_state(&mut second_rx));
    println!(
        "head2={}",
        TuiState::approval(&state).map_or("none", |a| &a.message)
    );
}
