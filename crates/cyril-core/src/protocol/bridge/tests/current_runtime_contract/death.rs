//! Agent-death ordering fences (cyril-l7tw), reinstated for the SDK2 runtime
//! (PR #115 review, finding 6). Death is delivered by aborting the in-process
//! SDK runtime task — the same io_done signal a real agent EOF produces.

use super::*;

/// l7tw C1/C5 via the real death mechanism: the agent dies while the prompt
/// is PARKED, so the error arm is reached by the rpc layer failing its
/// pending responses — never by an agent reply.
#[tokio::test]
async fn death_mid_turn_emits_bridge_error_before_turn_completed() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(V2Engine),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("death fence prompt send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt reached the agent before the kill"
            );
            kill.kill();
            let mut saw_bridge_error = false;
            loop {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::BridgeError { operation, message }) => {
                        assert_eq!(operation, "prompt");
                        assert!(!message.is_empty(), "error message must not be empty");
                        saw_bridge_error = true;
                    }
                    Some(Notification::TurnCompleted { stop_reason }) => {
                        assert_eq!(stop_reason, StopReason::EndTurn);
                        assert!(
                            saw_bridge_error,
                            "BridgeError must arrive before TurnCompleted on agent death"
                        );
                        break;
                    }
                    Some(_) => {}
                    None => panic!("no TurnCompleted within 5s of agent death"),
                }
            }
        },
    )
    .await;
}

/// l7tw C4: mid-turn death tells the whole story in order — BridgeError →
/// TurnCompleted → BridgeDisconnected — the loop exits, and later sends fail
/// at the sender instead of silently vanishing.
#[tokio::test]
async fn death_mid_turn_disconnect_after_completion() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(V2Engine),
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle, kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("death order prompt send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt reached the agent before the kill"
            );
            kill.kill();
            let mut order = Vec::new();
            loop {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::BridgeError { .. }) => order.push("error"),
                    Some(Notification::TurnCompleted { .. }) => order.push("completed"),
                    Some(Notification::BridgeDisconnected { .. }) => {
                        order.push("disconnected");
                        break;
                    }
                    Some(_) => {}
                    None => {
                        panic!("no BridgeDisconnected within 5s of mid-turn death; saw {order:?}")
                    }
                }
            }
            assert_eq!(
                order,
                ["error", "completed", "disconnected"],
                "mid-turn death tells the whole story in order"
            );
            let loop_result = tokio::time::timeout(Duration::from_secs(5), loop_handle)
                .await
                .expect_contract("run loop must exit after mid-turn death");
            assert!(loop_result.is_ok(), "loop task completed cleanly");
            let send_result = sender
                .send(BridgeCommand::SendPrompt {
                    session_id: crate::types::SessionId::new("s-dead"),
                    prompt: crate::types::PromptEnvelope::prepared(vec!["hello?".to_owned()], None),
                })
                .await;
            assert!(
                send_result.is_err(),
                "sends after disconnect must error, not silently vanish"
            );
        },
    )
    .await;
}

/// l7tw C2 on the death path: the killed turn ends with exactly ONE
/// `TurnCompleted` — the BridgeError adds visibility without disturbing the
/// single-terminal-marker invariant.
#[tokio::test]
async fn death_mid_turn_single_turn_completed() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(V2Engine),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("single-completion prompt send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt reached the agent before the kill"
            );
            kill.kill();
            let mut completions = 0;
            while let Some(notification) = recv_notif(&mut rx, 2).await {
                if matches!(notification, Notification::TurnCompleted { .. }) {
                    completions += 1;
                }
            }
            assert_eq!(completions, 1, "exactly one TurnCompleted after death");
        },
    )
    .await;
}

/// l7tw C4/C13 (the KAS wrinkle): a dual-completion turn whose wire
/// `turn_end` lands BEFORE the agent dies — the watchdog then finds no turn
/// in flight and takes the idle path; expected exactly one TurnCompleted
/// (pre-kill) and exactly one BridgeDisconnected.
#[cfg(feature = "kas")]
#[tokio::test]
async fn death_after_turn_end_single_disconnect() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        emit_turn_end: true,
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("dual-terminal prompt send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt reached the agent before the kill"
            );
            // The wire turn_end was emitted before parking; settle the dual
            // completion before killing.
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
            kill.kill();
            let mut completions = 0;
            let mut disconnects = 0;
            while let Some(notification) = recv_notif(&mut rx, 2).await {
                match notification {
                    Notification::TurnCompleted { .. } => completions += 1,
                    Notification::BridgeDisconnected { .. } => disconnects += 1,
                    _ => {}
                }
            }
            assert_eq!(
                completions, 0,
                "the dual turn completed exactly once (pre-kill)"
            );
            assert_eq!(
                disconnects, 1,
                "death after turn end disconnects exactly once"
            );
        },
    )
    .await;
}

/// Slice-8 stress fixture: only the DYING owner's terminal satisfies its
/// deferred disconnect — a foreign session's terminal in the deferred window
/// must not emit BridgeDisconnected before the owner's error/completed pair.
#[cfg(feature = "kas")]
#[tokio::test]
async fn foreign_terminal_does_not_satisfy_deferred_disconnect() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        emit_turn_end: true,
        turn_end_session: Some("sess_foreign".to_owned()),
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, kill| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("deferred-disconnect prompt send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt reached the agent before the kill"
            );
            kill.kill();
            let mut order = Vec::new();
            loop {
                match recv_notif(&mut rx, 5).await {
                    Some(Notification::BridgeError { .. }) => order.push("error"),
                    Some(Notification::TurnCompleted { .. }) => order.push("completed"),
                    Some(Notification::BridgeDisconnected { .. }) => {
                        order.push("disconnected");
                        break;
                    }
                    Some(_) => {}
                    None => panic!("no BridgeDisconnected within 5s; saw {order:?}"),
                }
            }
            let first_disconnect = order.iter().position(|step| *step == "disconnected");
            let owner_error = order.iter().position(|step| *step == "error");
            assert!(
                owner_error < first_disconnect,
                "the dying owner's BridgeError precedes the disconnect; saw {order:?}"
            );
            assert_eq!(
                order.last().copied(),
                Some("disconnected"),
                "disconnect is last; saw {order:?}"
            );
        },
    )
    .await;
}
