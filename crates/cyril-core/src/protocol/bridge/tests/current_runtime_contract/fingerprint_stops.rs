//! Fingerprint fail-stop WIRING fences (cyril-6iek), reinstated for the SDK2
//! runtime (PR #115 review, finding 6). The detector's unit tests live in
//! `fingerprint`; these prove the runtime actually stops on a contradiction.

use super::*;

/// C3 bridge-level: a V2-bound loop on a KAS-shaped wire stops at
/// initialize — the loop task fails with a reason naming the evidence and
/// the remedy, and no command ever reaches the agent.
#[tokio::test]
async fn fingerprint_stops_v2_bound_on_kas_wire() {
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        |_sender, _rx, _permission_rx, _gate, loop_handle| async move {
            let error = tokio::time::timeout(Duration::from_secs(5), loop_handle)
                .await
                .expect_contract("fingerprint stop within 5s")
                .expect_contract("loop task joined")
                .expect_err_contract("a v2-bound loop on a KAS wire must fail-stop");
            let reason = error.to_string();
            assert!(
                reason.contains("_meta.kiro"),
                "reason names the evidence: {reason}"
            );
            assert!(
                reason.contains("KAS"),
                "reason names the detected engine: {reason}"
            );
            #[cfg(not(feature = "kas"))]
            assert!(
                reason.contains("--features kas"),
                "default build points at the rebuild: {reason}"
            );
            #[cfg(feature = "kas")]
            assert!(
                reason.contains("--agent-engine kas"),
                "kas build points at the flag: {reason}"
            );
            assert!(
                probe.borrow().received().is_empty(),
                "the command loop must never run after a fingerprint stop"
            );
        },
    )
    .await;
}

/// C7 bridge-level: initialize passes (v2-shaped) but `session/new` mints a
/// `sess_` id — the second layer stops the session from being announced.
#[tokio::test]
async fn fingerprint_stops_on_sess_id_v2_bound() {
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(false),
        sess_ids: Some(true),
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("sess-id fence NewSession send");
            match recv_notif(&mut rx, 5)
                .await
                .expect_contract("notification within 5s")
            {
                Notification::BridgeDisconnected { reason } => {
                    assert!(
                        reason.contains("session id"),
                        "reason names the id evidence: {reason}"
                    );
                }
                Notification::SessionCreated { session_id, .. } => {
                    panic!("session {session_id:?} announced despite a sess_-id contradiction")
                }
                other => panic!("unexpected notification: {other:?}"),
            }
        },
    )
    .await;
}

/// C7 load path, pre-flight: a caller-supplied KAS-shaped id under the V2
/// binding is refused BEFORE the RPC — the agent never sees `session/load`.
#[tokio::test]
async fn fingerprint_stops_on_sess_id_load_v2_bound() {
    let script = Rc::new(RefCell::new(Script::default()));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            sender
                .send(BridgeCommand::LoadSession {
                    session_id: crate::types::SessionId::new("sess_0123-reloaded"),
                })
                .await
                .expect_contract("load fence send");
            match recv_notif(&mut rx, 5)
                .await
                .expect_contract("notification within 5s")
            {
                Notification::BridgeDisconnected { reason } => {
                    assert!(
                        reason.contains("session id"),
                        "reason names the id evidence: {reason}"
                    );
                }
                other => panic!("unexpected notification: {other:?}"),
            }
            assert!(
                !probe
                    .borrow()
                    .received()
                    .iter()
                    .any(|entry| entry == "load_session"),
                "pre-flight refusal: the agent must never receive the load"
            );
        },
    )
    .await;
}

/// C8 kas lane: a Kas-bound loop whose agent passes the initialize
/// fingerprint but mints bare (v2-shaped) ids is stopped by the second layer.
#[cfg(feature = "kas")]
#[tokio::test]
async fn fingerprint_stops_kas_bound_on_uuid_id() {
    let script = Rc::new(RefCell::new(Script {
        wire_kas: Some(true),
        sess_ids: Some(false),
        ..Script::default()
    }));
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("kas-bound fence NewSession send");
            match recv_notif(&mut rx, 5)
                .await
                .expect_contract("notification within 5s")
            {
                Notification::BridgeDisconnected { reason } => {
                    assert!(
                        reason.contains("--agent-engine v2"),
                        "reason names the v2 remedy: {reason}"
                    );
                }
                other => panic!("unexpected notification: {other:?}"),
            }
        },
    )
    .await;
}
