//! cyril-auxt (C1/C8): over the real bridge, a KAS permission request reaches
//! the App without V2 trust tiers, and an "Always allow" grant leaves as the
//! `_meta.kiro.consent` echo KAS persists from. A v2 request keeps its tiers.

use super::*;
use crate::always_allow::{AlwaysAllowGrant, GrantResource, GrantScope};
use crate::protocol::engine::KasEngine;
use crate::types::{PermissionOptionId, PermissionResponse};

fn options() -> serde_json::Value {
    serde_json::json!([
        {"optionId": "accept", "name": "Allow", "kind": "allow_once"},
        {"optionId": "always-accept", "name": "Always allow", "kind": "allow_always"},
        {"optionId": "reject", "name": "Deny", "kind": "reject_once"}
    ])
}

/// `_meta` carrying both dialects: v2 `trustOptions` and KAS `kiro.consent`.
fn meta() -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::new();
    meta.insert(
        "trustOptions".to_owned(),
        serde_json::json!([{
            "label": "Full command",
            "display": "echo AUXT",
            "setting_key": "allowedCommands",
            "patterns": ["echo AUXT"]
        }]),
    );
    meta.insert(
        "kiro".to_owned(),
        serde_json::json!({
            "toolId": "run_command",
            "command": "echo AUXT",
            "consent": {
                "capability": "shell",
                "resource": "echo AUXT",
                "askType": "implicit",
                "triggeringResource": "echo AUXT",
                "workspaceRoot": "/ws"
            }
        }),
    );
    meta
}

fn script(kas: bool) -> Rc<RefCell<Script>> {
    Rc::new(RefCell::new(Script {
        request_permission_on_prompt: true,
        permission_options: Some(options()),
        permission_meta: Some(meta()),
        wire_kas: kas.then_some(true),
        sess_ids: kas.then_some(true),
        ..Script::default()
    }))
}

async fn prompt(sender: &BridgeSender, rx: &mut mpsc::Receiver<RoutedNotification>) {
    let session_id = start_session(sender, rx).await;
    sender
        .send(BridgeCommand::SendPrompt {
            session_id,
            prompt: crate::types::PromptEnvelope::prepared(vec!["go".to_owned()], None),
        })
        .await
        .expect_contract("always-allow prompt accepted");
}

async fn shut_down(sender: &BridgeSender, loop_handle: tokio::task::JoinHandle<crate::Result<()>>) {
    sender
        .send(BridgeCommand::Shutdown)
        .await
        .expect_contract("always-allow shutdown command accepted");
    tokio::time::timeout(Duration::from_secs(5), loop_handle)
        .await
        .expect_contract("always-allow loop shutdown timeout")
        .expect_contract("always-allow loop result")
        .expect_contract("always-allow domain shutdown");
}

#[tokio::test]
async fn kas_always_allow_leaves_as_the_consent_echo_without_v2_tiers() {
    let script = script(true);
    let observed = Rc::clone(&script);
    with_engine_harness(
        Rc::new(KasEngine::default()),
        script,
        move |sender, mut rx, mut permission_rx, _gate, loop_handle, _kill| async move {
            prompt(&sender, &mut rx).await;
            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("always-allow permission request timeout")
                .expect_contract("always-allow permission request channel");
            assert!(
                request.trust_options.is_empty(),
                "KAS must not offer V2 trust tiers (agent-JSON write path)"
            );
            assert!(
                request.consent.is_some(),
                "KAS consent typed for the overlay"
            );
            request
                .responder
                .send(PermissionResponse::AlwaysAllow {
                    option_id: PermissionOptionId::new("always-accept"),
                    grant: AlwaysAllowGrant::new(
                        GrantScope::User,
                        GrantResource::Exact("echo AUXT".to_owned()),
                    ),
                })
                .expect_contract("always-allow response accepted");

            let response = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    if let Some(response) =
                        observed.borrow().permission_responses().first().cloned()
                    {
                        break response;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect_contract("always-allow wire response timeout");
            assert_eq!(response["outcome"]["optionId"], "always-accept");
            assert_eq!(
                response["_meta"],
                serde_json::json!({"kiro": {"consent": {
                    "capability": "shell",
                    "scope": "user",
                    "resource": "echo AUXT",
                    "workspaceRoot": "/ws"
                }}})
            );
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}

#[tokio::test]
async fn v2_request_keeps_its_trust_tiers() {
    with_harness(
        script(false),
        move |sender, mut rx, mut permission_rx, _gate, loop_handle| async move {
            prompt(&sender, &mut rx).await;
            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("v2 permission request timeout")
                .expect_contract("v2 permission request channel");
            assert_eq!(request.trust_options.len(), 1, "v2 tiers unchanged");
            assert_eq!(request.trust_options[0].label, "Full command");
            request
                .responder
                .send(PermissionResponse::Cancel)
                .expect_contract("v2 cancel accepted");
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}

fn shell_script(kas: bool, case: &str) -> Rc<RefCell<Script>> {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/kas_permission/shell-preview-2.32.0.json"
    )))
    .expect_contract("retained shell preview fixture");
    let pair = &fixture[case];
    let mut updates = pair["updates"]
        .as_array()
        .expect_contract("shell tool updates")
        .clone();
    for update in &mut updates {
        // The harness gives its permission stub this stable tool ID.
        update["toolCallId"] = serde_json::json!("permission-tool");
    }
    Rc::new(RefCell::new(Script {
        request_permission_on_prompt: true,
        permission_options: Some(pair["permission"]["options"].clone()),
        permission_meta: Some(
            serde_json::from_value(pair["permission"]["_meta"].clone())
                .expect_contract("captured permission metadata"),
        ),
        permission_tool_updates: updates,
        wire_kas: kas.then_some(true),
        sess_ids: kas.then_some(true),
        ..Script::default()
    }))
}

#[tokio::test]
async fn kas_shell_preview_reaches_permission_after_ledger_join() {
    for (case, with_tool_id, command) in [
        ("with_cwd", true, "wc -c hello.txt && xxd hello.txt"),
        ("without_cwd", true, "sleep 45 && echo SLEPT"),
        ("with_cwd", false, "wc -c hello.txt && xxd hello.txt"),
    ] {
        let script = shell_script(true, case);
        if !with_tool_id {
            script.borrow_mut().permission_meta = None;
        }
        let observed = Rc::clone(&script);
        let expected_raw = script.borrow().permission_tool_updates[0]["rawInput"].clone();
        with_engine_harness(
            Rc::new(KasEngine::default()),
            script,
            move |sender, mut rx, mut permission_rx, _gate, loop_handle, _kill| async move {
                prompt(&sender, &mut rx).await;
                let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                    .await
                    .expect_contract("joined shell permission timeout")
                    .expect_contract("joined shell permission channel");
                let shell = request
                    .tool_call
                    .shell_command()
                    .expect_contract("KAS post-join shell projection");
                assert_eq!(shell.command(), command);
                assert_eq!(shell.cwd(), expected_raw["cwd"].as_str());
                assert_eq!(request.tool_call.raw_input(), Some(&expected_raw));
                assert_eq!(request.tool_call.kind(), crate::types::ToolKind::Execute);
                assert_eq!(
                    request.tool_call.status(),
                    crate::types::ToolCallStatus::InProgress,
                    "kind-omitting update was joined before permission delivery",
                );
                assert_eq!(
                    request.options.len(),
                    4,
                    "offered authorization choices retained"
                );
                assert!(request.trust_options.is_empty());
                assert!(request.can_reject_with_reason);
                if with_tool_id {
                    assert_eq!(
                        request
                            .consent
                            .as_ref()
                            .expect_contract("KAS consent retained")
                            .wire_capability(),
                        Some("shell"),
                    );
                }
                request
                    .responder
                    .send(PermissionResponse::Selected {
                        option_id: PermissionOptionId::new("accept"),
                        trust_option: None,
                    })
                    .expect_contract("shell approval answer");
                let response = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if let Some(response) =
                            observed.borrow().permission_responses().first().cloned()
                        {
                            break response;
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect_contract("shell approval wire answer");
                assert_eq!(response["outcome"]["optionId"], "accept");
                shut_down(&sender, loop_handle).await;
            },
        )
        .await;
    }
}

#[tokio::test]
async fn kas_shell_preview_uses_final_raw_input_without_stub_or_consent_fallbacks() {
    for (replacement, expected_command) in [
        (
            Some(serde_json::json!({"command": "  printf final |\n    cat  "})),
            Some("  printf final |\n    cat  "),
        ),
        (Some(serde_json::json!({"command": null})), None),
        (
            Some(serde_json::json!({"description": "not a command"})),
            None,
        ),
        (None, Some("wc -c hello.txt && xxd hello.txt")),
    ] {
        let script = shell_script(true, "with_cwd");
        let mut expected_raw = script.borrow().permission_tool_updates[0]["rawInput"].clone();
        {
            let mut script = script.borrow_mut();
            let update = script
                .permission_tool_updates
                .last_mut()
                .expect_contract("kind-omitting update");
            if let Some(raw) = replacement {
                update["rawInput"] = raw.clone();
                expected_raw = raw;
            } else {
                assert!(
                    update
                        .as_object_mut()
                        .expect_contract("update object")
                        .remove("rawInput")
                        .is_some(),
                );
            }
            script.permission_raw_input =
                Some(serde_json::json!({"command": "stub must not override joined raw"}));
        }
        with_engine_harness(
            Rc::new(KasEngine::default()),
            script,
            move |sender, mut rx, mut permission_rx, _gate, loop_handle, _kill| async move {
                prompt(&sender, &mut rx).await;
                let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                    .await
                    .expect_contract("updated shell permission timeout")
                    .expect_contract("updated shell permission channel");
                assert_eq!(
                    request
                        .tool_call
                        .shell_command()
                        .map(crate::types::ShellCommand::command),
                    expected_command,
                );
                assert_eq!(
                    request
                        .tool_call
                        .shell_command()
                        .and_then(crate::types::ShellCommand::cwd),
                    expected_command.and(expected_raw["cwd"].as_str()),
                    "missing cwd is not replaced with consent workspaceRoot",
                );
                assert_eq!(request.tool_call.raw_input(), Some(&expected_raw));
                request
                    .responder
                    .send(PermissionResponse::Cancel)
                    .expect_contract("updated shell permission cancellation");
                shut_down(&sender, loop_handle).await;
            },
        )
        .await;
    }
}

#[tokio::test]
async fn v2_shell_shaped_permission_never_gets_kas_projection() {
    let script = shell_script(false, "with_cwd");
    let expected_raw = script.borrow().permission_tool_updates[0]["rawInput"].clone();
    with_harness(
        script,
        move |sender, mut rx, mut permission_rx, _gate, loop_handle| async move {
            prompt(&sender, &mut rx).await;
            let request = tokio::time::timeout(Duration::from_secs(5), permission_rx.recv())
                .await
                .expect_contract("v2 shell-shaped permission timeout")
                .expect_contract("v2 shell-shaped permission channel");
            assert_eq!(request.tool_call.kind(), crate::types::ToolKind::Execute);
            assert_eq!(request.tool_call.raw_input(), Some(&expected_raw));
            assert!(
                request.tool_call.shell_command().is_none(),
                "KAS metadata and execute-shaped raw input do not classify other engines",
            );
            request
                .responder
                .send(PermissionResponse::Cancel)
                .expect_contract("v2 shell-shaped permission cancellation");
            shut_down(&sender, loop_handle).await;
        },
    )
    .await;
}
