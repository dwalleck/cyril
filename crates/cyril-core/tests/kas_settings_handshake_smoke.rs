//! Gated live smoke (cyril-nhzw): a real KAS turn driven through cyril's bridge
//! with the settings handshake ON, verifying the two behavioral claims:
//!
//! - **Claim 11 (removed-invariant guard):** enabling the marshaled `_meta.kiro.
//!   settings` (thinking, toolSearch, subagentOrchestration, …) must NOT hard-fail
//!   the turn. A newly-enabled flag that makes KAS emit a typed `session/update`
//!   variant acp 0.11.2 can't deserialize would abort mid-turn (no `#[serde(other)]`
//!   catch-all) → the turn never reaches `TurnCompleted` / the bridge disconnects.
//!   Passing this proves the enabled posture is safe end-to-end.
//! - **Claim 10:** `subagentOrchestration` (default-on) makes the agent invoke the
//!   DAG `orchestrate_subagent` tool — observed as a `ToolCall` titled "Orchestrate
//!   Sub-agent" (the shape the prove-it probe captured). Model-mediated: if the
//!   agent answers without delegating, re-run (a strong delegation instruction
//!   reliably triggers it — see the probe). The deterministic CI fence for cyril's
//!   OUTBOUND wire is the unit test `settings::tests::marshal_live_fixture`.
//!
//! Manual-gated: needs `--features kas`, a **fresh** `kiro-cli login` (SSO token
//! file), the self-extracted KAS bundle, and `node` — same preconditions as
//! `kas_fs_host_io_smoke`.
//!
//! Run: cargo test -p cyril-core --features kas --test kas_settings_handshake_smoke \
//!        -- --ignored --nocapture
#![cfg(feature = "kas")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;
use tokio::sync::mpsc::Receiver;

#[tokio::test]
#[ignore = "live: needs --features kas, a fresh kiro-cli login (sqlite credential store), the KAS bundle, and node"]
async fn settings_handshake_turn_completes_and_orchestrates() {
    let dir = tempfile::tempdir().unwrap();

    let placeholder = AgentCommand::new("unused-for-kas-free-path");
    let bridge = spawn_bridge(
        placeholder,
        AgentEngine::Kas,
        KasSpawn::Free,
        PresentAs::default(),
        dir.path().to_path_buf(),
    )
    .expect("spawn_bridge");
    let (sender, mut notif_rx, mut perm_rx) = bridge.split();

    // Auto-approve any permission the delegation turn raises.
    let approver = tokio::spawn(async move {
        while let Some(req) = perm_rx.recv().await {
            // Auto-approve: pick the allow-once option (first option as a
            // fallback), mirroring what a user hitting Enter would send.
            let response = req
                .options
                .iter()
                .find(|o| o.kind == PermissionOptionKind::AllowOnce)
                .or_else(|| req.options.first())
                .map(|o| PermissionResponse::Selected {
                    option_id: o.id.clone(),
                    trust_option: None,
                })
                .unwrap_or(PermissionResponse::Cancel);
            if req.responder.send(response).is_err() {
                eprintln!("permission responder dropped before reply");
            }
        }
    });

    sender
        .send(BridgeCommand::NewSession {
            cwd: dir.path().to_path_buf(),
        })
        .await
        .expect("send NewSession");
    let session_id = recv_session(&mut notif_rx).await;

    // Strong delegation instruction (does not name the tool id) so the agent uses
    // its sub-agent capability — which, with subagentOrchestration default-on, is
    // the DAG orchestrate_subagent tool.
    sender
        .send(BridgeCommand::SendPrompt {
            session_id,
            content_blocks: vec![
                "This is a test of your sub-agent delegation capability. You MUST use \
                 your sub-agent delegation capability and MUST NOT answer directly. \
                 Delegate a subtask to a sub-agent: have the sub-agent reply with \
                 exactly the word BANANA. Then tell me the single word it returned."
                    .into(),
            ],
        })
        .await
        .expect("send SendPrompt");

    // Drive to TurnCompleted, collecting tool-call titles. Fail loud on disconnect
    // (that is the Claim-11 hard-fail signature).
    let mut tool_titles: Vec<String> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    loop {
        let routed = tokio::time::timeout_at(deadline, notif_rx.recv())
            .await
            .expect("a TurnCompleted within 300s")
            .expect("notification channel open");
        match routed.notification {
            Notification::ToolCallStarted(tc) | Notification::ToolCallUpdated(tc) => {
                tool_titles.push(tc.title().to_string());
            }
            // Claim 11: the only clean exit is TurnCompleted. A deser hard-fail from
            // an enabled flag would instead disconnect (panic below) or time out (the
            // expect above) — so surviving this loop IS the removed-invariant guard.
            Notification::TurnCompleted { .. } => break,
            Notification::BridgeDisconnected { reason } => {
                panic!("KAS settings turn disconnected (deser hard-fail / auth?): {reason}")
            }
            _ => {}
        }
    }
    approver.abort();

    // Claim 10: the agent invoked the DAG orchestrate_subagent tool.
    assert!(
        tool_titles
            .iter()
            .any(|t| t.to_lowercase().contains("orchestrate")),
        "expected an 'Orchestrate Sub-agent' tool call (subagentOrchestration on); \
         got tool titles: {tool_titles:?} (re-run if the agent answered directly)"
    );
}

/// Drain until the first `SessionCreated`; panic on disconnect or 30s timeout.
async fn recv_session(rx: &mut Receiver<RoutedNotification>) -> SessionId {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    loop {
        let routed = tokio::time::timeout_at(deadline, rx.recv())
            .await
            .expect("SessionCreated within 30s")
            .expect("notification channel open");
        match routed.notification {
            Notification::SessionCreated { session_id, .. } => return session_id,
            Notification::BridgeDisconnected { reason } => {
                panic!("bridge disconnected before session: {reason}")
            }
            _ => {}
        }
    }
}
