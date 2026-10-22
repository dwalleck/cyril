use agent_client_protocol::schema::v1 as acp;
use serde_json::json;

use crate::protocol::engine::{Engine, KasEngine, V2Engine};
use crate::test_support::must_succeed;
use crate::types::Notification;

fn session_info_frame(kiro: serde_json::Value) -> acp::SessionNotification {
    must_succeed(
        serde_json::from_value(json!({
            "sessionId": "mcp-error-session",
            "update": {"sessionUpdate": "session_info_update", "_meta": {"kiro": kiro}},
        })),
        "source-shaped session-info envelope decodes through the SDK schema",
    )
}

/// Source-derived MCP payload, not a claimed raw MCP capture. The producer's
/// flat fields are authoritative; displayError is the optional emitted mirror.
#[test]
fn kas_mcp_display_error_preserves_conversion_result_and_message() {
    let message = "  MCP server 'private-server' connection failed: authorization refused.  ";
    for mirror in [false, true] {
        let mut kiro = json!({
            "kind": "display_error",
            "errorType": "mcp_connection_error",
            "message": message,
        });
        if mirror {
            kiro["displayError"] = json!({
                "errorType": "mcp_connection_error",
                "message": message,
            });
        }
        let frame = session_info_frame(kiro);
        let Some(Notification::McpConnectionError { message: converted }) =
            KasEngine::default().convert_session_update(&frame)
        else {
            panic!("a source-shaped MCP display_error must produce a classified consumer event");
        };
        assert_eq!(
            converted, message,
            "the original message must survive conversion",
        );
        assert!(V2Engine.convert_session_update(&frame).is_none());
    }
}

#[test]
fn kas_mcp_display_error_requires_flat_classification_and_nonempty_message() {
    let (_guard, capture, dispatch) = crate::test_support::capture_json_subscriber();
    tracing::dispatcher::with_default(&dispatch, || {
        for kiro in [
            json!({"kind": "display_error", "message": "private-error-not-for-logs"}),
            json!({"kind": "display_error", "errorType": null, "message": "private-error-not-for-logs"}),
            json!({"kind": "display_error", "errorType": "mcp_connection_error"}),
            json!({"kind": "display_error", "errorType": "mcp_connection_error", "message": null}),
            json!({"kind": "display_error", "errorType": "mcp_connection_error", "message": 7}),
            json!({"kind": "display_error", "errorType": "mcp_connection_error", "message": ""}),
            json!({"kind": "display_error", "errorType": "mcp_connection_error", "message": " \n\t"}),
            json!({"kind": "display_error", "displayError": {
                "errorType": "mcp_connection_error", "message": "private-error-not-for-logs",
            }}),
        ] {
            assert!(
                KasEngine::default()
                    .convert_session_update(&session_info_frame(kiro))
                    .is_none()
            );
        }
    });
    let events = capture.captured();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["level"] == "WARN")
            .count(),
        8
    );
    assert!(
        !must_succeed(
            serde_json::to_string(&events),
            "error diagnostics serialize"
        )
        .contains("private-error-not-for-logs")
    );
}

#[test]
fn kas_mcp_display_error_does_not_claim_other_error_kinds_or_missing_envelopes() {
    for kiro in [
        json!({"kind": "display_error", "errorType": "InvalidModelError", "message": "model unavailable"}),
        json!({"kind": "display_error", "errorType": "other_error", "message": "other failure",
            "displayError": {"errorType": "mcp_connection_error", "message": "not authoritative"}}),
        json!({"kind": "other_kind", "errorType": "mcp_connection_error", "message": "not display_error"}),
        json!({"errorType": "mcp_connection_error", "message": "no kind"}),
        serde_json::Value::Null,
    ] {
        assert!(
            KasEngine::default()
                .convert_session_update(&session_info_frame(kiro))
                .is_none()
        );
    }
    let no_meta = must_succeed(
        serde_json::from_value(json!({
            "sessionId": "mcp-error-session",
            "update": {"sessionUpdate": "session_info_update"},
        })),
        "missing optional metadata remains a valid SDK envelope",
    );
    assert!(
        KasEngine::default()
            .convert_session_update(&no_meta)
            .is_none()
    );
}
