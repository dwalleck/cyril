use agent_client_protocol::UntypedMessage;
use serde_json::json;
use std::rc::Rc;
use std::time::Duration;

use super::super::{CommandOutcome, DomainConfig, DomainMediator};
use crate::protocol::bridge::create_channel_pair;
use crate::protocol::engine::KasEngine;
use crate::test_support::must_succeed;
use crate::types::*;

fn created(session: &str) -> Notification {
    Notification::SessionCreated {
        session_id: SessionId::new(session),
        current_mode: None,
        current_model: None,
        available_modes: vec![],
        available_models: vec![],
    }
}

#[tokio::test]
async fn early_status_and_policy_promote_only_matching_response_after_reset_and_config() {
    let (handle, bridge) = create_channel_pair();
    let (_sender, mut rx, _permissions, _source, _completion) = handle.split();
    let (mut mediator, _channels) = must_succeed(
        DomainMediator::new(
            DomainConfig {
                engine: Rc::new(KasEngine::default()),
                cwd: std::env::temp_dir(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                host_shell: None,
                kas_token: None,
            },
            bridge,
        ),
        "MCP mediator",
    );
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new("old"),
                SessionOrigin::Fresh,
                None,
                created("old"),
            )
            .await,
        "old session",
    );
    must_succeed(rx.try_recv(), "old usage");
    must_succeed(rx.try_recv(), "old created");
    must_succeed(mediator.mcp.begin_session(None), "admit early new");
    for (method, params) in [
        (
            "_kiro/mcp/status",
            json!({"sessionId":"peer","servers":[{"name":"wrong","status":"disabled"}]}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":[{"name":"first","status":"connecting"}]}),
        ),
        ("_kiro/mcp/status", json!({"sessionId":"root","servers":[]})),
        (
            "_kiro/mcp/governance_disabled",
            json!({"sessionId":"root","reason":"api_failure"}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":"malformed"}),
        ),
    ] {
        must_succeed(
            mediator
                .handle_extension_notification(must_succeed(
                    UntypedMessage::new(method, params),
                    "MCP frame",
                ))
                .await,
            "MCP ingress",
        );
    }
    assert!(
        rx.try_recv().is_err(),
        "early MCP data must not be misattributed to old root"
    );
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new("root"),
                SessionOrigin::Fresh,
                Some(Notification::ConfigOptionsUpdated(vec![])),
                created("root"),
            )
            .await,
        "new root",
    );
    let mut controller = crate::session::SessionController::new();
    let mut labels = vec![];
    while let Ok(routed) = rx.try_recv() {
        let label = match &routed.notification {
            Notification::UsageSessionStarted { .. } => "usage",
            Notification::SessionCreated { .. } => "created",
            Notification::ConfigOptionsUpdated(_) => "config",
            Notification::McpUpdate(McpUpdate::Snapshot(_)) => "snapshot",
            Notification::McpUpdate(McpUpdate::Policy { .. }) => "policy",
            Notification::McpUpdate(McpUpdate::Diagnostic { .. }) => "diagnostic",
            other => panic!("unexpected publication: {other:?}"),
        };
        labels.push(label);
        controller.apply_notification(&routed.notification);
    }
    assert_eq!(
        labels,
        [
            "usage",
            "created",
            "config",
            "snapshot",
            "policy",
            "diagnostic"
        ]
    );
    let state = controller.mcp();
    assert_eq!(
        state.session_id.as_ref().map(SessionId::as_str),
        Some("root")
    );
    assert!(
        state
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.servers.is_empty())
    );
    assert_eq!(state.policy, Some(McpPolicy::ApiFailure));
    assert!(state.diagnostic.is_some());
    for session in ["old", "peer"] {
        must_succeed(mediator.handle_extension_notification(must_succeed(UntypedMessage::new("_kiro/mcp/status",json!({"sessionId":session,"servers":[{"name":"wrong","status":"connecting"}]})),"late frame")).await,"late ingress");
    }
    assert!(
        rx.try_recv().is_err(),
        "old and workflow snapshots never publish into current root"
    );
    must_succeed(
        mediator
            .mcp
            .begin_session(Some(SessionId::new("failed-load"))),
        "admit failed load",
    );
    must_succeed(
        mediator
            .handle_extension_notification(must_succeed(
                UntypedMessage::new(
                    "_kiro/mcp/status",
                    json!({"sessionId":"failed-load","servers":[]}),
                ),
                "failed early",
            ))
            .await,
        "failed ingress",
    );
    assert!(
        must_succeed(
            mediator
                .apply_command_outcome(CommandOutcome::SessionStartFailed(Box::new(
                    Notification::BridgeError {
                        operation: "Load session".into(),
                        message: "not found".into()
                    }
                )))
                .await,
            "load failure"
        )
        .is_continue()
    );
    must_succeed(rx.try_recv(), "visible load failure");
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new("failed-load"),
                SessionOrigin::Loaded,
                None,
                created("failed-load"),
            )
            .await,
        "later load",
    );
    must_succeed(rx.try_recv(), "later usage");
    must_succeed(rx.try_recv(), "later created");
    assert!(
        rx.try_recv().is_err(),
        "failed attempt staging is discarded"
    );
}

#[test]
fn pending_claim_is_synchronous_and_late_results_cannot_clear_new_session_or_operation() {
    use super::super::mcp::{McpOwner, McpRequestPlan};
    let mut owner = McpOwner::default();
    owner.activate(SessionId::new("root"));
    let update = must_succeed(
        crate::protocol::convert::kas::mcp::convert(
            "kiro/mcp/status",
            &json!({"sessionId":"root","servers":[{"name":"server","status":"connecting"}]}),
        ),
        "status",
    )
    .unwrap_or_else(|| panic!("status"));
    let Notification::McpUpdate(update) = update else {
        panic!("MCP");
    };
    owner.ingest(&update);
    let request = McpRequest {
        session_id: SessionId::new("root"),
        server_name: "server".into(),
        operation: McpOperation::Reset { start_oauth: true },
    };
    let first = match owner.request(request.clone()) {
        McpRequestPlan::Pending(pending) => pending,
        McpRequestPlan::Immediate(outcome) => panic!("first: {outcome:?}"),
    };
    let McpRequestPlan::Immediate(outcome) = owner.request(request.clone()) else {
        panic!("duplicate request must complete immediately");
    };
    assert_eq!(
        outcome.result.err().map(|error| error.kind),
        Some(McpErrorKind::AlreadyPending)
    );
    let old = McpOutcome {
        id: first.id,
        request: request.clone(),
        result: Ok(McpOperationResult::Acknowledged),
    };
    assert_eq!(owner.cancel().len(), 1);
    let next = match owner.request(request) {
        McpRequestPlan::Pending(pending) => pending,
        McpRequestPlan::Immediate(outcome) => panic!("next: {outcome:?}"),
    };
    assert!(owner.finish(old.clone()).is_none());
    assert_eq!(owner.state.pending["server"].id, next.id);
    owner.activate(SessionId::new("replacement"));
    assert!(owner.finish(old).is_none());
    assert!(owner.state.pending.is_empty());
}

#[test]
fn matching_promotion_preserves_early_passive_url_through_latest_connecting_snapshot() {
    let mut owner = super::super::mcp::McpOwner::default();
    must_succeed(owner.begin_session(None), "admit early new");
    for server in [
        json!({"name":"server","status":"failed","failedAuthorization":true,"authorizationUrl":"https://auth.invalid/passive","errorMessage":"Unauthorized"}),
        json!({"name":"server","status":"connecting"}),
    ] {
        let notification = must_succeed(
            crate::protocol::convert::kas::mcp::convert(
                "kiro/mcp/status",
                &json!({"sessionId":"root","servers":[server]}),
            ),
            "early snapshot",
        )
        .unwrap_or_else(|| panic!("snapshot"));
        let Notification::McpUpdate(update) = notification else {
            panic!("MCP update");
        };
        assert!(!owner.ingest(&update));
    }
    let updates = owner.activate(SessionId::new("root"));
    let mut state = McpState::for_session(SessionId::new("root"));
    for update in &updates {
        state.apply(update);
    }
    assert!(matches!(
        state
            .snapshot
            .as_ref()
            .unwrap_or_else(|| panic!("snapshot"))
            .servers[0]
            .status,
        McpServerStatus::Connecting
    ));
    assert_eq!(
        state.authorization_urls.get("server").map(String::as_str),
        Some("https://auth.invalid/passive")
    );
}

#[test]
fn a_successful_late_read_revalidates_latest_connection_catalog_and_original_server() {
    use super::super::mcp::{McpOwner, McpRequestPlan};
    for (latest, expected) in [
        (
            json!([{"name":"server","status":"connecting"}]),
            McpErrorKind::Disconnected,
        ),
        (
            json!([{"name":"server","status":"connected","tools":[],"prompts":[],"resources":[],"resourceTemplates":[]}]),
            McpErrorKind::EntryMissing,
        ),
        (
            json!([{"name":"replacement","status":"connected","tools":[],"prompts":[{"name":"prompt","promptName":"prompt"}],"resources":[],"resourceTemplates":[]}]),
            McpErrorKind::ServerMissing,
        ),
    ] {
        let mut owner = McpOwner::default();
        owner.activate(SessionId::new("root"));
        let decode = |servers| {
            let notification = must_succeed(
                crate::protocol::convert::kas::mcp::convert(
                    "kiro/mcp/status",
                    &json!({"sessionId":"root","servers":servers}),
                ),
                "snapshot",
            )
            .unwrap_or_else(|| panic!("snapshot"));
            let Notification::McpUpdate(update) = notification else {
                panic!("MCP snapshot");
            };
            update
        };
        owner.ingest(&decode(json!([{"name":"server","status":"connected","tools":[],"prompts":[{"name":"prompt","promptName":"prompt"}],"resources":[],"resourceTemplates":[]}])));
        let request = McpRequest {
            session_id: SessionId::new("root"),
            server_name: "server".into(),
            operation: McpOperation::GetPrompt {
                name: "prompt".into(),
                arguments: Default::default(),
            },
        };
        let pending = match owner.request(request.clone()) {
            McpRequestPlan::Pending(pending) => pending,
            McpRequestPlan::Immediate(outcome) => panic!("accepted read: {outcome:?}"),
        };
        owner.ingest(&decode(latest));
        let outcome = owner
            .finish(McpOutcome {
                id: pending.id,
                request: request.clone(),
                result: Ok(McpOperationResult::Preview(std::sync::Arc::new(
                    McpPreview::Prompt {
                        description: None,
                        messages: vec![],
                    },
                ))),
            })
            .unwrap_or_else(|| panic!("correlated outcome"));
        assert_eq!(
            outcome.request, request,
            "a changed selection must never retarget the sent request"
        );
        assert_eq!(outcome.result.err().map(|error| error.kind), Some(expected));
        assert!(owner.state.pending.is_empty());
    }
}

async fn push_mcp_frame(mediator: &mut DomainMediator, method: &str, params: serde_json::Value) {
    must_succeed(
        mediator
            .handle_extension_notification(must_succeed(
                UntypedMessage::new(method, params),
                "MCP frame",
            ))
            .await,
        "MCP ingress",
    );
}

fn collect_publications(
    rx: &mut tokio::sync::mpsc::Receiver<RoutedNotification>,
    controller: &mut crate::session::SessionController,
) -> Vec<RoutedNotification> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        controller.apply_notification(&event.notification);
        events.push(event);
    }
    events
}

async fn active_mcp_root() -> (
    DomainMediator,
    tokio::sync::mpsc::Receiver<RoutedNotification>,
    crate::session::SessionController,
) {
    let (handle, bridge) = create_channel_pair();
    let (_sender, mut rx, _permissions, _source, _completion) = handle.split();
    let (mut mediator, _channels) = must_succeed(
        DomainMediator::new(
            DomainConfig {
                engine: Rc::new(KasEngine::default()),
                cwd: std::env::temp_dir(),
                present_as: None,
                stall_threshold: Duration::from_secs(30),
                host_shell: None,
                kas_token: None,
            },
            bridge,
        ),
        "active MCP mediator",
    );
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new("root"),
                SessionOrigin::Fresh,
                None,
                created("root"),
            )
            .await,
        "active root",
    );
    push_mcp_frame(
        &mut mediator,
        "_kiro/mcp/status",
        json!({
            "sessionId":"root",
            "servers":[
                {"name":"oauth","status":"failed","failedAuthorization":true,
                 "authorizationUrl":"https://auth.invalid/obsolete","errorMessage":"Unauthorized"},
                {"name":"removed","status":"disabled"}
            ]
        }),
    )
    .await;
    let mut controller = crate::session::SessionController::new();
    collect_publications(&mut rx, &mut controller);
    assert_eq!(
        controller
            .mcp()
            .authorization_urls
            .get("oauth")
            .map(String::as_str),
        Some("https://auth.invalid/obsolete"),
        "establish the real published passive URL"
    );
    (mediator, rx, controller)
}

async fn active_root_updates_during_failed_load(target: &str) {
    let (mut mediator, mut rx, mut controller) = active_mcp_root().await;
    assert!(must_succeed(
        mediator
            .begin_mcp_session(Some(SessionId::new(target)))
            .await,
        "admit load"
    ));
    // A different candidate and an unrelated peer must remain isolated even
    // though updates for the still-active root remain authoritative.
    for candidate in ["peer", target]
        .into_iter()
        .filter(|candidate| *candidate != "root")
    {
        push_mcp_frame(
            &mut mediator,
            "_kiro/mcp/status",
            json!({
                "sessionId":candidate,"servers":[{"name":"candidate-only","status":"connecting"}]
            }),
        )
        .await;
        push_mcp_frame(
            &mut mediator,
            "_kiro/mcp/governance_disabled",
            json!({"sessionId":candidate,"reason":"api_failure"}),
        )
        .await;
        push_mcp_frame(
            &mut mediator,
            "_kiro/mcp/status",
            json!({"sessionId":candidate,"servers":"candidate malformed"}),
        )
        .await;
    }
    for (method, params) in [
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":[
                {"name":"oauth","status":"connected","tools":[],"prompts":[],"resources":[],"resourceTemplates":[]}
            ]}),
        ),
        (
            "_kiro/mcp/governance_disabled",
            json!({"sessionId":"root","reason":"admin_disabled"}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":"malformed"}),
        ),
    ] {
        push_mcp_frame(&mut mediator, method, params).await;
    }
    // Capture consumer state before resolving the load; flushing on failure
    // alone must not make a still-active root appear live during the wait.
    let live = collect_publications(&mut rx, &mut controller);
    let before_failure = controller.mcp();
    assert!(
        must_succeed(
            mediator
                .apply_command_outcome(CommandOutcome::SessionStartFailed(Box::new(
                    Notification::BridgeError {
                        operation: "Load session".into(),
                        message: "recoverable load refusal".into()
                    }
                ),))
                .await,
            "recoverable load failure"
        )
        .is_continue()
    );
    let failure = collect_publications(&mut rx, &mut controller);
    assert!(
        matches!(&failure[..], [RoutedNotification { notification: Notification::BridgeError { operation, message }, .. }]
        if operation == "Load session" && message == "recoverable load refusal")
    );
    assert_eq!(
        controller.id().map(SessionId::as_str),
        Some("root"),
        "failed load preserves the established root"
    );
    assert!(
        before_failure.authorization_urls.is_empty(),
        "a connected update must clear the obsolete OAuth URL before the load completes"
    );
    let snapshot = before_failure
        .snapshot
        .as_ref()
        .unwrap_or_else(|| panic!("live root snapshot"));
    assert_eq!(
        snapshot.servers.len(),
        1,
        "the active replacement removes the obsolete server"
    );
    assert_eq!(snapshot.servers[0].name, "oauth");
    assert!(matches!(
        snapshot.servers[0].status,
        McpServerStatus::Connected { .. }
    ));
    assert_eq!(before_failure.policy, Some(McpPolicy::AdminDisabled));
    assert!(
        before_failure
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.starts_with("Malformed MCP data:"))
    );
    assert_eq!(
        live.len(),
        3,
        "publish status, governance and diagnostic immediately, never candidate data"
    );
    for (event, expected) in live.iter().zip(["snapshot", "policy", "diagnostic"]) {
        assert_eq!(
            event.session_id.as_ref().map(SessionId::as_str),
            Some("root")
        );
        assert!(matches!(
            (&event.notification, expected),
            (Notification::McpUpdate(McpUpdate::Snapshot(_)), "snapshot")
                | (Notification::McpUpdate(McpUpdate::Policy { .. }), "policy")
                | (
                    Notification::McpUpdate(McpUpdate::Diagnostic { .. }),
                    "diagnostic"
                )
        ));
    }
    assert_eq!(
        controller.mcp(),
        before_failure,
        "failure must not roll back published authority"
    );
    assert_eq!(&mediator.mcp.state, before_failure.as_ref());
    assert!(must_succeed(
        mediator
            .begin_mcp_session(Some(SessionId::new(target)))
            .await,
        "retry released load"
    ));
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new(target),
                SessionOrigin::Loaded,
                None,
                created(target),
            )
            .await,
        "retry load without new MCP pushes",
    );
    let retry = collect_publications(&mut rx, &mut controller);
    assert_eq!(
        retry.len(),
        2,
        "failed staging cannot reappear in a later successful load"
    );
    assert!(matches!(
        retry[0].notification,
        Notification::UsageSessionStarted { .. }
    ));
    assert!(matches!(
        retry[1].notification,
        Notification::SessionCreated { .. }
    ));
    assert_eq!(
        controller.mcp().as_ref(),
        &McpState::for_session(SessionId::new(target))
    );
}

#[tokio::test]
async fn same_id_reload_failure_preserves_live_status_policy_diagnostic_and_url_resolution() {
    active_root_updates_during_failed_load("root").await;
}

#[tokio::test]
async fn different_id_load_failure_discards_candidate_without_hiding_active_root_updates() {
    active_root_updates_during_failed_load("candidate").await;
}

#[tokio::test]
async fn same_id_reload_success_replays_live_mcp_authority_after_created_and_config() {
    let (mut mediator, mut rx, mut controller) = active_mcp_root().await;
    assert!(must_succeed(
        mediator
            .begin_mcp_session(Some(SessionId::new("root")))
            .await,
        "admit same-ID load"
    ));
    for (method, params) in [
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":[
                {"name":"oauth","status":"failed","failedAuthorization":true,
                 "authorizationUrl":"https://auth.invalid/current","errorMessage":"Authorize current attempt"}
            ]}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":[{"name":"oauth","status":"connecting"}]}),
        ),
        (
            "_kiro/mcp/governance_disabled",
            json!({"sessionId":"root","reason":"api_failure"}),
        ),
        (
            "_kiro/mcp/status",
            json!({"sessionId":"root","servers":"malformed"}),
        ),
    ] {
        push_mcp_frame(&mut mediator, method, params).await;
    }
    let live = collect_publications(&mut rx, &mut controller);
    let before_success = controller.mcp();
    must_succeed(
        mediator
            .publish_session_start(
                SessionId::new("root"),
                SessionOrigin::Loaded,
                Some(Notification::ConfigOptionsUpdated(vec![])),
                created("root"),
            )
            .await,
        "successful same-ID load",
    );
    let promoted = collect_publications(&mut rx, &mut controller);
    let labels: Vec<_> = promoted
        .iter()
        .map(|event| match &event.notification {
            Notification::UsageSessionStarted { .. } => "usage",
            Notification::SessionCreated { .. } => "created",
            Notification::ConfigOptionsUpdated(_) => "config",
            Notification::McpUpdate(McpUpdate::Snapshot(_)) => "snapshot",
            Notification::McpUpdate(McpUpdate::AuthorizationUrls { .. }) => "urls",
            Notification::McpUpdate(McpUpdate::Policy { .. }) => "policy",
            Notification::McpUpdate(McpUpdate::Diagnostic { .. }) => "diagnostic",
            other => panic!("unexpected reload publication: {other:?}"),
        })
        .collect();
    assert_eq!(
        labels,
        [
            "usage",
            "created",
            "config",
            "snapshot",
            "urls",
            "policy",
            "diagnostic"
        ]
    );
    let state = controller.mcp();
    let snapshot = state
        .snapshot
        .as_ref()
        .unwrap_or_else(|| panic!("replayed root snapshot"));
    assert_eq!(snapshot.servers.len(), 1);
    assert_eq!(snapshot.servers[0].name, "oauth");
    assert!(matches!(
        snapshot.servers[0].status,
        McpServerStatus::Connecting
    ));
    assert_eq!(
        state.authorization_urls.get("oauth").map(String::as_str),
        Some("https://auth.invalid/current")
    );
    assert_eq!(state.policy, Some(McpPolicy::ApiFailure));
    assert!(
        state
            .diagnostic
            .as_deref()
            .is_some_and(|message| message.starts_with("Malformed MCP data:"))
    );
    for event in promoted.iter().skip(3) {
        assert_eq!(
            event.session_id.as_ref().map(SessionId::as_str),
            Some("root")
        );
    }
    assert_eq!(
        live.len(),
        4,
        "the established root must also receive each push before successful reset"
    );
    assert_eq!(
        before_success, state,
        "success must replay the same authority, not lose the live updates at reset"
    );
    assert_eq!(&mediator.mcp.state, state.as_ref());
}
