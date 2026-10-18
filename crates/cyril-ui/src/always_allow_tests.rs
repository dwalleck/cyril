//! cyril-auxt: the approval overlay's KAS "Always allow" scope page and
//! folder editor (claims C6/C7 in `.cyril-auxt/design.md`). Each test drives
//! `UiState` the way the App's key routing does and asserts what reaches the
//! request's responder.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use cyril_core::always_allow::{AlwaysAllowGrant, GrantResource, GrantScope, ScopeChoice};
use cyril_core::review::consent::PermissionConsent;
use cyril_core::types::{
    Notification, PermissionOption, PermissionOptionId, PermissionOptionKind, PermissionRequest,
    PermissionResponse, SessionId, ToolCall, ToolCallId, ToolCallStatus, ToolKind, TrustOption,
};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::oneshot::{Receiver, error::TryRecvError};

use crate::state::UiState;
use crate::traits::{ApprovalPhase, TuiState};

fn kas_options() -> Vec<PermissionOption> {
    [
        ("accept", "Allow", PermissionOptionKind::AllowOnce),
        (
            "always-accept",
            "Always allow",
            PermissionOptionKind::AllowAlways,
        ),
        ("reject", "Deny", PermissionOptionKind::RejectOnce),
        (
            "always-reject",
            "Always deny",
            PermissionOptionKind::RejectAlways,
        ),
    ]
    .into_iter()
    .map(|(id, label, kind)| PermissionOption {
        id: PermissionOptionId::new(id),
        label: label.into(),
        kind,
        is_destructive: false,
    })
    .collect()
}

fn shell_consent() -> PermissionConsent {
    PermissionConsent::new(
        Some("shell"),
        Some("echo AUXTV1 now".into()),
        Some("/ws".into()),
        Some("run_command".into()),
        Some("echo AUXTV1 now".into()),
        Vec::new(),
    )
    .with_triggering_resource(Some("echo AUXTV1 now".into()))
}

fn write_consent() -> PermissionConsent {
    PermissionConsent::new(
        Some("fs_write"),
        Some("src/a/x.txt".into()),
        Some("/ws".into()),
        Some("fs_write".into()),
        None,
        Vec::new(),
    )
}

fn request(
    session: &str,
    consent: Option<PermissionConsent>,
    trust_options: Vec<TrustOption>,
) -> (PermissionRequest, Receiver<PermissionResponse>) {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let tool_call = ToolCall::new(
        ToolCallId::new("tc_1"),
        "echo AUXTV1 now".into(),
        ToolKind::Execute,
        ToolCallStatus::Pending,
        None,
    );
    let request = PermissionRequest {
        session_id: SessionId::new(session),
        tool_call,
        message: "echo AUXTV1 now".into(),
        options: kas_options(),
        trust_options,
        can_reject_with_reason: true,
        responder: tx,
        consent,
    };
    (request, rx)
}

fn with_main(main: &str) -> UiState {
    let mut state = UiState::new(500);
    state.apply_notification(&Notification::SessionCreated {
        session_id: SessionId::new(main),
        current_mode: None,
        current_model: None,
        available_modes: Vec::new(),
        available_models: Vec::new(),
    });
    state
}

/// Show `request`, move to "Always allow" and press Enter.
fn open_scope_page(state: &mut UiState, request: PermissionRequest) {
    state.show_approval(request);
    state.approval_select_next();
    assert!(state.approval_confirm().is_none());
}

fn phase(state: &UiState) -> ApprovalPhase {
    state.approval().expect("approval still open").phase.clone()
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn grant(scope: GrantScope, resource: GrantResource) -> AlwaysAllowGrant {
    AlwaysAllowGrant::new(scope, resource)
}

fn assert_unanswered(rx: &mut Receiver<PermissionResponse>) {
    assert!(
        matches!(rx.try_recv(), Err(TryRecvError::Empty)),
        "the request must still be waiting for an answer"
    );
}

fn always_allow(response: PermissionResponse) -> AlwaysAllowGrant {
    match response {
        PermissionResponse::AlwaysAllow { option_id, grant } => {
            assert_eq!(option_id.as_str(), "always-accept");
            grant
        }
        other => panic!("expected an Always-allow grant, got {other:?}"),
    }
}

#[test]
fn allow_always_on_a_kas_request_opens_the_scope_page() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, request);

    assert_unanswered(&mut rx);
    assert!(matches!(
        phase(&state),
        ApprovalPhase::SelectScope {
            scope: GrantScope::Session,
            owned: true,
            ..
        }
    ));
    let approval = state.approval().expect("open");
    assert_eq!(
        approval.scope_choices(),
        vec![
            ScopeChoice::Exact("echo AUXTV1 now".into()),
            ScopeChoice::Pattern("echo *".into()),
            ScopeChoice::WholeTool,
        ]
    );
}

#[test]
fn exact_row_at_user_scope_answers_with_the_grant() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_cycle_scope();
    state.approval_cycle_scope();
    assert!(state.approval_confirm().is_none());

    assert!(!state.has_approval());
    assert_eq!(
        always_allow(rx.try_recv().expect("answered")),
        grant(
            GrantScope::User,
            GrantResource::Exact("echo AUXTV1 now".into())
        )
    );
}

#[test]
fn scope_cycle_keeps_the_highlighted_row_and_wraps() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_select_next();
    state.approval_select_next();
    state.approval_select_next(); // clamps on the last row: whole tool
    for _ in 0..3 {
        state.approval_cycle_scope(); // session → workspace → user → session
    }
    state.approval_confirm();

    assert_eq!(
        always_allow(rx.try_recv().expect("answered")),
        grant(GrantScope::Session, GrantResource::WholeTool)
    );
}

#[test]
fn scope_cycle_follows_the_highlighted_row_when_rows_change() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_select_next();
    state.approval_select_next(); // session rows: exact, folder, whole → whole (2)
    state.approval_cycle_scope(); // workspace: exact, folder, whole
    state.approval_cycle_scope(); // user: exact, whole → whole is now row 1
    assert_eq!(state.approval().expect("open").selected, 1);
    state.approval_confirm();

    assert_eq!(
        always_allow(rx.try_recv().expect("answered")),
        grant(GrantScope::User, GrantResource::WholeTool)
    );
}

#[test]
fn scope_cycle_from_the_folder_row_returns_to_the_first_row() {
    let mut state = with_main("main");
    let (request, _rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_select_next(); // folder row
    state.approval_cycle_scope(); // its label names the scope, so it resets
    assert_eq!(state.approval().expect("open").selected, 0);
}

#[test]
fn next_queued_scope_page_opens_at_the_last_scope() {
    let mut state = with_main("main");
    let (first, _first_rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, first);
    let (second, _second_rx) = queued("main", "tc_2", shell_command("cargo build"));
    state.show_approval(second); // waiting while the first is answered
    state.approval_cycle_scope();
    state.approval_cycle_scope(); // user
    state.approval_confirm(); // exact grant: the second stays queued

    state.approval_select_next();
    state.approval_confirm(); // second's Always allow
    assert!(matches!(
        phase(&state),
        ApprovalPhase::SelectScope {
            scope: GrantScope::User,
            ..
        }
    ));
}

#[test]
fn scope_resets_to_session_once_the_queue_drains() {
    let mut state = with_main("main");
    let (first, _first_rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, first);
    state.approval_cycle_scope();
    state.approval_cycle_scope(); // user
    state.approval_confirm();
    assert!(!state.has_approval(), "queue drained");

    let (second, _second_rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, second);
    assert!(matches!(
        phase(&state),
        ApprovalPhase::SelectScope {
            scope: GrantScope::Session,
            ..
        }
    ));
}

/// A queued request for `session` with its own tool call id.
fn queued(
    session: &str,
    tool_call: &str,
    consent: PermissionConsent,
) -> (PermissionRequest, Receiver<PermissionResponse>) {
    let (mut request, rx) = request(session, Some(consent), Vec::new());
    request.tool_call = ToolCall::new(
        ToolCallId::new(tool_call),
        "queued".into(),
        ToolKind::Execute,
        ToolCallStatus::Pending,
        None,
    );
    (request, rx)
}

fn shell_command(command: &str) -> PermissionConsent {
    PermissionConsent::new(
        Some("shell"),
        Some(command.into()),
        Some("/ws".into()),
        Some("run_command".into()),
        Some(command.into()),
        Vec::new(),
    )
    .with_triggering_resource(Some(command.into()))
}

fn answered_once(rx: &mut Receiver<PermissionResponse>) {
    match rx.try_recv() {
        Ok(PermissionResponse::Selected {
            option_id,
            trust_option: None,
        }) => assert_eq!(option_id.as_str(), "accept"),
        other => panic!("expected the queued request answered allow_once, got {other:?}"),
    }
}

#[test]
fn whole_tool_grant_settles_queued_requests_of_that_capability() {
    let mut state = with_main("main");
    let (granted, mut granted_rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, granted);
    let (same, mut same_rx) = queued("main", "tc_2", shell_command("cargo build"));
    let (write, mut write_rx) = queued("main", "tc_3", write_consent());
    let (step, mut step_rx) = queued("step-1", "tc_4", shell_command("cargo build"));
    state.show_approval(same);
    state.show_approval(write);
    state.show_approval(step);
    for _ in 0..2 {
        state.approval_select_next(); // whole tool
    }
    state.approval_confirm();

    assert_eq!(
        always_allow(granted_rx.try_recv().expect("granted")),
        grant(GrantScope::Session, GrantResource::WholeTool)
    );
    answered_once(&mut same_rx);
    assert_unanswered(&mut write_rx);
    assert_unanswered(&mut step_rx);
    let waiting = state
        .approval()
        .expect("the uncovered requests stay queued");
    assert_eq!(waiting.tool_call.id().as_str(), "tc_3", "queue order kept");
}

#[test]
fn exact_grant_on_the_main_session_settles_nothing() {
    let mut state = with_main("main");
    let (granted, _granted_rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, granted);
    let (same, mut same_rx) = queued("main", "tc_2", shell_consent());
    state.show_approval(same);
    state.approval_confirm(); // exact row

    assert_unanswered(&mut same_rx);
    assert!(state.has_approval());
}

#[test]
fn another_sessions_grant_settles_its_own_matching_requests() {
    let mut state = with_main("main");
    let (granted, _granted_rx) = request("step-1", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, granted);
    let (same, mut same_rx) = queued("step-1", "tc_2", shell_consent());
    let (other_step, mut other_step_rx) = queued("step-2", "tc_3", shell_consent());
    let (main, mut main_rx) = queued("main", "tc_4", shell_consent());
    state.show_approval(same);
    state.show_approval(other_step);
    state.show_approval(main);
    state.approval_confirm(); // exact row, session scope

    answered_once(&mut same_rx);
    assert_unanswered(&mut other_step_rx);
    assert_unanswered(&mut main_rx);
}

#[test]
fn esc_on_the_scope_page_returns_to_the_options() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_cancel();

    let approval = state.approval().expect("still open");
    assert_eq!(approval.phase, ApprovalPhase::SelectOption);
    assert_eq!(approval.selected, 1, "cursor back on Always allow");
    assert_unanswered(&mut rx);
}

#[test]
fn folder_row_edits_and_grants_a_folder_glob() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_cycle_scope(); // workspace: exact, folder, whole
    state.approval_select_next();
    state.approval_confirm();

    assert!(state.approval_folder_active());
    match phase(&state) {
        ApprovalPhase::EnterFolder { editor, scope, .. } => {
            assert_eq!(editor.text(), "src/a");
            assert_eq!(scope, GrantScope::Workspace);
        }
        other => panic!("expected the folder editor, got {other:?}"),
    }
    state.approval_folder_key(key(KeyCode::Backspace));
    state.approval_folder_key(key(KeyCode::Backspace));
    state.approval_folder_key(key(KeyCode::Enter));

    let sent = always_allow(rx.try_recv().expect("answered"));
    assert_eq!(sent.scope(), GrantScope::Workspace);
    assert_eq!(sent.resource().wire(), "src/**");
    assert!(!state.has_approval());
}

#[test]
fn invalid_folder_keeps_the_editor_and_esc_returns_to_the_rows() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_select_next(); // session: exact, folder, whole
    state.approval_confirm();
    for _ in 0.."src/a".len() {
        state.approval_folder_key(key(KeyCode::Backspace));
    }
    assert!(state.approval_folder_paste("lib"));
    state.approval_folder_key(key(KeyCode::Enter));

    match phase(&state) {
        ApprovalPhase::EnterFolder {
            editor, attempted, ..
        } => {
            assert_eq!(editor.text(), "lib");
            assert!(attempted, "the reason now renders as an error");
        }
        other => panic!("expected the folder editor, got {other:?}"),
    }
    assert_unanswered(&mut rx);

    state.approval_folder_key(key(KeyCode::Esc));
    let approval = state.approval().expect("still open");
    assert!(matches!(approval.phase, ApprovalPhase::SelectScope { .. }));
    assert!(matches!(
        approval.scope_choices().get(approval.selected),
        Some(ScopeChoice::Folder(_))
    ));
}

#[test]
fn another_sessions_file_grant_is_answered_once() {
    let mut state = with_main("main");
    let (request, mut rx) = request("step-1", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    assert!(matches!(
        phase(&state),
        ApprovalPhase::SelectScope { owned: false, .. }
    ));
    assert_eq!(
        state.approval().expect("open").scope_choices(),
        vec![
            ScopeChoice::Exact("src/a/x.txt".into()),
            ScopeChoice::WholeTool
        ],
        "no folder row for another session"
    );
    state.approval_confirm();

    match rx.try_recv().expect("answered") {
        PermissionResponse::Selected { option_id, .. } => assert_eq!(option_id.as_str(), "accept"),
        other => panic!("expected allow_once, got {other:?}"),
    }
}

#[test]
fn without_kas_consent_always_allow_answers_directly() {
    let mut state = with_main("main");
    let (request, mut rx) = request("main", None, Vec::new());
    open_scope_page(&mut state, request);

    match rx.try_recv().expect("answered") {
        PermissionResponse::Selected {
            option_id,
            trust_option: None,
        } => assert_eq!(option_id.as_str(), "always-accept"),
        other => panic!("expected the plain pick, got {other:?}"),
    }
}

#[test]
fn v2_trust_tiers_keep_their_own_page() {
    let mut state = with_main("main");
    let tier = TrustOption {
        label: "Full command".into(),
        display: "echo AUXTV1 now".into(),
        setting_key: "allowedCommands".into(),
        patterns: vec!["echo AUXTV1 now".into()],
    };
    let (request, _rx) = request("main", Some(shell_consent()), vec![tier]);
    open_scope_page(&mut state, request);
    assert!(matches!(phase(&state), ApprovalPhase::SelectTrust { .. }));
}

fn screen(state: &UiState) -> anyhow::Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
    terminal.draw(|frame| crate::render::draw(frame, state as &dyn TuiState))?;
    let buffer = terminal.backend().buffer().clone();
    let width = usize::from(buffer.area.width);
    Ok(buffer
        .content()
        .chunks(width)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n"))
}

#[test]
fn scope_page_renders_the_tui_rows() -> anyhow::Result<()> {
    let mut state = with_main("main");
    let (request, _rx) = request("main", Some(shell_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_cycle_scope();
    let text = screen(&state)?;

    for expected in [
        "shell → echo AUXTV1 now",
        "Scope:   session   [workspace]   user",
        "Trust \"echo AUXTV1 now\"",
        "exact match · workspace",
        "Trust \"echo *\"",
        "pattern · workspace",
        "Trust entire tool (run_command)",
        "s cycle scope",
    ] {
        assert!(text.contains(expected), "missing {expected:?} in\n{text}");
    }
    Ok(())
}

#[test]
fn folder_page_renders_the_live_validation() -> anyhow::Result<()> {
    let mut state = with_main("main");
    let (request, _rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    state.approval_select_next();
    let text = screen(&state)?;
    assert!(
        text.contains("Allow a folder for this session…") && text.contains("src/a · session"),
        "folder row in\n{text}"
    );

    state.approval_confirm();
    let text = screen(&state)?;
    assert!(
        text.contains("Allows writes in src/a and every folder under it, for this session only."),
        "valid folder line in\n{text}"
    );
    for _ in 0.."src/a".len() {
        state.approval_folder_key(key(KeyCode::Backspace));
    }
    let text = screen(&state)?;
    assert!(text.contains("Type a folder path."), "reason in\n{text}");
    Ok(())
}

#[test]
fn tiny_viewport_does_not_panic() -> anyhow::Result<()> {
    let mut state = with_main("main");
    let (request, _rx) = request("main", Some(write_consent()), Vec::new());
    open_scope_page(&mut state, request);
    for (width, height) in [(20, 6), (60, 10), (8, 3)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| crate::render::draw(frame, &state as &dyn TuiState))?;
    }
    state.approval_select_next();
    state.approval_confirm();
    for (width, height) in [(20, 6), (60, 10), (8, 3)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| crate::render::draw(frame, &state as &dyn TuiState))?;
    }
    Ok(())
}

/// The pages an "Always allow" pick leads to, for the cyril-vojx sweep.
#[derive(Debug, Clone, Copy)]
enum LaterPage {
    /// KAS scope rows.
    Scope,
    /// KAS folder editor, prefilled with a valid folder.
    Folder,
    /// KAS folder editor whose validation line wraps at 80 columns.
    FolderLong,
    /// v2 trust tiers.
    Trust,
    /// v2 trust tiers, the selected one below a tier whose pattern list is
    /// wider than the popup.
    TrustLong,
}

/// A folder whose "Allows writes in …" line is wider than the 80-column
/// popup's 74 content columns.
const LONG_FOLDER: &str = "crates/cyril-ui/src/widgets/a-rather-long-directory-name";

/// A fresh request opened on `page` at the default 80x24 frame.
fn open_later_page(page: LaterPage) -> (UiState, Receiver<PermissionResponse>) {
    let mut state = with_main("main");
    let rx = match page {
        LaterPage::Scope => {
            let (request, rx) = request("main", Some(shell_consent()), Vec::new());
            open_scope_page(&mut state, request);
            rx
        }
        LaterPage::Folder => {
            let (request, rx) = request("main", Some(write_consent()), Vec::new());
            open_scope_page(&mut state, request);
            state.approval_select_next(); // session rows: exact, folder, whole
            state.approval_confirm();
            assert!(state.approval_folder_active(), "folder editor must open");
            rx
        }
        LaterPage::FolderLong => {
            let consent = PermissionConsent::new(
                Some("fs_write"),
                Some(format!("{LONG_FOLDER}/x.txt")),
                Some("/ws".into()),
                Some("fs_write".into()),
                None,
                Vec::new(),
            );
            let (request, rx) = request("main", Some(consent), Vec::new());
            open_scope_page(&mut state, request);
            state.approval_select_next(); // session rows: exact, folder, whole
            state.approval_confirm();
            match phase(&state) {
                ApprovalPhase::EnterFolder { editor, .. } => {
                    assert_eq!(editor.text(), LONG_FOLDER, "prefill")
                }
                other => panic!("expected the folder editor, got {other:?}"),
            }
            rx
        }
        LaterPage::TrustLong => {
            let long = TrustOption {
                label: "Every listed command".into(),
                display: (0..20)
                    .map(|n| format!("cargo test --package crate-{n}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                setting_key: "allowedCommands".into(),
                patterns: vec!["cargo test *".into()],
            };
            let short = TrustOption {
                label: "Full command".into(),
                display: "echo AUXTV1 now".into(),
                setting_key: "allowedCommands".into(),
                patterns: vec!["echo AUXTV1 now".into()],
            };
            let (request, rx) = request("main", None, vec![long, short]);
            open_scope_page(&mut state, request);
            state.approval_select_next(); // the short tier, below the long one
            assert!(matches!(phase(&state), ApprovalPhase::SelectTrust { .. }));
            rx
        }
        LaterPage::Trust => {
            let tier = TrustOption {
                label: "Full command".into(),
                display: "echo AUXTV1 now".into(),
                setting_key: "allowedCommands".into(),
                patterns: vec!["echo AUXTV1 now".into()],
            };
            let (request, rx) = request("main", None, vec![tier]);
            open_scope_page(&mut state, request);
            assert!(matches!(phase(&state), ApprovalPhase::SelectTrust { .. }));
            rx
        }
    };
    (state, rx)
}

/// Record an 80x`height` frame in the state, as the App's draw path does,
/// and return that frame's text.
fn screen_at(state: &mut UiState, height: u16) -> anyhow::Result<String> {
    state.set_terminal_size(80, height);
    let mut terminal = Terminal::new(TestBackend::new(80, height))?;
    terminal.draw(|frame| crate::render::draw(frame, &*state as &dyn TuiState))?;
    let buffer = terminal.backend().buffer().clone();
    Ok(buffer
        .content()
        .chunks(80)
        .map(|row| row.iter().map(|cell| cell.symbol()).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// REGRESSION FENCE (cyril-vojx): the pages after an "Always allow" pick
/// answer Enter only while their selection (or, on the folder page, the
/// folder editor) is drawn — at every frame height, including a popup
/// clamped to its borders (inner height 0) and no popup at all.
///
/// The long cases are the review's counterexamples: a folder long enough to
/// wrap the validation line, and a trust tier whose pattern list is wider
/// than the popup, above the selected tier. Either used to push the editor
/// (or the selection) below the popup while Enter still answered.
#[rstest::rstest]
#[case::scope_page(LaterPage::Scope)]
#[case::folder_editor(LaterPage::Folder)]
#[case::folder_editor_long_folder(LaterPage::FolderLong)]
#[case::trust_page(LaterPage::Trust)]
#[case::trust_page_long_pattern_list(LaterPage::TrustLong)]
fn later_pages_confirm_only_what_is_drawn(#[case] page: LaterPage) -> anyhow::Result<()> {
    let (mut inert, mut answered_at) = (0, 0);
    for height in 4u16..=22 {
        let (mut state, mut rx) = open_later_page(page);
        let text = screen_at(&mut state, height)?;
        let drawn = match page {
            LaterPage::Folder => text.contains("src/a█"),
            LaterPage::FolderLong => text.contains(&format!("{LONG_FOLDER}█")),
            LaterPage::Scope | LaterPage::Trust | LaterPage::TrustLong => text.contains('▸'),
        };
        state.approval_confirm();
        let answered = rx.try_recv().is_ok();
        assert_eq!(
            answered, drawn,
            "height={height} page={page:?}: Enter must act exactly when drawn\n{text}"
        );
        inert += usize::from(!drawn);
        answered_at += usize::from(answered);
    }
    anyhow::ensure!(
        inert > 0 && answered_at > 0,
        "the sweep must reach both a hidden and a drawn page: inert={inert} answered={answered_at}"
    );
    Ok(())
}
