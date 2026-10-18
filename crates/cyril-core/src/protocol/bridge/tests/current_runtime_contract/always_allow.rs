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
