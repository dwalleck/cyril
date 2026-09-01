//! Mid-turn command liveness and shutdown/cancel WIRING fences, reinstated
//! for the SDK2 runtime (PR #115 review, findings 1/2/6). These are the
//! fences the mediator-loop deadlock fix is accountable to: a parked prompt
//! must never stop the loop from serving commands.

use super::*;

/// C1 (the deadlock headline): with the prompt parked, a ListSettings sent
/// mid-turn is answered BEFORE TurnCompleted — the loop returned to recv()
/// instead of blocking on the prompt RPC.
#[tokio::test]
async fn loop_frees_during_turn() {
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
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("loop-frees prompt send");
            sender
                .send(BridgeCommand::ListSettings)
                .await
                .expect_contract("loop-frees settings send");
            let first = recv_notif(&mut rx, 5)
                .await
                .expect_contract("a mid-turn notification before turn end");
            assert!(
                matches!(&first, Notification::SettingsList { .. })
                    || matches!(&first, Notification::BridgeError { operation, .. } if operation == "settings/list"),
                "expected mid-turn ListSettings result before TurnCompleted, got {first:?}"
            );
            gate.notify_one();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
        },
    )
    .await;
}

/// C2: a SteerSession sent while the prompt is parked reaches the agent
/// (`_session/steer`) BEFORE the turn completes.
#[tokio::test]
async fn steer_reaches_agent_mid_turn() {
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
                    session_id: session_id.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("steer fence prompt send");
            sender
                .send(BridgeCommand::SteerSession {
                    session_id,
                    message: "stop".to_owned(),
                })
                .await
                .expect_contract("steer fence steer send");
            assert!(
                wait_for_received(&probe, "ext:session/steer", 5).await,
                "steer reached the agent mid-turn; received = {:?}",
                probe.borrow().received()
            );
            gate.notify_one();
            drain_to_turn(&mut rx).await;
        },
    )
    .await;
}

/// C3: a CancelRequest mid-turn reaches the agent and the parked prompt
/// resolves to Cancelled — one TurnCompleted{Cancelled}, no hang.
#[tokio::test]
async fn cancel_resolves_busy_turn() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["forever".to_owned()],
                        None,
                    ),
                })
                .await
                .expect_contract("cancel fence prompt send");
            sender
                .send(BridgeCommand::CancelRequest)
                .await
                .expect_contract("cancel fence cancel send");
            assert!(
                wait_for_received(&probe, "cancel", 5).await,
                "agent never received session/cancel; received = {:?}",
                probe.borrow().received()
            );
            assert_eq!(
                drain_to_turn(&mut rx).await,
                StopReason::Cancelled,
                "cancel resolved the parked turn as Cancelled"
            );
            assert!(
                probe
                    .borrow()
                    .received()
                    .iter()
                    .any(|entry| entry == "cancel"),
                "agent received the cancel mid-turn"
            );
        },
    )
    .await;
}

/// C7: Shutdown received while a turn is parked aborts the prompt task and
/// the loop returns promptly even though the prompt never completes.
#[tokio::test]
async fn shutdown_aborts_inflight_prompt() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let session_id = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(
                        vec!["forever".to_owned()],
                        None,
                    ),
                })
                .await
                .expect_contract("shutdown fence prompt send");
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("shutdown fence shutdown send");
            let returned = tokio::time::timeout(Duration::from_secs(2), loop_handle).await;
            assert!(
                matches!(returned, Ok(Ok(Ok(())))),
                "run loop returned cleanly after a mid-turn Shutdown, got {returned:?}"
            );
        },
    )
    .await;
}

/// Shutdown aborts EVERY live prompt task: a wire-released turn leaves its
/// prompt task parked; a second turn parks too. After Shutdown, nothing may
/// emit — a surviving task would complete after the loop exited.
#[cfg(feature = "kas")]
#[tokio::test]
async fn shutdown_aborts_every_live_prompt_task() {
    let script = Rc::new(RefCell::new(Script {
        emit_turn_end: true,
        block_prompt: true,
        wire_kas: Some(true),
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_engine_harness(
        Rc::new(crate::protocol::engine::KasEngine::default()),
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle, _kill| async move {
            let session_id = start_session(&sender, &mut rx).await;

            // A: released by the wire turn_end; its prompt task stays parked.
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: session_id.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(vec!["A".to_owned()], None),
                })
                .await
                .expect_contract("live-tasks fence prompt A send");
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);

            // B: accepted (A released) and also parks — two live tasks.
            let live_flag = probe
                .borrow()
                .emit_turn_end_live
                .clone()
                .expect_contract("harness exposes the live turn_end flag");
            live_flag.store(false, std::sync::atomic::Ordering::Release);
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id,
                    prompt: crate::types::PromptEnvelope::prepared(vec!["B".to_owned()], None),
                })
                .await
                .expect_contract("live-tasks fence prompt B send");
            assert!(
                wait_for_received(&probe, "prompt", 5).await,
                "prompt B reached the agent"
            );

            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("live-tasks fence shutdown send");

            while let Some(notification) = recv_notif(&mut rx, 2).await {
                assert!(
                    !matches!(notification, Notification::TurnCompleted { .. }),
                    "no completion may arrive after shutdown — a task outlived the loop"
                );
            }
        },
    )
    .await;
}
