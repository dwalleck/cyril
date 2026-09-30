//! cyril-lki9 P3 probe (THROWAWAY — copied into
//! crates/cyril-core/src/protocol/bridge/tests/current_runtime_contract/ only for
//! the run, then deleted; this copy is the record).
//!
//! Question: with NO cyril-owned turn in flight (no SendPrompt), does the real
//! bridge run_loop + TurnMediator forward a wake turn's wire `turn_end` (an
//! unstamped, session-scoped `TurnCompleted`) to the App, or drop it?
//!
//! Sequence injected through the harness InboundProbe (post-conversion, the same
//! path live frames take into run_loop's notification arm), mirroring the
//! captured wake turn (tail capture: chunks … turn_end on the main session):
//!   1. scoped(main, AgentMessage{"WAKE-CHUNK", streaming})
//!   2. scoped(main, TurnCompleted{EndTurn})            <- the wake's wire turn_end
//!   3. scoped(main, AgentMessage{"SENTINEL", streaming}) <- proves 2 was processed
//! Output: the ordered list of notification kinds the App receiver saw.
use super::*;

#[tokio::test]
async fn probe_lki9_p3_unowned_wake_turn_end() {
    let script = Rc::new(RefCell::new(Script::default()));
    let injection = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, _gate, loop_handle| async move {
            let main = start_session(&sender, &mut rx).await;
            let inbound = injection
                .borrow()
                .inbound
                .clone()
                .expect_contract("inbound");
            let chunk = |t: &str| {
                Notification::AgentMessage(crate::types::AgentMessage {
                    text: t.to_owned(),
                    is_streaming: true,
                })
            };
            for n in [
                RoutedNotification::scoped(main.clone(), chunk("WAKE-CHUNK")),
                RoutedNotification::scoped(
                    main.clone(),
                    Notification::TurnCompleted {
                        stop_reason: StopReason::EndTurn,
                    },
                ),
                RoutedNotification::scoped(main.clone(), chunk("SENTINEL")),
            ] {
                inbound.send(n).await.expect_contract("inject");
            }
            let mut seen = Vec::new();
            loop {
                let r = tokio::time::timeout(Duration::from_secs(5), rx.recv())
                    .await
                    .expect_contract("recv timeout")
                    .expect_contract("recv channel");
                let label = match &r.notification {
                    Notification::AgentMessage(m) => format!("AgentMessage({})", m.text),
                    Notification::TurnCompleted { stop_reason } => {
                        format!("TurnCompleted({stop_reason:?})")
                    }
                    other => format!("other:{}", other_name(other)),
                };
                let done = label == "AgentMessage(SENTINEL)";
                seen.push(label);
                if done {
                    break;
                }
            }
            println!("LKI9-P3 APP-RECEIVED: {seen:?}");
            sender
                .send(BridgeCommand::Shutdown)
                .await
                .expect_contract("shutdown");
            loop_handle
                .await
                .expect_contract("join")
                .expect_contract("loop result");
        },
    )
    .await;
}

fn other_name(n: &Notification) -> String {
    format!("{n:?}").chars().take(40).collect()
}
