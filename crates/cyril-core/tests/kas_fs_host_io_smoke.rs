//! Gated live smoke (KAS-5a, cyril-7bdu): a real KAS turn drives
//! `fs/read_text_file` + `fs/write_text_file` callbacks that **cyril's** host-io
//! overrides serve (not the python probe) — the live oracle for the resolvers,
//! plus the C9 permission check.
//!
//! Manual-gated: needs `--features kas`, a **fresh** `kiro-cli login`, the
//! self-extracted KAS bundle, and `node`. NOTE: cyril's auth path (and the KAS
//! free path) read `~/.aws/sso/cache/kiro-auth-token.json`, NOT the sqlite store
//! — that file must be non-stale. The sqlite token refreshing while the SSO file
//! goes stale is the cyril-taba gap; if this test disconnects with "token
//! expired", refresh the SSO file even though `kiro-cli` looks authenticated.
//!
//! Oracle / assertions:
//! - READ resolver fired: the file's content surfaces in the agent's text.
//! - WRITE resolver fired: the written file lands on disk with the expected content.
//! - C9: the write traverses cyril's permission path; the read does not.
//!
//! Run: cargo test -p cyril-core --features kas --test kas_fs_host_io_smoke \
//!        -- --ignored --nocapture
#![cfg(feature = "kas")]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use cyril_core::protocol::bridge::spawn_bridge;
use cyril_core::types::*;
use tokio::sync::mpsc::Receiver;

#[tokio::test]
#[ignore = "live: needs --features kas, a fresh kiro-cli login (SSO token file), the KAS bundle, and node"]
async fn fs_read_write_served_by_cyril() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("magic.txt"), "the magic number is 4242\n").unwrap();
    let outfile = dir.path().join("summary.txt");

    // Free path: the bridge resolves the bundled `node + acp-server.js` argv, so
    // the agent_command is a placeholder (same as kas_freepath_smoke).
    let placeholder = AgentCommand::new("unused-for-kas-free-path");
    let bridge = spawn_bridge(
        placeholder,
        AgentEngine::Kas,
        KasSpawn::Free,
        dir.path().to_path_buf(),
    )
    .expect("spawn_bridge");
    let (sender, mut notif_rx, mut perm_rx) = bridge.split();

    // Auto-approve permissions; record tool-call titles so we can verify the WRITE
    // was gated (C9). KAS auto-allows reads, so no read permission is expected.
    let gated: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let gated_w = gated.clone();
    let approver = tokio::spawn(async move {
        while let Some(req) = perm_rx.recv().await {
            gated_w
                .lock()
                .unwrap()
                .push(req.tool_call.title().to_string());
            let _ = req.responder.send(PermissionResponse::AllowOnce);
        }
    });

    sender
        .send(BridgeCommand::NewSession {
            cwd: dir.path().to_path_buf(),
        })
        .await
        .expect("send NewSession");
    let session_id = recv_session(&mut notif_rx).await;

    sender
        .send(BridgeCommand::SendPrompt {
            session_id,
            content_blocks: vec![
                "Using your tools, do BOTH, one tool call at a time: \
                 1) read the file magic.txt and tell me the magic number it contains; \
                 2) write a file summary.txt whose entire contents are exactly: done-4242"
                    .into(),
            ],
        })
        .await
        .expect("send SendPrompt");

    // Drive to TurnCompleted, accumulating agent text; fail loud on disconnect.
    let mut agent_text = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(240);
    loop {
        let routed = tokio::time::timeout_at(deadline, notif_rx.recv())
            .await
            .expect("a TurnCompleted within 240s")
            .expect("notification channel open");
        match routed.notification {
            Notification::AgentMessage(m) => agent_text.push_str(&m.text),
            Notification::TurnCompleted { .. } => break,
            Notification::BridgeDisconnected { reason } => {
                panic!("KAS fs turn disconnected (auth/precondition?): {reason}")
            }
            _ => {}
        }
    }
    approver.abort();

    // Oracle: cyril's WRITE resolver served the callback -> file on disk.
    assert!(
        outfile.exists(),
        "cyril's write resolver must have created summary.txt"
    );
    assert!(
        std::fs::read_to_string(&outfile)
            .unwrap()
            .contains("done-4242"),
        "summary.txt must contain the written content"
    );
    // Oracle: cyril's READ resolver served the callback -> content reached the agent.
    assert!(
        agent_text.contains("4242"),
        "agent never reported the read content; got: {agent_text:?}"
    );
    // C9: the write traversed cyril's permission path; the read did not.
    let gated = gated.lock().unwrap();
    assert!(
        gated
            .iter()
            .any(|t| t.to_lowercase().contains("write") || t.contains("summary.txt")),
        "the write must be gated by a permission request; gated titles: {gated:?}"
    );
    assert!(
        !gated.iter().any(|t| t.contains("magic.txt")),
        "the read must NOT raise a permission; gated titles: {gated:?}"
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
