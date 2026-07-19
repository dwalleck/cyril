//! Gated live smoke (KAS-1 Part B, claims C1-wrapper + C10-live): a KAS turn via
//! the WRAPPER spawn (`kiro-cli acp --agent-engine <flag>`, `--auth=acp-callback`)
//! completes — which means cyril's `_kiro/auth/getAccessToken` responder fired
//! and returned a token KAS accepted. A wrong responder method-string, a missing
//! `profileArn`, or a stale token would surface here as a `BridgeDisconnected`
//! (the turn could not authenticate), failing the test loudly.
//!
//! Manual-gated: needs `--features kas`, a prior `kiro-cli login` (fresh token),
//! and kiro-cli >= 2.7.1. Run once per identity to cover C10-live (social +
//! AWS-IdP); AWS Builder-ID is unit-only (spec SC2, accepted).
//!
//! Run: cargo test -p cyril-core --features kas --test kas_wrapper_smoke \
//!        -- --ignored --nocapture
#![cfg(feature = "kas")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;
use tokio::sync::mpsc::Receiver;

#[tokio::test]
#[ignore = "live: needs --features kas, a fresh kiro-cli login, kiro-cli >= 2.7.1"]
async fn wrapper_turn_completes_with_auth_responder() {
    // The wrapper spawns `<program> acp --agent-engine <flag>`; the program comes
    // from the bound agent command (default kiro-cli).
    let agent_command = AgentCommand::new("kiro-cli").with_args(vec!["acp".to_string()]);
    let cwd = std::env::temp_dir();
    let bridge = spawn_bridge(
        agent_command,
        AgentEngine::Kas,
        KasSpawn::Wrapper,
        PresentAs::default(),
        cwd.clone(),
    )
    .expect("spawn_bridge");
    let (sender, mut notif_rx, _perm_rx) = bridge.split();

    sender
        .send(BridgeCommand::NewSession { cwd })
        .await
        .expect("send NewSession");
    let session_id = recv_session(&mut notif_rx).await;

    sender
        .send(BridgeCommand::SendPrompt {
            session_id,
            content_blocks: vec![
                "Reply with exactly the text KAS_SMOKE_OK and nothing else. Do not use any tools."
                    .into(),
            ],
        })
        .await
        .expect("send SendPrompt");

    let deadline = tokio::time::Instant::now() + Duration::from_secs(180);
    // Accumulate agent text: TurnCompleted alone is NOT proof of an authenticated
    // turn — a prompt-level error can collapse into TurnCompleted (cyril-l7tw) —
    // so the fence also demands the echoed sentinel. (`KAS_SMOKE_OK`, not "ok":
    // "ok" is a substring of "TokenInvalidError"'s "token".)
    let mut text = String::new();
    loop {
        let routed = tokio::time::timeout_at(deadline, notif_rx.recv())
            .await
            .expect("a TurnCompleted within 180s")
            .expect("notification channel open");
        match routed.notification {
            Notification::AgentMessage(m) => text.push_str(&m.text),
            Notification::TurnCompleted { .. } => {
                assert!(
                    text.contains("KAS_SMOKE_OK"),
                    "turn completed WITHOUT the echoed sentinel — an error turn \
                     collapsed into TurnCompleted (cyril-l7tw)? agent text: {text:?}"
                );
                return;
            }
            Notification::BridgeDisconnected { reason } => {
                panic!("wrapper KAS turn disconnected (auth responder?): {reason}")
            }
            _ => {}
        }
    }
}

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
