//! cyril-k3lz C8/C9: `BridgeCommand::SetThinking` wire requests and ack
//! mapping through the real domain mediator and SDK runtime, against the
//! in-process fake agent. Oracles: the 2.24.0 captures (v2 `reasoning`
//! response `v2-reasoning-args2` line 29; KAS set_config_option results
//! `kas-new-surface-noprompt-0668` lines 21/23) and the spec B2 rules.

use super::*;
use crate::types::ThinkingLever;

const V2_FIXTURE: &str =
    include_str!("../../../../../tests/fixtures/v2/thinking/reasoning-args2-2.24.0.json");
const KAS_FIXTURE: &str =
    include_str!("../../../../../tests/fixtures/kas/thinking/set-config-sequence-0668.json");

/// The captured `result` of the `reasoning {thinkingEnabled:false}` request.
fn captured_reasoning_ack() -> serde_json::Value {
    let fixture: serde_json::Value = serde_json::from_str(V2_FIXTURE).expect_contract("v2 fixture");
    fixture["frames"]
        .as_array()
        .expect_contract("frames")
        .iter()
        .find(|f| f["captureLine"] == 29)
        .map(|f| f["frame"]["result"].clone())
        .expect_contract("capture line 29 (reasoning thinkingEnabled:false result)")
}

/// The captured `set_config_option` result for a fixture step.
fn captured_config_result(step: &str) -> serde_json::Value {
    let fixture: serde_json::Value =
        serde_json::from_str(KAS_FIXTURE).expect_contract("KAS fixture");
    let options = fixture["steps"]
        .as_array()
        .expect_contract("steps")
        .iter()
        .find(|s| s["step"] == step)
        .map(|s| s["configOptions"].clone())
        .expect_contract("fixture step");
    serde_json::json!({ "configOptions": options })
}

fn reasoning_params(enabled: bool) -> serde_json::Value {
    serde_json::json!({
        "sessionId": "fake-0",
        "command": {"command": "reasoning", "args": {"thinkingEnabled": enabled}},
    })
}

#[tokio::test]
async fn set_thinking_reasoning_wire_and_ack() {
    let ack = captured_reasoning_ack();
    assert_eq!(ack["success"], true, "fixture is the captured success ack");
    let responses = [
        ack,
        serde_json::json!({"success": false, "error": "nope"}),
        serde_json::json!({"success": false, "error": "", "message": "denied"}),
        serde_json::json!({"success": false}),
        serde_json::json!({"message": "no success field"}),
    ];
    let script = Rc::new(RefCell::new(Script::default()));
    script
        .borrow()
        .ext_responses
        .lock()
        .expect_contract("ext responses")
        .extend(
            responses
                .into_iter()
                .map(|r| ("kiro.dev/commands/execute".to_owned(), r)),
        );
    let observed = Rc::clone(&script);
    with_harness(script, |sender, mut rx, _permissions, _gate, _loop| async move {
        // B6 at the bridge: no active session is reported, nothing sent.
        sender
            .send(BridgeCommand::SetThinking {
                lever: ThinkingLever::ReasoningCommand,
                enabled: true,
            })
            .await
            .expect_contract("send before session");
        let no_session = next_notification("no session", &mut rx).await;
        assert!(
            matches!(&no_session, Notification::BridgeError { operation, message }
                if operation == "Thinking change" && message.contains("no active session")),
            "no session: {no_session:?}"
        );

        start_session(&sender, &mut rx).await;
        let expected: [(bool, Result<bool, &str>); 5] = [
            (false, Ok(false)),
            (true, Err("nope")),
            (true, Err("denied")),
            (false, Err("unknown error")),
            (true, Err("response missing success")),
        ];
        for (enabled, want) in expected {
            sender
                .send(BridgeCommand::SetThinking {
                    lever: ThinkingLever::ReasoningCommand,
                    enabled,
                })
                .await
                .expect_contract("send SetThinking");
            let got = next_notification("SetThinking reasoning", &mut rx).await;
            match want {
                Ok(value) => assert!(
                    matches!(got, Notification::ThinkingToggled { enabled } if enabled == value),
                    "ack for enabled={enabled}: {got:?}"
                ),
                Err(text) => assert!(
                    matches!(&got, Notification::BridgeError { operation, message }
                        if operation == "Thinking change" && message == text),
                    "failure {text:?} for enabled={enabled}: {got:?}"
                ),
            }
        }
    })
    .await;

    let calls = observed.borrow().ext_calls().clone();
    let want: Vec<(String, serde_json::Value)> = [false, true, true, false, true]
        .into_iter()
        .map(|enabled| {
            (
                "kiro.dev/commands/execute".to_owned(),
                reasoning_params(enabled),
            )
        })
        .collect();
    assert_eq!(
        calls, want,
        "exactly one reasoning request per toggle, args = {{thinkingEnabled}} only"
    );
}

#[tokio::test]
async fn set_thinking_reasoning_rpc_error_is_a_thinking_failure() {
    let script = Rc::new(RefCell::new(Script {
        fail_extensions: vec!["kiro.dev/commands/execute".to_owned()],
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permissions, _gate, _loop| async move {
            start_session(&sender, &mut rx).await;
            sender
                .send(BridgeCommand::SetThinking {
                    lever: ThinkingLever::ReasoningCommand,
                    enabled: false,
                })
                .await
                .expect_contract("send SetThinking");
            let got = next_notification("SetThinking rpc error", &mut rx).await;
            assert!(
                matches!(&got, Notification::BridgeError { operation, message }
                if operation == "Thinking change" && !message.is_empty()),
                "rpc error: {got:?}"
            );
        },
    )
    .await;
}

#[tokio::test]
async fn set_thinking_config_option_wire_and_ack() {
    let script = Rc::new(RefCell::new(Script::default()));
    script
        .borrow()
        .config_option_responses
        .lock()
        .expect_contract("config responses")
        .extend([
            captured_config_result("cfg_thinking_off"),
            captured_config_result("cfg_effort_max2"),
        ]);
    let observed = Rc::clone(&script);
    with_harness(
        script,
        |sender, mut rx, _permissions, _gate, _loop| async move {
            start_session(&sender, &mut rx).await;
            for (enabled, want) in [(false, "off"), (true, "on")] {
                sender
                    .send(BridgeCommand::SetThinking {
                        lever: ThinkingLever::ConfigOption,
                        enabled,
                    })
                    .await
                    .expect_contract("send SetThinking");
                let got = next_notification("SetThinking config", &mut rx).await;
                let Notification::ConfigOptionSet { config_id, options } = &got else {
                    panic!("expected ConfigOptionSet for enabled={enabled}, got {got:?}");
                };
                assert_eq!(config_id, "thinking", "ack names the thinking option");
                let thinking = options.iter().find(|o| o.key == "thinking");
                assert_eq!(
                    thinking.and_then(|o| o.value.as_deref()),
                    Some(want),
                    "rebuilt set reports thinking={want}"
                );
            }
        },
    )
    .await;

    let calls = observed.borrow().ext_calls().clone();
    assert_eq!(
        calls,
        [false, true]
            .into_iter()
            .map(|enabled| (
                "session/set_config_option".to_owned(),
                serde_json::json!({
                    "sessionId": "fake-0",
                    "configId": "thinking",
                    "value": if enabled { "on" } else { "off" },
                }),
            ))
            .collect::<Vec<_>>(),
        "exactly one set_config_option per toggle with the on/off literal"
    );
}

/// cyril-k3lz review findings 2 + 3: the mediator's session-start and
/// failed-load notifications, replayed in emitted order into a
/// `SessionController`, keep the snapshot's thinking state.
///
/// - A `session/new` whose `configOptions` report a toggleable model (the
///   captured `cfg_model` set, claude-sonnet-4.6, thinking on) must leave the
///   state toggleable: the `SessionCreated` reset precedes the snapshot.
/// - A recoverable `session/load` failure must not reset the live session.
#[tokio::test]
async fn session_start_and_failed_load_keep_thinking_state() {
    let script = Rc::new(RefCell::new(Script {
        new_session_config_options: Some(
            captured_config_result("cfg_model")["configOptions"].clone(),
        ),
        ..Script::default()
    }));
    with_harness(
        script,
        |sender, mut rx, _permissions, _gate, _loop| async move {
            sender
                .send(BridgeCommand::NewSession {
                    cwd: std::env::temp_dir(),
                })
                .await
                .expect_contract("send NewSession");
            let mut ctrl = crate::session::SessionController::new();
            let mut kinds = Vec::new();
            for label in ["usage", "created", "config"] {
                let got = next_notification(label, &mut rx).await;
                kinds.push(match &got {
                    Notification::UsageSessionStarted { .. } => "UsageSessionStarted",
                    Notification::SessionCreated { .. } => "SessionCreated",
                    Notification::ConfigOptionsUpdated(_) => "ConfigOptionsUpdated",
                    other => panic!("unexpected session-start notification {other:?}"),
                });
                ctrl.apply_notification(&got);
            }
            assert_eq!(
                kinds,
                ["UsageSessionStarted", "SessionCreated", "ConfigOptionsUpdated"],
                "the reset boundary precedes the session's own snapshot"
            );
            let toggleable = crate::types::ThinkingState::ToggleableByConfigOption { enabled: true };
            assert_eq!(ctrl.thinking(), &toggleable, "snapshot survives session start");

            sender
                .send(BridgeCommand::LoadSession {
                    session_id: crate::types::SessionId::new("load-typo"),
                })
                .await
                .expect_contract("send LoadSession");
            let failed = next_notification("failed load", &mut rx).await;
            assert!(
                matches!(&failed, Notification::BridgeError { operation, .. } if operation == "Load session"),
                "a failed load is an operation error, not a disconnect: {failed:?}"
            );
            ctrl.apply_notification(&failed);
            assert_eq!(ctrl.thinking(), &toggleable, "failed load keeps thinking");
            assert_eq!(
                ctrl.status(),
                &crate::types::session::SessionStatus::Active,
                "failed load keeps the live session active"
            );
        },
    )
    .await;
}
