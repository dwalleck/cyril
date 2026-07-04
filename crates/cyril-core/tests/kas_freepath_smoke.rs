//! Gated live smoke (KAS-1 Part A, claim C1): a free-path KAS turn runs to
//! completion THROUGH cyril's bridge — not the raw probe harness.
//!
//! Manual-gated (design caveat): needs `--features kas`, a prior `kiro-cli
//! login` (populates the sqlite credential store), the self-extracted KAS
//! bundle, and `node`. Oracle parity with prove-it-prototype (cyril-dcc6): the
//! bridge spawns the identical `--auth=acp-callback` argv kiro-cli itself uses
//! (unit-fenced by `discovery::argv_matches_kiro_cli_own_spawn`), and cyril's
//! sqlite-backed `_kiro/auth/getAccessToken` responder authenticates the turn
//! — the probe proved this end-to-end twice (C14b). This smoke asserts the
//! user-observable end: a `TurnCompleted` with no `BridgeDisconnected`.
//!
//! Run: cargo test -p cyril-core --features kas --test kas_freepath_smoke \
//!        -- --ignored --nocapture
#![cfg(feature = "kas")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;
use tokio::sync::mpsc::Receiver;

#[tokio::test]
#[ignore = "live: needs --features kas, kiro-cli login, the KAS bundle, and node"]
async fn freepath_turn_completes_through_bridge() {
    // For the KAS free path the `agent_command` param is ignored — the bridge
    // resolves the bundled `node + acp-server.js` argv via discovery — so a
    // placeholder is fine here.
    let placeholder = AgentCommand::new("unused-for-kas-free-path");
    let cwd = std::env::temp_dir();
    let bridge = spawn_bridge(placeholder, AgentEngine::Kas, KasSpawn::Free, cwd.clone())
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
            content_blocks: vec!["Reply with exactly the text KAS_SMOKE_OK and nothing else. Do not use any tools.".into()],
        })
        .await
        .expect("send SendPrompt");

    // Drive to TurnCompleted; fail loudly on BridgeDisconnected (a missing
    // precondition or an auth failure) or a 180s timeout.
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
                panic!("free-path KAS turn disconnected: {reason}")
            }
            _ => {} // streamed text / metadata — keep draining
        }
    }
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
