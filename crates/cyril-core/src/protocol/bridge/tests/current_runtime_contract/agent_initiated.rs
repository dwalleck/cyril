//! cyril-lki9 transport fences: agent-initiated turn frames through the real
//! SDK2 transport and the bridge's inbound path.
//!
//! The converter and the engine hook have unit fences. What they cannot show is
//! that the agent-initiated tag survives ACP deserialization, the SDK's
//! `session/update` handler and the mediator's inbound conversion, and is
//! attached to the SAME frame it came on — the only place the App's per-turn
//! announcement can be derived from.

use super::*;

/// Drive one prompt whose single chunk carries `chunk_meta`, and return the
/// `origin` the App-facing receiver saw on that chunk.
async fn chunk_origin(
    chunk_meta: Option<serde_json::Value>,
) -> Option<crate::types::AgentInitiation> {
    let script = Rc::new(RefCell::new(Script {
        emit_chunks: 1,
        chunk_meta,
        wire_kas: Some(true),
        ..Script::default()
    }));
    let seen: Rc<RefCell<Option<Option<crate::types::AgentInitiation>>>> =
        Rc::new(RefCell::new(None));
    let out = Rc::clone(&seen);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("lki9 C3 prompt send");
            for _ in 0..10 {
                let routed = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                    .await
                    .expect_contract("lki9 C3 chunk timeout")
                    .expect_contract("lki9 C3 channel open");
                if let Notification::AgentMessage(message) = &routed.notification {
                    assert_eq!(message.text, "c0", "the probe chunk, not another frame");
                    *out.borrow_mut() = Some(routed.origin.clone());
                    return;
                }
            }
            panic!("lki9 C3: the chunk never reached the App receiver");
        },
    )
    .await;
    let result = seen.borrow().clone();
    result.expect_contract("lki9 C3 chunk observed")
}

/// C3: a tagged chunk reaches the App with `origin` set from ITS OWN `_meta`;
/// the untagged control chunk (the historical shape) arrives with `None`.
#[tokio::test]
async fn origin_is_stamped_on_the_frame_it_came_on() {
    let tagged = chunk_origin(Some(serde_json::json!({
        "kiro": { "agentInitiated": true, "agentInitiatedReason": "workflow-complete-wake" }
    })))
    .await;
    assert_eq!(
        tagged
            .as_ref()
            .and_then(crate::types::AgentInitiation::reason),
        Some("workflow-complete-wake"),
        "tagged chunk must carry its agent-initiated origin, got {tagged:?}"
    );
    let untagged = chunk_origin(None).await;
    assert_eq!(untagged, None, "an untagged chunk must carry no origin");
}

/// Inject a wire-shaped frame (post-conversion, the path live frames take into
/// the loop) and wait until the App receiver has seen it — proof the mediator
/// processed it before the test's next command.
async fn inject_and_see(
    script: &Rc<RefCell<Script>>,
    rx: &mut mpsc::Receiver<RoutedNotification>,
    routed: RoutedNotification,
) {
    let inbound = script
        .borrow()
        .inbound
        .clone()
        .expect_contract("lki9 harness exposes the inbound sender");
    let want = std::mem::discriminant(&routed.notification);
    inbound.send(routed).await.expect_contract("lki9 inject");
    for _ in 0..10 {
        let seen = recv_notif(rx, 5)
            .await
            .expect_contract("lki9 injected frame reaches the App");
        if std::mem::discriminant(&seen) == want {
            return;
        }
    }
    panic!("lki9: injected frame never reached the App receiver");
}

/// Which session a `CancelRequest` reached, with or without a server turn in
/// flight on the FIRST session while the loop's main has moved to a second.
async fn cancel_target(with_server_turn: bool) -> Vec<String> {
    let script = Rc::new(RefCell::new(Script::default()));
    let probe = Rc::clone(&script);
    with_harness(
        Rc::clone(&script),
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            let first = start_session(&sender, &mut rx).await;
            if with_server_turn {
                inject_and_see(
                    &probe,
                    &mut rx,
                    RoutedNotification::scoped(first.clone(), Notification::TurnStarted),
                )
                .await;
            }
            let _second = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::CancelRequest)
                .await
                .expect_contract("lki9 C12 cancel send");
            assert!(
                wait_for_received(&probe, "cancel", 5).await,
                "lki9 C12: the agent never received session/cancel"
            );
        },
    )
    .await;
    let ledger = std::sync::Arc::clone(&script.borrow().cancelled_sessions);
    let sessions = ledger.lock().map(|v| v.clone());
    sessions.expect_contract("cancelled_sessions lock")
}

/// C12: Esc during a server turn cancels THAT turn's session (the turn's
/// snapshot), not whatever the loop's main session has become. Positive
/// control: with no turn in flight the cancel falls back to the current main —
/// so the two answers differ and the assertion is decisive.
#[tokio::test]
async fn cancel_targets_server_turn() {
    assert_eq!(
        cancel_target(true).await,
        ["fake-0"],
        "server turn on fake-0 must be the cancel target"
    );
    assert_eq!(
        cancel_target(false).await,
        ["fake-1"],
        "control: with no turn, cancel falls back to the current main session"
    );
}

/// C22: a silent server turn raises `TurnStalled` scoped to the main session
/// once the threshold elapses; after its `turn_end`, silence raises nothing.
#[tokio::test(start_paused = true)]
async fn stall_fires_during_server_turn() {
    let script = Rc::new(RefCell::new(Script::default()));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            let main = start_session(&sender, &mut rx).await;
            inject_and_see(
                &probe,
                &mut rx,
                RoutedNotification::scoped(main.clone(), Notification::TurnStarted),
            )
            .await;
            let routed = tokio::time::timeout(Duration::from_secs(60), rx.recv())
                .await
                .expect_contract("lki9 C22 stall within 60 virtual seconds")
                .expect_contract("lki9 C22 channel open");
            match &routed.notification {
                Notification::TurnStalled { quiet } => {
                    assert!(
                        *quiet >= DEFAULT_STALL_THRESHOLD,
                        "quiet {quiet:?} below threshold"
                    );
                    assert_eq!(
                        routed.session_id.as_ref(),
                        Some(&main),
                        "stall scoped to main"
                    );
                }
                other => panic!(
                    "lki9 C22: expected TurnStalled during a silent server turn, got {other:?}"
                ),
            }
            inject_and_see(
                &probe,
                &mut rx,
                RoutedNotification::scoped(
                    main.clone(),
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    },
                ),
            )
            .await;
            assert!(
                recv_notif(&mut rx, 120).await.is_none(),
                "lki9 C22: no stall may fire after the server turn ended"
            );
        },
    )
    .await;
}
