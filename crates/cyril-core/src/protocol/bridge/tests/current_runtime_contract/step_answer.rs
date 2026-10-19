//! cyril-9104 C7: answering a parked workflow step sends `session/prompt` to
//! the STEP session and stays outside cyril's own turn mediation.

use super::*;
use crate::types::SessionId;

/// The answer reaches the agent as a prompt to the step session while the
/// main session's own prompt is still in flight, and a later main prompt is
/// still accepted: the answer never occupies the busy guard.
#[tokio::test]
async fn step_answer_prompts_the_step_session_outside_the_main_turn() {
    let script = Rc::new(RefCell::new(Script {
        block_prompt: true,
        ..Script::default()
    }));
    let probe = Rc::clone(&script);
    with_harness(
        script,
        move |sender, mut rx, _permission_rx, gate, _loop_handle| async move {
            let main = start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::AnswerWorkflowStep {
                    session_id: SessionId::new("sess_step"),
                    answer: "Edit src/lib.rs.".to_owned(),
                })
                .await
                .expect_contract("answer send");
            sender
                .send(BridgeCommand::SendPrompt {
                    session_id: main.clone(),
                    prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
                })
                .await
                .expect_contract("main prompt send");
            // A mid-turn command answers before any turn ends: had the main
            // prompt been refused as busy, its BridgeError would come first.
            sender
                .send(BridgeCommand::ListSettings)
                .await
                .expect_contract("settings send");
            let first = recv_notif(&mut rx, 5)
                .await
                .expect_contract("a notification before any turn end");
            assert!(
                matches!(&first, Notification::SettingsList { .. })
                    || matches!(&first, Notification::BridgeError { operation, .. } if operation == "settings/list"),
                "the main prompt must be accepted while the answer is in flight, got {first:?}"
            );
            assert_eq!(
                probe.borrow().prompted().clone(),
                [
                    ("sess_step".to_owned(), "Edit src/lib.rs.".to_owned()),
                    (main.as_str().to_owned(), "go".to_owned()),
                ],
                "one prompt to the step session, then the main prompt"
            );
            gate.notify_waiters();
            assert_eq!(drain_to_turn(&mut rx).await, StopReason::EndTurn);
        },
    )
    .await;
}

/// A step answer the agent refuses surfaces as a visible bridge error.
#[tokio::test]
async fn failed_step_answer_is_a_bridge_error() {
    let script = Rc::new(RefCell::new(Script {
        prompt_err: true,
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permission_rx, _gate, _loop_handle| async move {
            start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::AnswerWorkflowStep {
                    session_id: SessionId::new("sess_step"),
                    answer: "yes".to_owned(),
                })
                .await
                .expect_contract("answer send");
            let error = recv_notif(&mut rx, 5)
                .await
                .expect_contract("the failure is reported");
            assert!(
                matches!(&error, Notification::BridgeError { operation, .. } if operation == "answer workflow step"),
                "got {error:?}"
            );
        },
    )
    .await;
}
