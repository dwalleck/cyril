use super::kiro::to_ext_notification;
use crate::types::{Notification, SessionId};
use serde_json::json;

#[test]
fn v2_mcp_events_keep_real_outer_session_id_without_manufacturing_one() {
    for method in [
        "kiro.dev/mcp/server_initialized",
        "kiro.dev/mcp/server_init_failure",
        "kiro.dev/mcp/oauth_request",
    ] {
        for wire_id in [Some("actual-candidate"), None] {
            let mut params = json!({"serverName":"exact-server","error":"original cause","oauthUrl":"https://example.test/auth?state=exact#finish"});
            if let Some(id) = wire_id {
                params["sessionId"] = json!(id);
            }
            let notification = crate::test_support::must_succeed(
                to_ext_notification(method, &params),
                "v2 conversion",
            )
            .unwrap_or_else(|| panic!("recognized event"));
            let (session, server) = match &notification {
                Notification::McpServerInitialized {
                    session_id,
                    server_name,
                }
                | Notification::McpServerInitFailure {
                    session_id,
                    server_name,
                    ..
                }
                | Notification::McpOAuthRequest {
                    session_id,
                    server_name,
                    ..
                } => (session_id, server_name),
                other => panic!("wrong MCP event: {other:?}"),
            };
            assert_eq!(session.as_ref().map(SessionId::as_str), wire_id);
            assert_eq!(server, "exact-server");
            if let Notification::McpServerInitFailure { error, .. } = &notification {
                assert_eq!(error.as_deref(), Some("original cause"));
            }
            if let Notification::McpOAuthRequest { url, .. } = &notification {
                assert_eq!(url, "https://example.test/auth?state=exact#finish");
            }
        }
    }
}

#[test]
fn empty_or_nonstring_mcp_session_ids_remain_absent_not_active_root() {
    for id in [json!(""), json!(null), json!(7)] {
        let converted = crate::test_support::must_succeed(
            to_ext_notification(
                "kiro.dev/mcp/server_initialized",
                &json!({"sessionId":id,"serverName":"legacy"}),
            ),
            "conversion",
        )
        .unwrap_or_else(|| panic!("ready"));
        assert!(matches!(
            converted,
            Notification::McpServerInitialized {
                session_id: None,
                ..
            }
        ));
    }
}
