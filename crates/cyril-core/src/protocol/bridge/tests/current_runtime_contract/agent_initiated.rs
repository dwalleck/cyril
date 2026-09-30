//! cyril-lki9 transport fences: agent-initiated turn frames through the real
//! SDK2 transport and the bridge's inbound path.
//!
//! The converter and the engine hook have unit fences. What they cannot show is
//! that the agent-initiated tag survives ACP deserialization, the SDK's
//! `session/update` handler and the mediator's inbound conversion, and is
//! attached to the SAME frame it came on — the only place the App's per-turn
//! announcement can be derived from.

use super::routing::message;
use super::*;

/// Drive one prompt whose single chunk carries `chunk_meta`, and return the
/// `origin` the App-facing receiver saw on that chunk.
#[cfg(feature = "kas")]
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
#[cfg(feature = "kas")]
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

/// Inject `frames` in order, then an untagged main-session SENTINEL, and return
/// a compact label for everything the App receiver saw up to it.
async fn announce_trace(frames: Vec<RoutedNotification>) -> Vec<String> {
    let script = Rc::new(RefCell::new(Script::default()));
    let probe = Rc::clone(&script);
    let out: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let seen = Rc::clone(&out);
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            let main = start_session(&sender, &mut rx).await;
            let inbound = probe
                .borrow()
                .inbound
                .clone()
                .expect_contract("lki9 inbound");
            for mut frame in frames {
                if frame
                    .session_id
                    .as_ref()
                    .map(crate::types::SessionId::as_str)
                    == Some("MAIN")
                {
                    frame.session_id = Some(main.clone());
                }
                inbound.send(frame).await.expect_contract("lki9 C10 inject");
            }
            inbound
                .send(RoutedNotification::scoped(
                    main.clone(),
                    message("SENTINEL"),
                ))
                .await
                .expect_contract("lki9 C10 sentinel");
            loop {
                let routed = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                    .await
                    .expect_contract("lki9 C10 recv timeout")
                    .expect_contract("lki9 C10 channel open");
                let who = if routed.session_id.as_ref() == Some(&main) {
                    "main".to_owned()
                } else {
                    routed
                        .session_id
                        .as_ref()
                        .map(ToString::to_string)
                        .unwrap_or_default()
                };
                let label = match &routed.notification {
                    Notification::AgentInitiatedTurn(origin) => {
                        format!("{who}:ANNOUNCE({})", origin.reason().unwrap_or("<none>"))
                    }
                    Notification::AgentMessage(m) if m.text == "SENTINEL" => break,
                    Notification::AgentMessage(m) => format!("{who}:{}", m.text),
                    Notification::TurnStarted => format!("{who}:START"),
                    Notification::TurnCompleted { .. } => format!("{who}:END"),
                    other => format!("{who}:{other:?}"),
                };
                seen.borrow_mut().push(label);
            }
        },
    )
    .await;
    out.borrow().clone()
}

fn tagged(session: &str, text: &str, reason: &str) -> RoutedNotification {
    RoutedNotification::scoped(crate::types::SessionId::new(session), message(text))
        .with_origin(crate::types::AgentInitiation::new(Some(reason.to_owned())))
}

fn plain(session: &str, text: &str) -> RoutedNotification {
    RoutedNotification::scoped(crate::types::SessionId::new(session), message(text))
}

fn start(session: &str) -> RoutedNotification {
    RoutedNotification::scoped(
        crate::types::SessionId::new(session),
        Notification::TurnStarted,
    )
}

fn end(session: &str) -> RoutedNotification {
    RoutedNotification::scoped(
        crate::types::SessionId::new(session),
        Notification::TurnCompleted {
            stop_reason: StopReason::EndTurn,
        },
    )
}

/// C10: exactly one announcement per session per turn, immediately before the
/// turn's first agent-initiated frame — through the real inbound path.
/// Stress: the tag first appears on a turn's 3rd frame; 57 tagged frames in
/// one turn; the next turn re-arms; a turn missing its START re-arms off the
/// previous END, and a turn whose previous END was missed re-arms off its own
/// START (each reset fenced alone); an untagged turn announces nothing; a
/// woken step session interleaving with main gets its own single announcement.
#[tokio::test]
async fn announce_once_per_turn() {
    let wake = "workflow-complete-wake";
    let mut frames = vec![start("MAIN"), plain("MAIN", "p1"), plain("MAIN", "p2")];
    frames.extend((0..57).map(|i| tagged("MAIN", &format!("t{i}"), wake)));
    frames.push(end("MAIN"));
    // Next wake: re-armed.
    frames.extend([start("MAIN"), tagged("MAIN", "w2", wake), end("MAIN")]);
    // A turn whose turn_start never arrived still announces: re-armed by the
    // previous turn's END alone (fences the turn-end reset, mutation M10).
    frames.extend([tagged("MAIN", "x1", wake), end("MAIN")]);
    // A turn whose previous turn's END was never seen still announces:
    // re-armed by its START alone (fences the turn-start reset, M10b).
    frames.extend([
        start("MAIN"),
        tagged("MAIN", "y1", wake),
        start("MAIN"),
        tagged("MAIN", "y2", wake),
        end("MAIN"),
    ]);
    // An ordinary untagged turn: no announcement.
    frames.extend([start("MAIN"), plain("MAIN", "u1"), end("MAIN")]);
    // Main and a woken step interleave.
    frames.extend([
        start("MAIN"),
        start("child-7"),
        tagged("child-7", "s1", "send-message-wake"),
        tagged("MAIN", "m1", wake),
        tagged("child-7", "s2", "send-message-wake"),
        tagged("MAIN", "m2", wake),
        end("child-7"),
        end("MAIN"),
    ]);
    let got = announce_trace(frames).await;

    let mut want: Vec<String> = vec!["main:START".into(), "main:p1".into(), "main:p2".into()];
    want.push(format!("main:ANNOUNCE({wake})"));
    want.extend((0..57).map(|i| format!("main:t{i}")));
    want.push("main:END".into());
    want.extend([
        "main:START".into(),
        format!("main:ANNOUNCE({wake})"),
        "main:w2".into(),
        "main:END".into(),
        format!("main:ANNOUNCE({wake})"),
        "main:x1".into(),
        // (x1's END: no turn is active — the start never arrived — so the
        // mediator drops it as unowned; the announce re-arm still happened in
        // `observe` before that disposition, which is what M10 removes.)
        "main:START".into(),
        format!("main:ANNOUNCE({wake})"),
        "main:y1".into(),
        "main:START".into(),
        format!("main:ANNOUNCE({wake})"),
        "main:y2".into(),
        "main:END".into(),
        "main:START".into(),
        "main:u1".into(),
        "main:END".into(),
        "main:START".into(),
        "child-7:START".into(),
        "child-7:ANNOUNCE(send-message-wake)".into(),
        "child-7:s1".into(),
        format!("main:ANNOUNCE({wake})"),
        "main:m1".into(),
        "child-7:s2".into(),
        "main:m2".into(),
        "child-7:END".into(),
        "main:END".into(),
    ]);
    assert_eq!(got, want, "C10 announcement placement and count");
}
