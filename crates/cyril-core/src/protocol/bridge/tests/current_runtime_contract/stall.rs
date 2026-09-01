//! Stall-watchdog WIRING fences (cyril-14ou), reinstated for the SDK2
//! runtime (PR #115 review, finding 6). The pure unit half lives in
//! `turn_liveness`; these prove the loop arms feed it correctly.

use super::*;

/// cyril-14ou C1 (loop level): a parked turn produces exactly one
/// `TurnStalled`, scoped to the stalled turn's session, once the threshold
/// elapses — and only one per quiet period.
#[tokio::test(start_paused = true)]
async fn stall_emits_at_threshold() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, gate, _loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: session_id.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(vec!["park".to_owned()], None),
                })
                .await
                .expect_contract("stall fence prompt send");

            let routed = tokio::time::timeout(Duration::from_secs(60), rx.recv())
                .await
                .expect_contract("stall within 60 virtual seconds")
                .expect_contract("stall fence channel open");
            match &routed.notification {
                Notification::TurnStalled { quiet } => {
                    assert!(
                        *quiet >= DEFAULT_STALL_THRESHOLD,
                        "reported quiet {quiet:?} below threshold"
                    );
                    assert_eq!(
                        routed.session_id.as_ref(),
                        Some(&session_id),
                        "stall must be scoped to the stalled turn's session"
                    );
                }
                other => panic!("expected TurnStalled, got {other:?}"),
            }

            assert!(
                recv_notif(&mut rx, 120).await.is_none(),
                "a second TurnStalled fired for the same quiet period"
            );

            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
        },
    )
    .await;
}

/// cyril-14ou C4 (loop level): frames scoped to a FOREIGN session do not
/// feed the main turn's liveness clock — the stall still fires while foreign
/// chatter streams every 5 virtual seconds.
#[tokio::test(start_paused = true)]
async fn foreign_traffic_does_not_mask_stall() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, gate, _loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["park".to_owned()], None),
                })
                .await
                .expect_contract("foreign-chatter prompt send");

            let inbound = probe
                .borrow()
                .inbound
                .clone()
                .expect_contract("harness exposes the inbound seam");
            let injector = tokio::task::spawn_local(async move {
                loop {
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    if inbound
                        .send(RoutedNotification::scoped(
                            crate::types::SessionId::new("sess_foreign"),
                            Notification::AgentMessage(crate::types::AgentMessage {
                                text: "chatter".to_owned(),
                                is_streaming: true,
                            }),
                        ))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            });

            let mut saw_stall = false;
            for _ in 0..40 {
                match recv_notif(&mut rx, 20).await {
                    Some(Notification::TurnStalled { .. }) => {
                        saw_stall = true;
                        break;
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            injector.abort();
            assert!(
                saw_stall,
                "foreign-session traffic masked the main turn's stall (C4)"
            );

            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
        },
    )
    .await;
}

/// cyril-14ou C5 (loop level): with no turn in flight the tick arm is gated
/// off — long idle stretches emit nothing.
#[tokio::test(start_paused = true)]
async fn no_stall_without_active_turn() {
    let script = Rc::new(RefCell::new(Script::default()));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            let _session_id = start_session(&sender, &mut rx).await;
            tokio::time::sleep(Duration::from_secs(90)).await;
            assert!(
                !matches!(
                    recv_notif(&mut rx, 30).await,
                    Some(Notification::TurnStalled { .. })
                ),
                "idle bridge emitted a phantom stall (C5)"
            );
        },
    )
    .await;
}
